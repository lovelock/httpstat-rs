# httpstat-rs

Rust rewrite of [httpstat](https://github.com/reorx/httpstat) — curl statistics made simple.

Solves the cold start problem of the Python version: no subprocess fork/exec, calls libcurl in-process via FFI. Self-contained binary with no runtime dependencies.

## Features

- **Beautiful terminal output** — timing breakdown of DNS, TCP, TLS, server processing, and content transfer
- **Structured JSON/JSONL output** — `--format json` / `jsonl` with schema_version=1
- **SLO threshold checking** — `--slo total=500,connect=100` exits with code 4 on violation
- **Save results to file** — `--save path.json` for multi-step workflows
- **NO_COLOR support** — respects the [NO_COLOR](https://no-color.org) convention

## Install

```bash
cargo install --git https://github.com/lovelock/httpstat-rs
```

Or build from source:

```bash
git clone https://github.com/lovelock/httpstat-rs
cd httpstat-rs
cargo build --release
# binary at target/release/httpstat
```

## Usage

```bash
httpstat https://example.com
```

### cURL Options

Pass any curl-supported option after the URL (except `-w`, `-D`, `-o`, `-s`, `-S` which are used internally):

```bash
httpstat https://example.com -X POST -H "Content-Type: application/json" -d '{"a":1}'
```

### Structured Output

```bash
httpstat https://example.com --format json
httpstat https://example.com --format jsonl
```

JSON output:

```json
{
  "schema_version": 1,
  "url": "https://example.com",
  "ok": true,
  "exit_code": 0,
  "response": {
    "status_line": "HTTP/2 200",
    "status_code": 200,
    "remote_ip": "93.184.216.34",
    "remote_port": "443",
    "headers": { "Content-Type": "text/html" }
  },
  "timings_ms": {
    "dns": 5, "connect": 10, "tls": 15,
    "server": 50, "transfer": 20, "total": 100,
    "namelookup": 5, "initial_connect": 15,
    "pretransfer": 30, "starttransfer": 80
  },
  "speed": { "download_kbs": 1234.5, "upload_kbs": 0.0 },
  "slo": null
}
```

### SLO Thresholds

Check response times against thresholds. Exits with code `4` on violation:

```bash
httpstat https://example.com --slo total=500,connect=100,ttfb=200
```

Supported keys: `total`, `connect`, `ttfb` (time to first byte), `dns`, `tls`.

### Save Results

```bash
httpstat https://example.com --save result.json
httpstat https://example.com --format json --save result.json
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HTTPSTAT_SHOW_BODY` | `false` | Show response body (limited to 1024 bytes) |
| `HTTPSTAT_SHOW_IP` | `true` | Show remote/local IP and port |
| `HTTPSTAT_SHOW_SPEED` | `false` | Show download/upload speed |
| `HTTPSTAT_SAVE_BODY` | `true` | Save body to temp file |
| `HTTPSTAT_DEBUG` | `false` | Show debug logs |
| `HTTPSTAT_METRICS_ONLY` | `false` | Equivalent to `--format json` (backward compat) |
| `NO_COLOR` | — | Disable ANSI color output |

```bash
HTTPSTAT_SHOW_SPEED=true httpstat https://example.com
NO_COLOR=1 httpstat https://example.com
```

## License

MIT
