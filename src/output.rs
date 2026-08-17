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
