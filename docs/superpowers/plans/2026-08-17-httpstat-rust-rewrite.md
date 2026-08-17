# httpstat Rust Rewrite — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that visualizes curl HTTP timing statistics, solving the cold start problem of the Python original.

**Architecture:** Single-process libcurl FFI via the `curl` crate (vendored). Three source modules: `timing` (metrics + range calculations), `output` (ANSI color timing diagram), `main` (CLI + curl orchestration). No async runtime.

**Tech Stack:** Rust 2021 edition, `curl` 0.4 (vendored), `clap` 4 (derive)

## Global Constraints

- Edition 2021, MSRV 1.70
- `curl` crate with `vendored` feature (self-contained binary)
- `clap` 4 with `derive` feature
- No async runtime (tokio, async-std)
- `NO_COLOR` env var disables ANSI output
- URL without `http://`/`https://` prefix gets `http://` prepended

---

## File Structure

```
src/
  timing.rs     — CurlTimings struct, from_curl() constructor, range calculations
  output.rs     — print_pretty() with ANSI color timing diagram
  main.rs       — CLI (clap), curl orchestration, entry point
```

---

### Task 1: Project scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs` (stub)
- Create: `src/timing.rs` (empty)
- Create: `src/output.rs` (empty)

- [ ] **Step 1: Initialize cargo project**

Run: `cargo init --name httpstat /home/frost/dev/oss/statrs`

- [ ] **Step 2: Write Cargo.toml**

Replace generated `Cargo.toml` with:

```toml
[package]
name = "httpstat"
version = "0.1.0"
edition = "2021"
description = "curl statistics made simple — Rust rewrite"

[dependencies]
curl = { version = "0.4", features = ["vendored"] }
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 3: Create module stubs**

Replace `src/main.rs` with:

```rust
mod output;
mod timing;

fn main() {
    println!("httpstat — not yet implemented");
}
```

Create `src/timing.rs`:

```rust
```

Create `src/output.rs`:

```rust
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: compiles with vendored libcurl (may take ~30s first time)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "scaffold: init project with curl and clap deps"
```

---

### Task 2: Timing module

**Files:**
- Modify: `src/timing.rs`
- Test: (inline `#[cfg(test)]` module)

**Interfaces:**
- Produces: `CurlTimings` struct with `from_curl()` and range methods

- [ ] **Step 1: Write tests**

Add to `src/timing.rs`:

```rust
pub struct CurlTimings {
    pub namelookup: f64,
    pub connect: f64,
    pub appconnect: f64,
    pub pretransfer: f64,
    pub starttransfer: f64,
    pub total: f64,
}

impl CurlTimings {
    pub fn range_dns(&self) -> f64 {
        self.namelookup
    }

    pub fn range_connect(&self) -> f64 {
        self.connect - self.namelookup
    }

    pub fn range_tls(&self) -> f64 {
        self.pretransfer - self.connect
    }

    pub fn range_server(&self) -> f64 {
        self.starttransfer - self.pretransfer
    }

    pub fn range_transfer(&self) -> f64 {
        self.total - self.starttransfer
    }

    pub fn is_https(&self) -> bool {
        self.appconnect > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_timings() -> CurlTimings {
        CurlTimings {
            namelookup: 0.005,
            connect: 0.015,
            appconnect: 0.030,
            pretransfer: 0.030,
            starttransfer: 0.080,
            total: 0.100,
        }
    }

    #[test]
    fn test_range_dns() {
        let t = make_timings();
        assert!((t.range_dns() - 0.005).abs() < 1e-9);
    }

    #[test]
    fn test_range_connect() {
        let t = make_timings();
        assert!((t.range_connect() - 0.010).abs() < 1e-9);
    }

    #[test]
    fn test_range_tls() {
        let t = make_timings();
        assert!((t.range_tls() - 0.015).abs() < 1e-9);
    }

    #[test]
    fn test_range_server() {
        let t = make_timings();
        assert!((t.range_server() - 0.050).abs() < 1e-9);
    }

    #[test]
    fn test_range_transfer() {
        let t = make_timings();
        assert!((t.range_transfer() - 0.020).abs() < 1e-9);
    }

    #[test]
    fn test_is_https_true() {
        let t = make_timings();
        assert!(t.is_https());
    }

    #[test]
    fn test_is_https_false() {
        let mut t = make_timings();
        t.appconnect = 0.0;
        assert!(!t.is_https());
    }

    #[test]
    fn test_ranges_sum_to_total() {
        let t = make_timings();
        let sum = t.range_dns() + t.range_connect() + t.range_tls() + t.range_server() + t.range_transfer();
        assert!((sum - t.total).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib timing`
Expected: 8 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/timing.rs && git commit -m "feat(timing): CurlTimings struct with range calculations"
```

---

### Task 3: Output module

**Files:**
- Modify: `src/output.rs`

**Interfaces:**
- Consumes: `CurlTimings` (from Task 2)
- Produces: `print_pretty()` function that writes formatted output to stdout

- [ ] **Step 1: Implement output.rs**

Replace `src/output.rs` with:

```rust
use crate::timing::CurlTimings;

const RESET: &str = "\x1b[0m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[38;5;242m";

fn colorize(s: &str, color: &str, use_color: bool) -> String {
    if use_color {
        format!("{}{}{}", color, s, RESET)
    } else {
        s.to_string()
    }
}

fn fmta(ms: i64, use_color: bool) -> String {
    let s = format!("{:>5}ms", ms);
    colorize(&s, CYAN, use_color)
}

fn fmtb(ms: i64, use_color: bool) -> String {
    let s = format!("{:<5}ms", ms);
    colorize(&s, CYAN, use_color)
}

pub fn print_pretty(t: &CurlTimings, status_line: &str, use_color: bool) {
    let dns = (t.range_dns() * 1000.0) as i64;
    let connect = (t.range_connect() * 1000.0) as i64;
    let tls = (t.range_tls() * 1000.0) as i64;
    let server = (t.range_server() * 1000.0) as i64;
    let transfer = (t.range_transfer() * 1000.0) as i64;
    let total_ms = (t.total * 1000.0) as i64;
    let namelookup_ms = (t.namelookup * 1000.0) as i64;
    let connect_ms = (t.connect * 1000.0) as i64;
    let pretransfer_ms = (t.pretransfer * 1000.0) as i64;
    let starttransfer_ms = (t.starttransfer * 1000.0) as i64;

    // status line
    let parts: Vec<&str> = status_line.splitn(2, '/').collect();
    if parts.len() == 2 {
        println!(
            "{}{}{}{}",
            colorize(parts[0], GREEN, use_color),
            colorize("/", GRAY, use_color),
            colorize(parts[1], CYAN, use_color),
            ""
        );
    } else {
        println!("{}", colorize(status_line, GREEN, use_color));
    }
    println!();

    if t.is_https() {
        print_https(dns, connect, tls, server, transfer, total_ms, namelookup_ms, connect_ms, pretransfer_ms, starttransfer_ms, use_color);
    } else {
        print_http(dns, connect, server, transfer, total_ms, namelookup_ms, connect_ms, starttransfer_ms, use_color);
    }
}

fn print_https(
    dns: i64, connect: i64, tls: i64, server: i64, transfer: i64,
    total_ms: i64, namelookup_ms: i64, connect_ms: i64, pretransfer_ms: i64, starttransfer_ms: i64,
    use_color: bool,
) {
    let gray = |s: &str| colorize(s, GRAY, use_color);

    println!("  DNS Lookup   TCP Connection   TLS Handshake   Server Processing   Content Transfer");
    println!(
        "[{} | {} | {} | {} | {} ]",
        fmta(dns, use_color),
        fmta(connect, use_color),
        fmta(tls, use_color),
        fmta(server, use_color),
        fmta(transfer, use_color),
    );
    println!(
        " {}|{} {}|{} {}|{} {}|{} {}|{}",
        gray("          "), gray(""),
        gray("             "), gray(""),
        gray("              "), gray(""),
        gray("                   "), gray(""),
        gray("                  "), gray(""),
    );
    println!(
        "  {}{}          {}|{}                   {}|{}                   {}|{}",
        gray("namelookup:"), fmtb(namelookup_ms, use_color),
        gray(""), gray(""),
        gray(""), gray(""),
        gray(""), gray(""),
    );
    println!(
        "                   {}{}          {}|{}                   {}|{}",
        gray("connect:"), fmtb(connect_ms, use_color),
        gray(""), gray(""),
        gray(""), gray(""),
    );
    println!(
        "                                {}{}           {}|{}",
        gray("pretransfer:"), fmtb(pretransfer_ms, use_color),
        gray(""), gray(""),
    );
    println!(
        "                                                    {}{}         |{}",
        gray("starttransfer:"), fmtb(starttransfer_ms, use_color),
        gray(""),
    );
    println!(
        "                                                                              {}{}",
        gray("total:"),
        fmtb(total_ms, use_color),
    );
}

fn print_http(
    dns: i64, connect: i64, server: i64, transfer: i64,
    total_ms: i64, namelookup_ms: i64, connect_ms: i64, starttransfer_ms: i64,
    use_color: bool,
) {
    let gray = |s: &str| colorize(s, GRAY, use_color);

    println!("  DNS Lookup   TCP Connection   Server Processing   Content Transfer");
    println!(
        "[{} | {} | {} | {} ]",
        fmta(dns, use_color),
        fmta(connect, use_color),
        fmta(server, use_color),
        fmta(transfer, use_color),
    );
    println!(
        " {}|{} {}|{} {}|{} {}|{}",
        gray("          "), gray(""),
        gray("             "), gray(""),
        gray("                   "), gray(""),
        gray("                  "), gray(""),
    );
    println!(
        "  {}{}          {}|{}                   {}|{}",
        gray("namelookup:"), fmtb(namelookup_ms, use_color),
        gray(""), gray(""),
        gray(""), gray(""),
    );
    println!(
        "                   {}{}               {}|{}",
        gray("connect:"), fmtb(connect_ms, use_color),
        gray(""), gray(""),
    );
    println!(
        "                                    {}{}           |{}",
        gray("starttransfer:"), fmtb(starttransfer_ms, use_color),
        gray(""),
    );
    println!(
        "                                                             {}{}",
        gray("total:"),
        fmtb(total_ms, use_color),
    );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: compiles (main.rs still stub, but module links)

- [ ] **Step 3: Commit**

```bash
git add src/output.rs && git commit -m "feat(output): pretty ANSI timing diagram for HTTP and HTTPS"
```

---

### Task 4: Main — CLI + curl orchestration

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `CurlTimings::from_curl()` (Task 2), `print_pretty()` (Task 3)

- [ ] **Step 1: Implement main.rs**

Replace `src/main.rs` with:

```rust
mod output;
mod timing;

use clap::Parser;
use curl::easy::Easy;
use std::env;
use std::process;

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

fn run() -> i32 {
    let cli = Cli::parse();
    let url = normalize_url(&cli.url);

    let use_color = env::var("NO_COLOR").is_err() && atty_is_tty();

    let mut easy = Easy::new();
    easy.url(&url).unwrap_or_else(|e| {
        eprintln!("httpstat: invalid URL: {}", e);
        process::exit(1);
    });
    easy.follow_location(true);
    easy.max_redirections(10);
    easy.nosignal(true);

    // Collect response headers
    let mut headers_raw = Vec::new();
    {
        let mut header_list = easy.header_function(|header| {
            headers_raw.extend_from_slice(header);
            true
        }).unwrap();

        if let Err(e) = easy.perform() {
            eprintln!("httpstat: curl error: {}", e);
            return easy.errno().unwrap_or(1) as i32;
        }
    }

    // Collect timing metrics
    let timings = CurlTimings {
        namelookup: easy.get_info(curl::easy::Info::NameLookupTime).unwrap_or(0.0),
        connect: easy.get_info(curl::easy::Info::ConnectTime).unwrap_or(0.0),
        appconnect: easy.get_info(curl::easy::Info::AppConnectTime).unwrap_or(0.0),
        pretransfer: easy.get_info(curl::easy::Info::PreTransferTime).unwrap_or(0.0),
        starttransfer: easy.get_info(curl::easy::Info::StartTransferTime).unwrap_or(0.0),
        total: easy.get_info(curl::easy::Info::ResponseTime).unwrap_or(0.0),
    };

    // Parse status line from headers
    let headers_text = String::from_utf8_lossy(&headers_raw);
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: compiles successfully

- [ ] **Step 3: Manual test — HTTP**

Run: `cargo run -- http://httpbin.org/get`
Expected: timing diagram printed with DNS, TCP, server, transfer columns

- [ ] **Step 4: Manual test — HTTPS**

Run: `cargo run -- https://http2.akamai.com`
Expected: timing diagram with TLS handshake column added

- [ ] **Step 5: Manual test — NO_COLOR**

Run: `NO_COLOR=1 cargo run -- https://http2.akamai.com`
Expected: same output but no ANSI escape sequences

- [ ] **Step 6: Manual test — missing URL**

Run: `cargo run`
Expected: clap prints help and exits

- [ ] **Step 7: Manual test — URL without prefix**

Run: `cargo run -- httpbin.org/get`
Expected: works (prepends `http://`)

- [ ] **Step 8: Commit**

```bash
git add src/main.rs && git commit -m "feat(main): CLI parsing, curl orchestration, timing collection"
```

---

### Task 5: Run all tests + final build

- [ ] **Step 1: Run unit tests**

Run: `cargo test`
Expected: all 8 timing tests pass

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`
Expected: compiles, binary at `target/release/httpstat`

- [ ] **Step 3: Manual E2E — compare with Python version**

Run both and compare timing values:
```bash
python ../httpstat/httpstat.py https://http2.akamai.com
cargo run --release -- https://http2.akamai.com
```
Expected: similar timing breakdown (values may differ slightly due to measurement approach)

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: verify tests pass and release build works"
```
