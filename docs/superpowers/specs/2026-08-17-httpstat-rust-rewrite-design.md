# httpstat Rust Rewrite — MVP Design

## Overview

Rewrite [httpstat](https://github.com/reorx/httpstat) (Python CLI, ~574 lines) in Rust as a minimal MVP. The primary motivation is solving the cold start problem — the Python version spawns `curl` as a subprocess for each request, adding process startup overhead. The Rust version calls libcurl in-process via FFI.

## Goals

- **Fast cold start**: no subprocess fork/exec, direct libcurl FFI calls
- **Self-contained binary**: no runtime dependencies (libcurl vendored at build time)
- **Feature scope**: MVP — pretty timing visualization only. No JSON output, no SLO, no `--save`, no env vars beyond `NO_COLOR`

## Non-Goals (MVP)

- `--format json` / `--format jsonl` output
- `--slo` threshold checking
- `--save` file output
- Environment variables beyond `NO_COLOR`
- Extra curl flags passthrough
- HTTP/2 specific timing (appconnect may report TLS timing for HTTP/2)

## Architecture

```
src/
  main.rs       — CLI entry, curl orchestration, timing collection
  timing.rs     — Timing struct, range calculations, HTTP/HTTPS awareness
  output.rs     — Pretty terminal output with ANSI colors
```

### Dependencies

| Crate | Purpose | Notes |
|-------|---------|-------|
| `curl` | HTTP client (libcurl FFI) | `vendored` feature for self-contained builds |
| `clap` | CLI argument parsing | Derive API |

No async runtime (tokio, async-std). Blocking API only.

## Core Flow

1. Parse URL from CLI args via `clap`
2. Normalize URL: if no `http://` or `https://` prefix, prepend `http://` (matching curl CLI behavior)
3. Create `curl::easy::Easy`, configure: set URL, follow redirects (`CURLOPT_FOLLOWLOCATION`), disable signal handling (`CURLOPT_NOSIGNAL`)
3. Perform request via `easy.perform()`
4. Collect timing metrics from `easy.get_info()`:
   - `time_namelookup` — DNS resolution time (seconds, convert to ms)
   - `time_connect` — TCP connection established
   - `time_appconnect` — TLS handshake complete (HTTPS only)
   - `time_pretransfer` — request ready to send
   - `time_starttransfer` — first byte of response (TTFB)
   - `time_total` — total elapsed time
5. Compute derived ranges:
   - `dns = time_namelookup`
   - `connect = time_connect - time_namelookup`
   - `tls = time_pretransfer - time_connect` (HTTPS only; for HTTP, TLS phase is absent)
   - `server = time_starttransfer - time_pretransfer`
   - `transfer = time_total - time_starttransfer`
6. Print timing diagram to stdout

## Timing Data

All metrics come directly from libcurl via `curl::easy::InfoType`. No manual instrumentation needed. Values are in seconds (float), converted to integer milliseconds for display.

### Derived Ranges

```
dns        = time_namelookup
connect    = time_connect - time_namelookup
tls        = time_pretransfer - time_connect         (HTTPS only)
server     = time_starttransfer - time_pretransfer
transfer   = time_total - time_starttransfer
```

## Output Format

### HTTPS (5 columns)

```
  DNS Lookup   TCP Connection   TLS Handshake   Server Processing   Content Transfer
[    5ms   |     10ms     |     15ms     |       50ms        |       20ms      ]
           |               |              |                   |                  |
  namelookup:5ms          |              |                   |                  |
                   connect:15ms          |                   |                  |
                                pretransfer:30ms             |                  |
                                                    starttransfer:80ms         |
                                                                              total:100ms
```

### HTTP (4 columns, no TLS)

```
  DNS Lookup   TCP Connection   Server Processing   Content Transfer
[    5ms   |     10ms     |       50ms        |       20ms      ]
           |               |                   |                  |
  namelookup:5ms          |                   |                  |
                   connect:15ms               |                  |
                                    starttransfer:65ms           |
                                                             total:85ms
```

### Color Scheme

- Timing values (ms): cyan `\x1b[36m`
- Frame lines (brackets, pipes): grayscale `\x1b[38;5;242m`
- Status line: green for protocol, cyan for status code
- Headers: grayscale for key, cyan for value
- Disabled when `NO_COLOR` env var is set (any value)

## CLI Interface

```
Usage: httpstat <URL>
       httpstat -h | --help
       httpstat --version
```

### Arguments

- `URL` — target URL, with or without `http://` / `https://` prefix

### Flags

- `-h`, `--help` — print help
- `--version` — print version (e.g., `httpstat 0.1.0`, from `CARGO_PKG_VERSION`)

### Environment Variables

- `NO_COLOR` — when set (to any value), disable all ANSI color output. See [no-color.org](https://no-color.org).

## Error Handling

- Invalid URL → print error to stderr, exit code 1
- curl errors (connection refused, DNS failure, etc.) → print curl error message to stderr, exit with curl's error code
- Non-TTY stdout → disable colors automatically (same behavior as `NO_COLOR`)

## Project Setup

```bash
cargo init --name httpstat /home/frost/dev/oss/statrs
```

### Cargo.toml

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

## Testing Strategy (MVP)

- Unit tests for timing range calculations in `timing.rs`
- Manual E2E testing against known endpoints
- No CI integration in MVP scope

## Future Extensions (Not MVP)

- `--format json` / `--format jsonl` structured output
- `--slo` threshold checking with exit code 4
- `--save` file output
- Environment variables (`HTTPSTAT_SHOW_BODY`, `HTTPSTAT_SHOW_IP`, etc.)
- Extra curl flag passthrough (`-X POST`, `-H`, etc.)
- JSON output with `schema_version: 1`
