use crate::timing::CurlTimings;

const RESET: &str = "\x1b[0m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[38;5;242m";
const RED: &str = "\x1b[31m";

fn colorize(s: &str, color: &str, use_color: bool) -> String {
    if use_color {
        format!("{}{}{}", color, s, RESET)
    } else {
        s.to_string()
    }
}

/// Format a value for the bracket row (centered, 7 chars)
fn fmta(ms: i64, use_color: bool) -> String {
    let s = format!("{:^7}", format!("{}ms", ms));
    colorize(&s, CYAN, use_color)
}

/// Format a value for the label row (left-aligned, 7 chars)
fn fmtb(ms: i64, use_color: bool) -> String {
    let s = format!("{:<7}", format!("{}ms", ms));
    colorize(&s, CYAN, use_color)
}

pub struct PrettyOptions {
    pub show_body: bool,
    pub show_ip: bool,
    pub show_speed: bool,
    pub save_body: bool,
    pub body_path: Option<String>,
    pub body_content: Option<String>,
    pub body_total_len: Option<usize>,
}

pub fn print_pretty(
    t: &CurlTimings,
    _status_line: &str,
    headers_text: &str,
    opts: &PrettyOptions,
    use_color: bool,
) {
    let dns = (t.range_dns() * 1000.0).round() as i64;
    let connect = (t.range_connect() * 1000.0).round() as i64;
    let tls = (t.range_tls() * 1000.0).round() as i64;
    let server = (t.range_server() * 1000.0).round() as i64;
    let transfer = (t.range_transfer() * 1000.0).round() as i64;
    let total_ms = (t.total * 1000.0).round() as i64;
    let namelookup_ms = (t.namelookup * 1000.0).round() as i64;
    let connect_ms = (t.connect * 1000.0).round() as i64;
    let pretransfer_ms = (t.pretransfer * 1000.0).round() as i64;
    let starttransfer_ms = (t.starttransfer * 1000.0).round() as i64;

    // IP info
    if opts.show_ip {
        println!(
            "Connected to {}:{} from {}:{}",
            colorize(&t.remote_ip, CYAN, use_color),
            colorize(&t.remote_port, CYAN, use_color),
            t.local_ip,
            t.local_port,
        );
        println!();
    }

    // Headers
    for (i, line) in headers_text.lines().enumerate() {
        if i == 0 {
            let parts: Vec<&str> = line.splitn(2, '/').collect();
            if parts.len() == 2 {
                println!(
                    "{}{}{}",
                    colorize(parts[0], GREEN, use_color),
                    colorize("/", GRAY, use_color),
                    colorize(parts[1], CYAN, use_color),
                );
            } else {
                println!("{}", colorize(line, GREEN, use_color));
            }
        } else if let Some(pos) = line.find(':') {
            println!(
                "{}{}",
                colorize(&line[..pos + 1], GRAY, use_color),
                colorize(&line[pos + 1..], CYAN, use_color),
            );
        }
    }
    println!();

    // Body
    if opts.show_body {
        let body_limit = 1024;
        if let Some(body) = &opts.body_content {
            let body_len = opts.body_total_len.unwrap_or(body.len());
            if body_len > body_limit {
                print!("{}{}", &body[..body_limit], colorize("...", CYAN, use_color));
                println!();
                let mut msg = format!(
                    "{} is truncated ({} out of {})",
                    colorize("Body", GREEN, use_color),
                    body_limit,
                    body_len,
                );
                if opts.save_body {
                    if let Some(path) = &opts.body_path {
                        msg += &format!(", stored in: {}", path);
                    }
                }
                println!("{}", msg);
            } else {
                println!("{}", body);
            }
        }
    } else if opts.save_body {
        if let Some(path) = &opts.body_path {
            println!(
                "{} stored in: {}",
                colorize("Body", GREEN, use_color),
                path,
            );
        }
    }

    // Timing diagram — matches Python httpstat template exactly
    let g = |s: &str| if use_color { format!("\x1b[38;5;242m{}\x1b[0m", s) } else { s.to_string() };

    if t.is_https() {
        println!("  DNS Lookup   TCP Connection   TLS Handshake   Server Processing   Content Transfer");
        println!(
            "[{} | {} | {} | {} | {} ]",
            fmta(dns, use_color), fmta(connect, use_color),
            fmta(tls, use_color), fmta(server, use_color), fmta(transfer, use_color),
        );
        println!(
            "{}",
            format!(
                " {}|{} {}|{} {}|{} {}|{} {}|{}",
                g("          "), g(""), g("             "), g(""),
                g("              "), g(""), g("                   "), g(""),
                g("                  "), g(""),
            )
        );
        println!(
            "{}",
            format!(
                "    {}{}        {}               {}                   {}                  {}",
                g("namelookup:"), fmtb(namelookup_ms, use_color),
                g("|"), g("|"), g("|"), g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                        {}{}       {}                   {}                  {}",
                g("connect:"), fmtb(connect_ms, use_color),
                g("|"), g("|"), g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                                    {}{}           {}                  {}",
                g("pretransfer:"), fmtb(pretransfer_ms, use_color),
                g("|"), g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                                                      {}{}          {}",
                g("starttransfer:"), fmtb(starttransfer_ms, use_color),
                g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                                                                                 {}{}",
                g("total:"), fmtb(total_ms, use_color),
            )
        );
    } else {
        println!("  DNS Lookup   TCP Connection   Server Processing   Content Transfer");
        println!(
            "[{} | {} | {} | {} ]",
            fmta(dns, use_color), fmta(connect, use_color),
            fmta(server, use_color), fmta(transfer, use_color),
        );
        println!(
            "{}",
            format!(
                " {}|{} {}|{} {}|{} {}|{}",
                g("          "), g(""), g("             "), g(""),
                g("                   "), g(""), g("                  "), g(""),
            )
        );
        println!(
            "{}",
            format!(
                "    {}{}        {}                   {}                  {}",
                g("namelookup:"), fmtb(namelookup_ms, use_color),
                g("|"), g("|"), g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                        {}{}           {}                  {}",
                g("connect:"), fmtb(connect_ms, use_color),
                g("|"), g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                                    {}{}           {}",
                g("starttransfer:"), fmtb(starttransfer_ms, use_color),
                g("|"),
            )
        );
        println!(
            "{}",
            format!(
                "                                                             {}{}",
                g("total:"), fmtb(total_ms, use_color),
            )
        );
    }

    // Speed
    if opts.show_speed {
        println!(
            "speed_download: {:.1} KiB/s, speed_upload: {:.1} KiB/s",
            t.speed_download / 1024.0,
            t.speed_upload / 1024.0,
        );
    }
}

pub fn print_slo_violations(violations: &[crate::timing::SloViolation], use_color: bool) {
    println!();
    for v in violations {
        println!(
            "{}",
            colorize(
                &format!("SLO VIOLATION: {} = {}ms (threshold: {}ms)", v.key, v.actual_ms, v.threshold_ms),
                RED,
                use_color,
            )
        );
    }
}
