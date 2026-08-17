mod output;
mod timing;

use clap::Parser;
use curl::easy::Easy;
use std::env;
use std::process;
use std::sync::{Arc, Mutex};

use timing::CurlTimings;

#[derive(Parser)]
#[command(name = "httpstat", version, about = "curl statistics made simple")]
struct Cli {
    /// URL to request
    url: String,
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

fn run() -> i32 {
    let cli = Cli::parse();
    let url = normalize_url(&cli.url);

    let use_color = env::var("NO_COLOR").is_err() && atty_is_tty();

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
    };

    // Parse status line from headers
    let headers_guard = headers_raw.lock().unwrap();
    let headers_text = String::from_utf8_lossy(&headers_guard);
    let status_line = headers_text
        .lines()
        .next()
        .unwrap_or("HTTP/1.1 000")
        .trim()
        .to_string();

    output::print_pretty(&timings, &status_line, use_color);
    0
}

fn atty_is_tty() -> bool {
    unsafe { libc_isatty(1) != 0 }
}

extern "C" {
    #[link_name = "isatty"]
    fn libc_isatty(fd: i32) -> i32;
}

fn main() {
    let code = run();
    process::exit(code);
}
