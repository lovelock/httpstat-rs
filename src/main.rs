mod output;
mod timing;

use std::env;
use std::fs;
use std::io::Write;
use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 从 args 列表中移除指定 flag（及其值），返回值或 None
fn pop_arg(args: &mut Vec<String>, flag: &str, has_value: bool) -> Option<String> {
    if let Some(idx) = args.iter().position(|a| a == flag) {
        args.remove(idx);
        if has_value {
            if idx < args.len() {
                Some(args.remove(idx))
            } else {
                None
            }
        } else {
            Some(String::new())
        }
    } else {
        None
    }
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

fn print_help() {
    println!(concat!(
        "Usage: httpstat URL [CURL_OPTIONS]\n",
        "       httpstat -h | --help\n",
        "       httpstat --version\n",
        "\n",
        "Arguments:\n",
        "  URL     url to request, could be with or without `http(s)://` prefix\n",
        "\n",
        "Options:\n",
        "  CURL_OPTIONS  any curl supported options, except for -w -D -o -S -s,\n",
        "                which are already used internally.\n",
        "  -h --help     show this screen.\n",
        "  --version     show version.\n",
        "  --format      output format: pretty, json, jsonl. Default is `pretty`.\n",
        "  --slo         SLO thresholds as key=value pairs, e.g. `total=500,connect=100`.\n",
        "                Valid keys: total, connect, ttfb, dns, tls.\n",
        "                Exits with code 4 on violation.\n",
        "  --save        save structured output to a file path.\n",
        "\n",
        "Environments:\n",
        "  HTTPSTAT_SHOW_BODY    Set to `true` to show response body. Default is `false`.\n",
        "  HTTPSTAT_SHOW_IP      Set to `false` to disable IP display. Default is `true`.\n",
        "  HTTPSTAT_SHOW_SPEED   Set to `true` to show speed. Default is `false`.\n",
        "  HTTPSTAT_SAVE_BODY    Set to `false` to disable body file. Default is `true`.\n",
        "  HTTPSTAT_CURL_BIN     curl binary path. Default is `curl` from $PATH.\n",
        "  HTTPSTAT_METRICS_ONLY Set to `true` to force JSON output. Default is `false`.\n",
        "  HTTPSTAT_DEBUG        Set to `true` for debug logs. Default is `false`.\n",
        "  NO_COLOR              Disable colored output.",
    ));
}

const CURL_FORMAT: &str = r#"{
"time_namelookup": %{time_namelookup},
"time_connect": %{time_connect},
"time_appconnect": %{time_appconnect},
"time_pretransfer": %{time_pretransfer},
"time_redirect": %{time_redirect},
"time_starttransfer": %{time_starttransfer},
"time_total": %{time_total},
"speed_download": %{speed_download},
"speed_upload": %{speed_upload},
"remote_ip": "%{remote_ip}",
"remote_port": "%{remote_port}",
"local_ip": "%{local_ip}",
"local_port": "%{local_port}"
}"#;

fn run() -> i32 {
    let mut args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_help();
        return 0;
    }

    // pop httpstat-specific flags
    let mut output_format = pop_arg(&mut args, "--format", true)
        .unwrap_or_else(|| "pretty".to_string());
    let slo_spec = pop_arg(&mut args, "--slo", true);
    let save_path = pop_arg(&mut args, "--save", true);

    // check help/version on first remaining arg
    if let Some(first) = args.first() {
        if first == "-h" || first == "--help" {
            print_help();
            return 0;
        }
        if first == "--version" {
            println!("httpstat {}", VERSION);
            return 0;
        }
    }

    // backward compat
    let metrics_only = env_bool("HTTPSTAT_METRICS_ONLY", false);
    if metrics_only && output_format == "pretty" {
        output_format = "json".to_string();
    }

    // validate format
    if !matches!(output_format.as_str(), "pretty" | "json" | "jsonl") {
        eprintln!("Error: invalid format \"{}\", must be pretty, json, or jsonl", output_format);
        return 1;
    }

    // parse SLO
    let slo = slo_spec.as_ref().map(|s| timing::parse_slo(s)).transpose();
    let slo = match slo {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };

    // envs
    let show_body = env_bool("HTTPSTAT_SHOW_BODY", false);
    let show_ip = env_bool("HTTPSTAT_SHOW_IP", true);
    let show_speed = env_bool("HTTPSTAT_SHOW_SPEED", false);
    let save_body = env_bool("HTTPSTAT_SAVE_BODY", true);
    let is_debug = env_bool("HTTPSTAT_DEBUG", false);
    let curl_bin = env::var("HTTPSTAT_CURL_BIN").unwrap_or_else(|_| "curl".to_string());
    let use_color = env::var("NO_COLOR").is_err() && atty_is_tty();

    // url = first remaining arg
    if args.is_empty() {
        eprintln!("Error: URL is required");
        return 1;
    }
    let url = args.remove(0);
    let curl_args = args; // everything else

    // validate curl args — exclude options httpstat uses internally
    let exclude_options = ["-w", "--write-out", "-D", "--dump-header", "-o", "--output", "-s", "-S", "--silent"];
    for arg in &curl_args {
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

    // create temp files for headers and body
    let header_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
    let body_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
    let header_path = header_file.into_temp_path();
    let body_path = body_file.into_temp_path();

    // build curl command
    let mut cmd = process::Command::new(&curl_bin);
    cmd.arg("-w").arg(CURL_FORMAT);
    cmd.arg("-D").arg(&header_path);
    cmd.arg("-o").arg(&body_path);
    cmd.arg("-s").arg("-S");
    for arg in &curl_args {
        cmd.arg(arg);
    }
    cmd.arg(&url);

    if is_debug {
        eprintln!("cmd: {:?}", cmd);
    }

    // execute curl
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("httpstat: failed to execute curl: {}", e);
            return 1;
        }
    };

    // handle curl error
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("curl error: {}", stderr);
        return output.status.code().unwrap_or(1);
    }

    // parse timing JSON from stdout
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut timings: timing::CurlTimings = match serde_json::from_str(&stdout) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Could not decode json: {}", e);
            eprintln!("curl result: {}", stdout);
            return 1;
        }
    };

    // read headers
    let headers_text = fs::read_to_string(&header_path).unwrap_or_default();
    let headers_text = headers_text.trim().to_string();

    // read body
    let body_bytes = fs::read(&body_path).unwrap_or_default();
    let body_text = String::from_utf8_lossy(&body_bytes).to_string();
    let body_total_len = body_bytes.len();

    // compute download speed if not provided
    if timings.speed_download == 0.0 && timings.time_total > 0.0 {
        timings.speed_download = body_total_len as f64 / timings.time_total;
    }

    // persist body temp file if saving, or clean it up
    let body_file_path = if save_body {
        body_path.keep().ok().map(|p| p.to_string_lossy().to_string())
    } else {
        drop(body_path);
        None
    };

    // check SLO
    let slo_result = slo.as_ref().map(|s| timing::check_slo(s, &timings));
    let mut exit_code = 0;
    if let Some((pass, _)) = &slo_result {
        if !pass {
            exit_code = 4;
        }
    }

    // --- output ---
    if output_format == "json" || output_format == "jsonl" {
        let result = timing::build_json_result(
            &url, &timings, &headers_text, slo_result, exit_code,
        );
        let output_text = if output_format == "json" {
            serde_json::to_string_pretty(&result).unwrap()
        } else {
            serde_json::to_string(&result).unwrap()
        };
        println!("{}", output_text);

        if let Some(ref path) = save_path {
            if let Ok(mut f) = fs::File::create(path) {
                if let Err(e) = writeln!(f, "{}", output_text) {
                    eprintln!("Warning: failed to write to {}: {}", path, e);
                }
            }
        }
        return exit_code;
    }

    // --- pretty mode ---
    let pretty_opts = output::PrettyOptions {
        show_body,
        show_ip,
        show_speed,
        save_body,
        body_path: body_file_path.clone(),
        body_content: Some(body_text),
        body_total_len: Some(body_total_len),
    };

    output::print_pretty(&timings, &headers_text, &pretty_opts, use_color);

    if let Some((false, ref violations)) = slo_result {
        output::print_slo_violations(violations, use_color);
    }

    // save pretty output as json
    if let Some(ref path) = save_path {
        let result = timing::build_json_result(
            &url, &timings, &headers_text, slo_result, exit_code,
        );
        if let Ok(mut f) = fs::File::create(path) {
            if let Err(e) = writeln!(f, "{}", serde_json::to_string_pretty(&result).unwrap()) {
                eprintln!("Warning: failed to write to {}: {}", path, e);
            }
        }
    }

    exit_code
}

fn main() {
    let code = run();
    process::exit(code);
}
