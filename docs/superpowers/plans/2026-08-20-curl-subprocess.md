# curl subprocess 方案实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 httpstat 从 libcurl 直接调用改为 curl 子进程模式，与 Python 版行为完全一致，支持所有 curl 选项。

**Architecture:** 手动解析 `sys.argv` 提取 httpstat 自有参数（`-f`/`--format`、`--slo`、`--save`），其余全部作为 curl 参数透传。构造 curl 命令时注入 `-w`（timing JSON）、`-D`（header 临时文件）、`-o`（body 临时文件）、`-s`、`-S`，解析 curl 的 `-w` JSON 输出获取 timing 指标，读取临时文件获取 header 和 body。

**Tech Stack:** Rust, serde_json, std::process::Command, tempfile

---

## File Structure

- **Modify:** `Cargo.toml` — 移除 `clap` 和 `curl` 依赖，添加 `tempfile`
- **Rewrite:** `src/main.rs` — 手动 arg 解析 + curl 子进程执行
- **Modify:** `src/timing.rs` — `CurlTimings` 改为从 curl `-w` JSON 反序列化
- **Keep:** `src/output.rs` — 无需改动

---

### Task 1: 更新 Cargo.toml 依赖

**Files:**
- Modify: `Cargo.toml`

**Steps:**

- [ ] **Step 1: 修改 Cargo.toml**

移除 `clap` 和 `curl`，添加 `tempfile`：

```toml
[package]
name = "httpstat"
version = "0.1.0"
edition = "2021"
description = "curl statistics made simple — Rust rewrite"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check`
Expected: 编译通过（main.rs 会有编译错误，下一步修复）

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: replace clap+curl with tempfile for subprocess approach"
```

---

### Task 2: 改写 timing.rs — 从 curl JSON 反序列化

**Files:**
- Modify: `src/timing.rs`

**接口变更:**
- `CurlTimings` 新增 `#[derive(Deserialize)]`，字段名改为与 curl `-w` JSON 输出一致（`time_namelookup` 等）
- 保留 `range_*()` 方法和 `is_https()`，内部实现改为读新字段名
- `speed_download` / `speed_upload` 从 curl 输出获取（单位 bytes/sec）

**Steps:**

- [ ] **Step 1: 改写 CurlTimings 结构体**

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CurlTimings {
    pub time_namelookup: f64,
    pub time_connect: f64,
    pub time_appconnect: f64,
    pub time_pretransfer: f64,
    pub time_redirect: f64,
    pub time_starttransfer: f64,
    pub time_total: f64,
    pub speed_download: f64,
    pub speed_upload: f64,
    pub remote_ip: String,
    pub remote_port: String,
    pub local_ip: String,
    pub local_port: String,
}

impl CurlTimings {
    pub fn range_dns(&self) -> f64 {
        self.time_namelookup
    }

    pub fn range_connect(&self) -> f64 {
        self.time_connect - self.time_namelookup
    }

    pub fn range_tls(&self) -> f64 {
        self.time_pretransfer - self.time_connect
    }

    pub fn range_server(&self) -> f64 {
        self.time_starttransfer - self.time_pretransfer
    }

    pub fn range_transfer(&self) -> f64 {
        self.time_total - self.time_starttransfer
    }

    pub fn is_https(&self) -> bool {
        self.time_appconnect > 0.0
    }
}
```

- [ ] **Step 2: 更新 check_slo 中的字段引用**

`check_slo` 函数中 `match timing_key` 分支改为读 `time_*` 字段：

```rust
let actual = match timing_key {
    "time_total" => timings.time_total,
    "time_connect" => timings.time_connect,
    "time_starttransfer" => timings.time_starttransfer,
    "time_namelookup" => timings.time_namelookup,
    "time_pretransfer" => timings.time_pretransfer,
    _ => 0.0,
};
```

- [ ] **Step 3: 更新 build_json_result 中的字段引用**

所有 `t.namelookup` → `t.time_namelookup`，`t.connect` → `t.time_connect`，依此类推。

- [ ] **Step 4: 更新测试中的 make_timings()**

```rust
fn make_timings() -> CurlTimings {
    CurlTimings {
        time_namelookup: 0.005,
        time_connect: 0.015,
        time_appconnect: 0.030,
        time_pretransfer: 0.030,
        time_redirect: 0.0,
        time_starttransfer: 0.080,
        time_total: 0.100,
        speed_download: 10240.0,
        speed_upload: 0.0,
        remote_ip: "93.184.216.34".to_string(),
        remote_port: "443".to_string(),
        local_ip: "192.168.1.1".to_string(),
        local_port: "54321".to_string(),
    }
}
```

- [ ] **Step 5: 运行测试**

Run: `cargo test`
Expected: 所有 timing 相关测试通过

- [ ] **Step 6: Commit**

```bash
git add src/timing.rs
git commit -m "refactor: CurlTimings now deserializes from curl -w JSON output"
```

---

### Task 3: 重写 main.rs — 手动 arg 解析 + curl 子进程

**Files:**
- Rewrite: `src/main.rs`

这是核心改动。完全重写 `run()` 函数。

**接口:**
- 输入: `std::env::args()`
- 输出: exit code (i32)
- 依赖: `timing::CurlTimings`（Task 2 产出）、`output::print_pretty`（不变）

**curl `-w` JSON 格式（与 Python 版一致）:**

```json
{
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
}
```

**Steps:**

- [ ] **Step 1: 写 pop_arg 辅助函数**

```rust
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
```

- [ ] **Step 2: 写 env_bool 和 atty_is_tty（保留现有实现）**

```rust
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
```

- [ ] **Step 3: 写 print_help 和 print_version**

```rust
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
        "  -f --format   output format: pretty, json, jsonl. Default is `pretty`.\n",
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
```

- [ ] **Step 4: 写 curl_format 常量**

```rust
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
```

- [ ] **Step 5: 写 run() 主函数**

核心逻辑：

```rust
fn run() -> i32 {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        print_help();
        return 0;
    }

    // pop httpstat-specific flags
    let mut output_format = pop_arg(&mut args, "--format", true)
        .or_else(|| pop_arg(&mut args, "-f", true))
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
    let curl_args = args;  // everything else

    // validate curl args — exclude options httpstat uses internally
    let exclude_options = ["-w", "--write-out", "-D", "--dump-header", "-o", "--output", "-s", "--silent"];
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
    let mut cmd = std::process::Command::new(&curl_bin);
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

    // save body to temp file if needed
    let body_file_path = if save_body {
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
                let _ = writeln!(f, "{}", output_text);
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

    output::print_pretty(&timings, "", &headers_text, &pretty_opts, use_color);

    if let Some((false, ref violations)) = slo_result {
        output::print_slo_violations(violations, use_color);
    }

    // save pretty output as json
    if save_path.is_some() {
        let result = timing::build_json_result(
            &url, &timings, &headers_text, slo_result, exit_code,
        );
        if let Some(ref path) = save_path {
            if let Ok(mut f) = fs::File::create(path) {
                let _ = writeln!(f, "{}", serde_json::to_string_pretty(&result).unwrap());
            }
        }
    }

    // cleanup body file
    if !save_body {
        if let Some(ref path) = body_file_path {
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
    std::process::exit(code);
}
```

- [ ] **Step 6: 编译验证**

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat: switch to curl subprocess, support all curl options"
```

---

### Task 4: 端到端测试

**Steps:**

- [ ] **Step 1: 基本 GET 请求**

Run: `cargo run -- "https://httpbin.org/get"`
Expected: 正常输出 timing diagram

- [ ] **Step 2: -L 参数（follow redirect）**

Run: `cargo run -- "http://github.com" -L`
Expected: 跟随重定向，最终显示 200

- [ ] **Step 3: -X POST 参数**

Run: `cargo run -- "https://httpbin.org/post" -X POST`
Expected: 返回 POST echo 响应

- [ ] **Step 4: -H 自定义 header**

Run: `cargo run -- "https://httpbin.org/headers" -H "X-Custom: test"`
Expected: 响应中包含自定义 header

- [ ] **Step 5: curl 参数在 URL 前后都支持**

Run: `cargo run -- -L -X POST "https://httpbin.org/post"`
Run: `cargo run -- "https://httpbin.org/post" -L -X POST`
Expected: 两种顺序都能正常工作

- [ ] **Step 6: JSON 输出**

Run: `cargo run -- -f json "https://httpbin.org/get"`
Expected: 输出合法 JSON

- [ ] **Step 7: 排除选项检查**

Run: `cargo run -- "https://httpbin.org/get" -w "%{time_total}"`
Expected: 报错 `-w is not allowed`

- [ ] **Step 8: 运行单元测试**

Run: `cargo test`
Expected: 所有测试通过

- [ ] **Step 9: 最终 Commit（如有修复）**

```bash
git add -A && git commit -m "fix: address issues found during e2e testing"
```
