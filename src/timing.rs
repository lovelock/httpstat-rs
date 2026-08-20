use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[allow(dead_code)]
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

pub const SLO_KEY_MAP: &[(&str, &str)] = &[
    ("total", "time_total"),
    ("connect", "time_connect"),
    ("ttfb", "time_starttransfer"),
    ("dns", "time_namelookup"),
    ("tls", "time_pretransfer"),
];

pub fn parse_slo(spec: &str) -> Result<Vec<(String, u64)>, String> {
    let mut result = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err("empty SLO spec".to_string());
        }
        let (key, val) = part.split_once('=').ok_or_else(|| {
            format!("invalid SLO spec \"{}\", expected key=value", part)
        })?;
        let key = key.trim();
        let val = val.trim();
        if !SLO_KEY_MAP.iter().any(|(k, _)| *k == key) {
            let valid: Vec<&str> = SLO_KEY_MAP.iter().map(|(k, _)| *k).collect();
            return Err(format!(
                "unknown SLO key \"{}\", valid keys: {}",
                key,
                valid.join(", ")
            ));
        }
        let ms: u64 = val.parse().map_err(|_| {
            format!("SLO value for \"{}\" must be a positive integer, got \"{}\"", key, val)
        })?;
        if ms == 0 {
            return Err(format!("SLO value for \"{}\" must be positive, got 0", key));
        }
        result.push((key.to_string(), ms));
    }
    Ok(result)
}

pub fn check_slo(slo: &[(String, u64)], timings: &CurlTimings) -> (bool, Vec<SloViolation>) {
    let mut violations = Vec::new();
    for (key, threshold) in slo {
        let timing_key = SLO_KEY_MAP
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .unwrap();
        let actual = match timing_key {
            "time_total" => timings.time_total,
            "time_connect" => timings.time_connect,
            "time_starttransfer" => timings.time_starttransfer,
            "time_namelookup" => timings.time_namelookup,
            "time_pretransfer" => timings.time_pretransfer,
            _ => 0.0,
        };
        let actual_ms = (actual * 1000.0) as u64;
        if actual_ms > *threshold {
            violations.push(SloViolation {
                key: key.clone(),
                threshold_ms: *threshold,
                actual_ms,
            });
        }
    }
    (violations.is_empty(), violations)
}

#[derive(Serialize, Clone)]
pub struct SloViolation {
    pub key: String,
    pub threshold_ms: u64,
    pub actual_ms: u64,
}

#[derive(Serialize)]
pub struct JsonOutput {
    pub schema_version: u32,
    pub url: String,
    pub ok: bool,
    pub exit_code: i32,
    pub response: ResponseInfo,
    pub timings_ms: TimingsMs,
    pub speed: SpeedInfo,
    pub slo: Option<SloResult>,
}

#[derive(Serialize)]
pub struct ResponseInfo {
    pub status_line: String,
    pub status_code: u16,
    pub remote_ip: String,
    pub remote_port: String,
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct TimingsMs {
    pub dns: u64,
    pub connect: u64,
    pub tls: u64,
    pub server: u64,
    pub transfer: u64,
    pub total: u64,
    pub namelookup: u64,
    pub initial_connect: u64,
    pub pretransfer: u64,
    pub starttransfer: u64,
}

#[derive(Serialize)]
pub struct SpeedInfo {
    pub download_kbs: f64,
    pub upload_kbs: f64,
}

#[derive(Serialize, Clone)]
pub struct SloResult {
    #[serde(rename = "pass")]
    pub pass: bool,
    pub violations: Vec<SloViolation>,
}

pub fn build_json_result(
    url: &str,
    t: &CurlTimings,
    headers_text: &str,
    slo_result: Option<(bool, Vec<SloViolation>)>,
    exit_code: i32,
) -> JsonOutput {
    let first_line = headers_text.lines().next().unwrap_or("").trim();
    let status_line = first_line.to_string();
    let parts: Vec<&str> = first_line.splitn(3, ' ').collect();
    let status_code: u16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let mut headers_dict = std::collections::HashMap::new();
    for line in headers_text.lines().skip(1) {
        let line = line.trim();
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            headers_dict.insert(key, value);
        }
    }

    let ok = exit_code == 0;

    JsonOutput {
        schema_version: 1,
        url: url.to_string(),
        ok,
        exit_code,
        response: ResponseInfo {
            status_line,
            status_code,
            remote_ip: t.remote_ip.clone(),
            remote_port: t.remote_port.clone(),
            headers: headers_dict,
        },
        timings_ms: TimingsMs {
            dns: (t.range_dns() * 1000.0).round() as u64,
            connect: (t.range_connect() * 1000.0).round() as u64,
            tls: (t.range_tls() * 1000.0).round() as u64,
            server: (t.range_server() * 1000.0).round() as u64,
            transfer: (t.range_transfer() * 1000.0).round() as u64,
            total: (t.time_total * 1000.0).round() as u64,
            namelookup: (t.time_namelookup * 1000.0).round() as u64,
            initial_connect: (t.time_connect * 1000.0).round() as u64,
            pretransfer: (t.time_pretransfer * 1000.0).round() as u64,
            starttransfer: (t.time_starttransfer * 1000.0).round() as u64,
        },
        speed: SpeedInfo {
            download_kbs: (t.speed_download / 1024.0 * 10.0).round() / 10.0,
            upload_kbs: (t.speed_upload / 1024.0 * 10.0).round() / 10.0,
        },
        slo: slo_result.map(|(pass, violations)| SloResult { pass, violations }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        t.time_appconnect = 0.0;
        assert!(!t.is_https());
    }

    #[test]
    fn test_ranges_sum_to_total() {
        let t = make_timings();
        let sum = t.range_dns() + t.range_connect() + t.range_tls() + t.range_server() + t.range_transfer();
        assert!((sum - t.time_total).abs() < 1e-9);
    }

    // --- parse_slo ---

    #[test]
    fn test_parse_slo_single() {
        let result = parse_slo("total=500").unwrap();
        assert_eq!(result, vec![("total".to_string(), 500)]);
    }

    #[test]
    fn test_parse_slo_multiple() {
        let result = parse_slo("total=500,connect=100,ttfb=200").unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_slo_invalid_key() {
        assert!(parse_slo("badkey=100").is_err());
    }

    #[test]
    fn test_parse_slo_invalid_value() {
        assert!(parse_slo("total=abc").is_err());
    }

    #[test]
    fn test_parse_slo_negative() {
        assert!(parse_slo("total=-1").is_err());
    }

    #[test]
    fn test_parse_slo_zero() {
        assert!(parse_slo("total=0").is_err());
    }

    #[test]
    fn test_parse_slo_spaces() {
        let result = parse_slo(" total = 500 , connect = 100 ").unwrap();
        assert_eq!(result, vec![("total".to_string(), 500), ("connect".to_string(), 100)]);
    }

    // --- check_slo ---

    #[test]
    fn test_check_slo_all_pass() {
        let slo = vec![("total".to_string(), 200), ("connect".to_string(), 50)];
        let (pass, violations) = check_slo(&slo, &make_timings());
        assert!(pass);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_check_slo_violation() {
        let slo = vec![("total".to_string(), 50)];
        let (pass, violations) = check_slo(&slo, &make_timings());
        assert!(!pass);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].actual_ms, 100);
    }

    #[test]
    fn test_check_slo_exactly_at_threshold() {
        let slo = vec![("total".to_string(), 100)];
        let (pass, _) = check_slo(&slo, &make_timings());
        assert!(pass);
    }

    #[test]
    fn test_check_slo_ttfb() {
        let slo = vec![("ttfb".to_string(), 50)];
        let (_, violations) = check_slo(&slo, &make_timings());
        assert_eq!(violations[0].actual_ms, 80);
    }

    // --- build_json_result ---

    #[test]
    fn test_build_json_result_schema_version() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\ncontent-type: text/html\r\n",
            None, 0,
        );
        assert_eq!(result.schema_version, 1);
    }

    #[test]
    fn test_build_json_result_status_code() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            None, 0,
        );
        assert_eq!(result.response.status_code, 200);
    }

    #[test]
    fn test_build_json_result_timings() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            None, 0,
        );
        let t = &result.timings_ms;
        assert_eq!(t.dns, 5);
        assert_eq!(t.connect, 10);
        assert_eq!(t.tls, 15);
        assert_eq!(t.server, 50);
        assert_eq!(t.transfer, 20);
        assert_eq!(t.total, 100);
    }

    #[test]
    fn test_build_json_result_speed() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            None, 0,
        );
        assert!((result.speed.download_kbs - 10.0).abs() < 0.1);
        assert_eq!(result.speed.upload_kbs, 0.0);
    }

    #[test]
    fn test_build_json_result_slo_none() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            None, 0,
        );
        assert!(result.slo.is_none());
    }

    #[test]
    fn test_build_json_result_slo_pass() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            Some((true, vec![])),
            0,
        );
        assert!(result.slo.as_ref().unwrap().pass);
    }

    #[test]
    fn test_build_json_result_slo_fail() {
        let violations = vec![SloViolation {
            key: "total".to_string(),
            threshold_ms: 50,
            actual_ms: 100,
        }];
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            Some((false, violations)),
            4,
        );
        assert!(!result.slo.as_ref().unwrap().pass);
        assert_eq!(result.exit_code, 4);
        assert!(!result.ok);
    }

    #[test]
    fn test_build_json_result_http1_status_line() {
        let result = build_json_result(
            "http://example.com", &make_timings(),
            "HTTP/1.1 301 Moved Permanently\r\nLocation: https://example.com\r\n",
            None, 0,
        );
        assert_eq!(result.response.status_line, "HTTP/1.1 301 Moved Permanently");
        assert_eq!(result.response.status_code, 301);
    }

    #[test]
    fn test_build_json_serializable() {
        let result = build_json_result(
            "https://example.com", &make_timings(),
            "HTTP/2 200\r\n",
            Some((true, vec![])), 0,
        );
        let _ = serde_json::to_string(&result).unwrap();
    }
}
