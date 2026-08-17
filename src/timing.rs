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
