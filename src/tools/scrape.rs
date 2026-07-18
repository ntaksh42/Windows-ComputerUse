//! `Scrape` tool: fetches a URL over HTTP(S) and converts its HTML body to
//! Markdown, with per-hop SSRF protection on manually-followed redirects.
//!
//! `use_dom` and `use_sampling` are accepted for API compatibility with the
//! Python reference implementation but are not yet supported: `use_dom=true`
//! returns a fixed "not supported yet" message, and sampling is skipped
//! (raw Markdown is always returned).

use std::net::IpAddr;
use std::time::Duration;

use reqwest::Url;
use reqwest::redirect::Policy;
use rmcp::schemars;
use serde::Deserialize;

use crate::params::{BoolOrString, opt_bool};

const MAX_REDIRECTS: usize = 5;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Parameters for the `Scrape` tool.
///
/// `query` and `use_sampling` are accepted for API compatibility with the
/// Python reference implementation but are not read: sampling-based
/// summarization is not yet implemented, so raw Markdown is always returned.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[allow(dead_code)]
pub struct ScrapeParams {
    /// The URL to fetch.
    #[schemars(description = "The URL to fetch.")]
    pub url: String,
    /// Optional focus query (currently unused; sampling is not yet implemented).
    pub query: Option<String>,
    /// Extract from the active browser tab's DOM instead of an HTTP request.
    /// Not yet supported.
    #[serde(default)]
    pub use_dom: Option<BoolOrString>,
    /// Summarize the content via MCP sampling. Not yet implemented; raw
    /// Markdown is always returned.
    #[serde(default)]
    pub use_sampling: Option<BoolOrString>,
}

/// Runs the `Scrape` tool.
///
/// Returns `Ok` with the caller-facing success text, or `Err` with a
/// caller-facing error message for unexpected failures (invalid/blocked
/// URLs, connection failures) which the caller should surface as an MCP
/// `isError` result.
pub async fn scrape(params: ScrapeParams) -> Result<String, String> {
    let use_dom = opt_bool(&params.use_dom, false)?;
    if use_dom {
        return Ok("DOM mode not supported yet. Use Snapshot instead.".to_string());
    }

    let content = fetch_markdown(&params.url).await?;
    Ok(format!("URL: {}\nContent:\n{content}", params.url))
}

async fn fetch_markdown(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Error: Failed to build HTTP client: {e}"))?;

    let mut current = url.to_string();
    let mut response = None;
    for _ in 0..MAX_REDIRECTS {
        validate_url(&current).await?;

        let resp = client
            .get(&current)
            .send()
            .await
            .map_err(|e| format!("Error: Failed to connect to {current}: {e}"))?;

        if resp.status().is_redirection() {
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| format!("Error: Redirect from {current} has no Location header"))?;
            let base =
                Url::parse(&current).map_err(|e| format!("Error: Invalid URL: {current}: {e}"))?;
            let next = base.join(location).map_err(|e| {
                format!("Error: Invalid redirect location from {current}: {location}: {e}")
            })?;
            current = next.to_string();
            continue;
        }

        response = Some(resp);
        break;
    }

    let response =
        response.ok_or_else(|| "Error: Too many redirects while fetching URL".to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Error: HTTP error for {current}: {status}"));
    }

    let html = response
        .text()
        .await
        .map_err(|e| format!("Error: Failed to read response body from {current}: {e}"))?;
    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style"])
        .build();
    converter
        .convert(&html)
        .map_err(|e| format!("Error: Failed to convert HTML to Markdown: {e}"))
}

/// Validates a URL is safe to fetch (SSRF protection): only http/https,
/// no embedded credentials, and none of the resolved addresses are
/// private, loopback, link-local, multicast, reserved, or unspecified.
async fn validate_url(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| format!("Error: Invalid URL: {url}"))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(format!(
            "Error: URL scheme '{}' is not allowed; use http or https.",
            parsed.scheme()
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| format!("Error: URL has no hostname: {url}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Error: URLs with embedded credentials are not allowed.".to_string());
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("Error: Could not resolve hostname '{host}': {e}"))?;

    let mut any = false;
    for addr in addrs {
        any = true;
        let ip = addr.ip();
        if is_blocked_ip(ip) {
            return Err(format!(
                "Error: Private, loopback, link-local, multicast, and reserved addresses are blocked: {ip}"
            ));
        }
    }
    if !any {
        return Err(format!(
            "Error: Could not resolve hostname '{host}': no addresses returned"
        ));
    }

    Ok(())
}

/// Whether `ip` is a private, loopback, link-local, multicast, reserved, or
/// unspecified address that must not be reachable from `Scrape` (SSRF guard).
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 240 // 240.0.0.0/4: reserved for future use
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7: unique local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10: link-local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn blocks_private_ipv4() {
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
    }

    #[test]
    fn allows_public_ipv4() {
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)))); // example.com
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }

    #[test]
    fn blocks_private_ipv6() {
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
    }
}
