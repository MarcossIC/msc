use crate::core::system_info::types::NetworkInfo;
use crate::error::Result;
use std::time::Duration;

#[cfg(windows)]
use crate::platform::system::windows::network::{get_network_info, get_network_info_profiled};

pub fn collect() -> Result<NetworkInfo> {
    #[cfg(windows)]
    {
        get_network_info()
    }

    #[cfg(not(windows))]
    {
        // Could use networkmanager on Linux or networksetup on macOS
        Ok(get_fallback())
    }
}

pub fn collect_with_subs() -> Result<(NetworkInfo, Vec<(String, Duration)>)> {
    #[cfg(windows)]
    {
        get_network_info_profiled()
    }

    #[cfg(not(windows))]
    {
        Ok((get_fallback(), Vec::new()))
    }
}

pub fn get_fallback() -> NetworkInfo {
    NetworkInfo {
        wifi_adapters: vec![],
        ethernet_adapters: vec![],
        bluetooth_adapters: vec![],
        public_ip: None,
        internet_latency_ms: None,
    }
}

/// Internet (WAN) round-trip latency in ms — distinct from the per-adapter
/// gateway (LAN) latency. Runs on its own collector thread (off-critical-path).
pub fn internet_latency() -> Option<u32> {
    #[cfg(windows)]
    {
        crate::platform::system::windows::network::measure_internet_latency()
    }

    #[cfg(not(windows))]
    {
        None
    }
}

/// Validate the body returned by a public-IP echo service.
///
/// Services like icanhazip.com reply with the bare IP plus a trailing newline.
/// We parse it as a real `IpAddr` so any HTML error page, captive-portal
/// redirect, or junk is rejected outright (returns `None`), and re-emit the
/// canonical form.
fn sanitize_public_ip(body: &str) -> Option<String> {
    body.trim()
        .parse::<std::net::IpAddr>()
        .ok()
        .map(|ip| ip.to_string())
}

/// Best-effort public/WAN IP lookup with a tight timeout.
///
/// Runs on its own collector thread (off the network section's critical path).
/// Any failure — offline, DNS error, timeout, junk response — yields `None`, so
/// `sys info` never blocks waiting on the internet.
pub fn fetch_public_ip() -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1200))
        .build()
        .ok()?;

    let body = client
        .get("https://icanhazip.com")
        .send()
        .ok()?
        .text()
        .ok()?;

    sanitize_public_ip(&body)
}

#[cfg(test)]
mod tests {
    use super::sanitize_public_ip;

    #[test]
    fn accepts_ipv4_with_trailing_newline() {
        assert_eq!(
            sanitize_public_ip("203.0.113.7\n"),
            Some("203.0.113.7".to_string())
        );
    }

    #[test]
    fn accepts_ipv6() {
        assert_eq!(
            sanitize_public_ip("  2001:db8::1  "),
            Some("2001:db8::1".to_string())
        );
    }

    #[test]
    fn rejects_html_and_junk() {
        assert_eq!(sanitize_public_ip("<html>error</html>"), None);
        assert_eq!(sanitize_public_ip(""), None);
        assert_eq!(sanitize_public_ip("not-an-ip"), None);
    }
}
