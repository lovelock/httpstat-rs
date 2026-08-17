mod output;
mod timing;

use clap::Parser;
use curl::easy::Easy;
use std::env;
use std::fs;
use std::io::Write;
use std::process;
use std::sync::{Arc, Mutex};

use timing::CurlTimings;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "httpstat",
    version = VERSION,
    about = "curl statistics made simple",
    long_about = "Usage: httpstat URL [CURL_OPTIONS]\n       httpstat -h | --help\n       httpstat --version\n\nArguments:\n  URL     url to request, could be with or without `http(s)://` prefix\n\nOptions:\n  CURL_OPTIONS  any curl supported options, except for -w -D -o -S -s,\n                which are already used internally.\n  -h --help     show this screen.\n  --version     show version.\n  -f --format   output format: pretty, json, jsonl. Default is `pretty`.\n  --slo         SLO thresholds as key=value pairs, e.g. `total=500,connect=100`.\n                Valid keys: total, connect, ttfb, dns, tls.\n                Exits with code 4 on violation.\n  --save        save structured output to a file path."
)]
struct Cli {
    /// URL to request
    url: String,

    /// Output format: pretty, json, jsonl
    #[arg(short = 'f', long = "format", default_value = "pretty")]
    format: String,

    /// SLO thresholds as key=value pairs
    #[arg(long = "slo")]
    slo: Option<String>,

    /// Save structured output to a file path
    #[arg(long = "save")]
    save: Option<String>,

    /// Extra curl arguments (everything after URL)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    curl_args: Vec<String>,
}

fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    }
}

fn duration_to_secs(d: Result<std::time::Duration, curl::Error>) -> f64 {
    d.map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|v| {
            let v = v.trim().to_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}

fn atty_is_tty() -> bool {
    unsafe { libc_isatty(1) != 0 }
}

extern "C" {
    #[link_name = "isatty"]
    fn libc_isatty(fd: i32) -> i32;
}

fn run() -> i32 {
    let cli = Cli::parse();
    let url = normalize_url(&cli.url);
    let output_format = if env_bool("HTTPSTAT_METRICS_ONLY", false) && cli.format == "pretty" {
        "json".to_string()
    } else {
        cli.format.clone()
    };

    if !matches!(output_format.as_str(), "pretty" | "json" | "jsonl") {
        eprintln!("Error: invalid format \"{}\", must be pretty, json, or jsonl", output_format);
        return 1;
    }

    // Parse SLO
    let slo = if let Some(ref spec) = cli.slo {
        match timing::parse_slo(spec) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("Error: {}", e);
                return 1;
            }
        }
    } else {
        None
    };

    // Env vars
    let show_body = env_bool("HTTPSTAT_SHOW_BODY", false);
    let show_ip = env_bool("HTTPSTAT_SHOW_IP", true);
    let show_speed = env_bool("HTTPSTAT_SHOW_SPEED", false);
    let save_body = env_bool("HTTPSTAT_SAVE_BODY", true);
    let is_debug = env_bool("HTTPSTAT_DEBUG", false);

    let use_color = env::var("NO_COLOR").is_err() && atty_is_tty();

    // Validate curl args
    let exclude_options = ["-w", "--write-out", "-D", "--dump-header", "-o", "--output", "-s", "--silent"];
    for arg in &cli.curl_args {
        if exclude_options.contains(&arg.as_str()) {
            eprintln!("Error: {} is not allowed in extra curl args", arg);
            return 1;
        }
    }

    if is_debug {
        eprintln!("Envs:");
        eprintln!("  HTTPSTAT_SHOW_BODY={}", show_body);
        eprintln!("  HTTPSTAT_SHOW_IP={}", show_ip);
        eprintln!("  HTTPSTAT_SHOW_SPEED={}", show_speed);
        eprintln!("  HTTPSTAT_SAVE_BODY={}", save_body);
        eprintln!("  HTTPSTAT_DEBUG={}", is_debug);
    }

    // Setup curl
    let mut easy = Easy::new();
    easy.url(&url).unwrap_or_else(|e| {
        eprintln!("httpstat: invalid URL: {}", e);
        process::exit(1);
    });
    easy.follow_location(true).unwrap();
    easy.max_redirections(10).unwrap();
    easy.signal(false).unwrap();

    // Collect response headers
    let headers_raw = Arc::new(Mutex::new(Vec::new()));
    let headers_clone = Arc::clone(&headers_raw);
    easy.header_function(move |header| {
        headers_clone.lock().unwrap().extend_from_slice(header);
        true
    }).unwrap();

    // Collect body
    let body_raw = Arc::new(Mutex::new(Vec::new()));
    let body_clone = Arc::clone(&body_raw);
    easy.write_function(move |data| {
        body_clone.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }).unwrap();

    if let Err(e) = easy.perform() {
        eprintln!("httpstat: curl error: {}", e);
        return easy.os_errno().unwrap_or(1);
    }

    // Collect timing metrics
    let timings = CurlTimings {
        namelookup: duration_to_secs(easy.namelookup_time()),
        connect: duration_to_secs(easy.connect_time()),
        appconnect: duration_to_secs(easy.appconnect_time()),
        pretransfer: duration_to_secs(easy.pretransfer_time()),
        starttransfer: duration_to_secs(easy.starttransfer_time()),
        total: duration_to_secs(easy.total_time()),
        speed_download: 0.0,
        speed_upload: 0.0,
        remote_ip: easy.primary_ip().ok().flatten().unwrap_or("").to_string(),
        remote_port: easy.primary_port().map(|p| p.to_string()).unwrap_or_default(),
        local_ip: easy.local_ip().ok().flatten().unwrap_or("").to_string(),
        local_port: easy.local_port().map(|p| p.to_string()).unwrap_or_default(),
    };

    // Read headers
    let headers_guard = headers_raw.lock().unwrap();
    let headers_text = String::from_utf8_lossy(&headers_guard).to_string();
    let status_line = headers_text
        .lines()
        .next()
        .unwrap_or("HTTP/1.1 000")
        .trim()
        .to_string();
    drop(headers_guard);

    // Read body
    let body_guard = body_raw.lock().unwrap();
    let body_bytes = body_guard.clone();
    let body_text = String::from_utf8_lossy(&body_bytes).to_string();
    let body_total_len = body_bytes.len();
    drop(body_guard);

    // Compute download speed
    let mut timings = timings;
    if timings.total > 0.0 {
        timings.speed_download = body_total_len as f64 / timings.total;
    }

    // Temp file for body
    let body_path = if save_body {
        let mut tmp = std::env::temp_dir();
        tmp.push("httpstat_body_XXXXXX");
        let path_str = tmp.to_string_lossy().to_string();
        if let Ok(mut f) = fs::File::create(&path_str) {
            let _ = f.write_all(&body_bytes);
            Some(path_str)
        } else {
            None
        }
    } else {
        None
    };

    // Check SLO
    let slo_result = slo.as_ref().map(|s| timing::check_slo(s, &timings));
    let mut exit_code = 0;
    if let Some((pass, _)) = &slo_result {
        if !pass {
            exit_code = 4;
        }
    }

    // --- Output ---
    if output_format == "json" || output_format == "jsonl" {
        let result = timing::build_json_result(
            &url,
            &timings,
            &headers_text,
            slo_result,
            exit_code,
        );
        let output_text = if output_format == "json" {
            serde_json::to_string_pretty(&result).unwrap()
        } else {
            serde_json::to_string(&result).unwrap()
        };
        println!("{}", output_text);

        if let Some(ref save_path) = cli.save {
            if let Ok(mut f) = fs::File::create(save_path) {
                let _ = writeln!(f, "{}", output_text);
            }
        }

        return exit_code;
    }

    // --- Pretty mode ---
    let pretty_opts = output::PrettyOptions {
        show_body,
        show_ip,
        show_speed,
        save_body,
        body_path: body_path.clone(),
        body_content: Some(body_text),
        body_total_len: Some(body_total_len),
    };

    output::print_pretty(&timings, &status_line, &headers_text, &pretty_opts, use_color);

    // SLO violations in pretty mode
    if let Some((false, ref violations)) = slo_result {
        output::print_slo_violations(violations, use_color);
    }

    // Save pretty output as json if --save specified
    if cli.save.is_some() {
        let result = timing::build_json_result(
            &url,
            &timings,
            &headers_text,
            slo_result,
            exit_code,
        );
        if let Some(ref save_path) = cli.save {
            if let Ok(mut f) = fs::File::create(save_path) {
                let _ = writeln!(f, "{}", serde_json::to_string_pretty(&result).unwrap());
            }
        }
    }

    // Cleanup temp body file
    if !save_body {
        if let Some(ref path) = body_path {
            if is_debug {
                eprintln!("rm body file {}", path);
            }
            let _ = fs::remove_file(path);
        }
    }

    exit_code
}

fn main() {
    let code = run();
    process::exit(code);
}
