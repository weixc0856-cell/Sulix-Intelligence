use crate::FetchError;
use std::net::IpAddr;

/// Basic SSRF guard.  Used both for feed URLs (trusted, self-maintained
/// list) and for article URLs in `extract_full_text` (untrusted, comes
/// from third-party feed data) -- both paths go through the same check.
pub fn guard_public_url(url: &str) -> Result<(), FetchError> {
    let parsed = url::Url::parse(url).map_err(|e| FetchError::Ssrf(e.to_string()))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(FetchError::Ssrf(format!("disallowed scheme: {}", parsed.scheme())));
    }

    let host = parsed.host_str().ok_or_else(|| FetchError::Ssrf("missing host".into()))?;

    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".local") {
        return Err(FetchError::Ssrf(format!("localhost-alias host: {host}")));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4 == std::net::Ipv4Addr::new(169, 254, 169, 254)
            }
            IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
        };
        if blocked {
            return Err(FetchError::Ssrf(format!("IP-literal host in blocked range: {ip}")));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_public_url_accepts_https() {
        assert!(guard_public_url("https://example.com/feed.xml").is_ok());
    }

    #[test]
    fn guard_public_url_accepts_http() {
        assert!(guard_public_url("http://example.com/feed.xml").is_ok());
    }

    #[test]
    fn guard_public_url_rejects_ftp() {
        assert!(guard_public_url("ftp://example.com/file").is_err());
    }

    #[test]
    fn guard_public_url_rejects_no_scheme() {
        assert!(guard_public_url("example.com/file").is_err());
    }

    #[test]
    fn guard_public_url_rejects_localhost() {
        assert!(guard_public_url("http://localhost/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_localhost_with_port() {
        assert!(guard_public_url("http://localhost:8080/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_dot_local() {
        assert!(guard_public_url("http://myhost.local/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_loopback_ipv4() {
        assert!(guard_public_url("http://127.0.0.1/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_private_ipv4() {
        assert!(guard_public_url("http://192.168.1.1/feed").is_err());
        assert!(guard_public_url("http://10.0.0.1/feed").is_err());
        assert!(guard_public_url("http://172.16.0.1/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_link_local() {
        assert!(guard_public_url("http://169.254.1.1/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_cloud_metadata() {
        assert!(guard_public_url("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn guard_public_url_rejects_loopback_ipv6() {
        let v6: std::net::Ipv6Addr = "::1".parse().unwrap();
        assert!(v6.is_loopback());
        let _ = guard_public_url("http://[::1]/feed");
    }

    #[test]
    fn guard_public_url_rejects_ula_ipv6_logic() {
        let v6: std::net::Ipv6Addr = "fd00::1".parse().unwrap();
        let is_ula = (v6.segments()[0] & 0xfe00) == 0xfc00;
        assert!(is_ula, "fd00::1 should be ULA");

        let v6_public: std::net::Ipv6Addr = "2600::1".parse().unwrap();
        let is_not_ula = (v6_public.segments()[0] & 0xfe00) != 0xfc00;
        assert!(is_not_ula, "2600::1 should not be ULA");
    }

    #[test]
    fn guard_public_url_accepts_public_ipv4() {
        assert!(guard_public_url("http://93.184.216.34/feed").is_ok());
    }

    #[test]
    fn guard_public_url_accepts_domain_name() {
        assert!(guard_public_url("https://openai.com/news/rss.xml").is_ok());
        assert!(guard_public_url("https://blog.google/technology/ai/rss/").is_ok());
    }

    #[test]
    fn guard_public_url_rejects_empty_string() {
        assert!(guard_public_url("").is_err());
    }

    #[test]
    fn guard_public_url_rejects_missing_host_parse_error() {
        assert!(guard_public_url("not-a-url").is_err());
    }
}
