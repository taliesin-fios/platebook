// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Outbound fetch for recipe URL import, hardened against SSRF.
//!
//! Same protections as the Node route, plus DNS checks:
//! - the URL must be http(s); `localhost`, private, loopback, link-local and
//!   unique-local addresses are refused (IP literals in any notation, since
//!   the WHATWG parser normalizes them);
//! - redirects are followed by hand (at most 5 hops) and every hop is
//!   re-validated;
//! - every hostname is resolved through [`PublicResolver`], which drops
//!   non-public addresses, so a public name that resolves (or rebinds) to an
//!   internal IP is refused too;
//! - 10 s timeout per request, no proxies, body capped at 5 MB.

use std::error::Error as StdError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{ACCEPT, LOCATION, USER_AGENT};
use url::{Host, Url};

pub const MAX_HOPS: usize = 5;
pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_BODY: usize = 5 * 1024 * 1024;

/// True for addresses an import must never reach.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_v4(v4);
            }
            if let Some(v4) = embedded_v4(v6)
                && is_blocked_v4(v4)
            {
                return true;
            }
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 link local
                || (s[0] & 0xffc0) == 0xfec0 // fec0::/10 site local (deprecated)
                || (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1) // 64:ff9b:1::/48 local NAT64
                || (s[0] == 0x2001 && s[1] == 0) // 2001::/32 Teredo
                || v6 == Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)
        }
    }
}

/// The IPv4 address carried inside an IPv6 one, for the forms a host may
/// route to that IPv4 address: NAT64 (`64:ff9b::/96`), 6to4 (`2002::/16`)
/// and the deprecated IPv4-compatible `::a.b.c.d`.
fn embedded_v4(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = v6.segments();
    let v4 = |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return Some(v4(s[6], s[7]));
    }
    if s[0] == 0x2002 {
        return Some(v4(s[1], s[2]));
    }
    if s[..6] == [0, 0, 0, 0, 0, 0] {
        return Some(v4(s[6], s[7]));
    }
    None
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()                 // 127/8
        || ip.is_private()           // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local()        // 169.254/16 (cloud metadata)
        || ip.is_unspecified()       // 0.0.0.0
        || ip.is_broadcast()
        || ip.is_multicast()
        || a == 0                    // 0/8
        || (a == 100 && (64..128).contains(&b)) // 100.64/10 carrier-grade NAT
        || (a == 192 && b == 0 && ip.octets()[2] == 0) // 192.0.0/24 IETF protocol assignments
        || (a == 198 && (b == 18 || b == 19)) // 198.18/15 benchmarking
        || a >= 240 // 240/4 reserved (and broadcast)
}

/// Port of `isSsrfBlocked(url)`: unparseable, non-http(s), localhost or a
/// blocked IP literal.
pub fn is_url_blocked(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return true;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    match url.host() {
        None => true,
        Some(Host::Ipv4(ip)) => is_blocked_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => is_blocked_ip(IpAddr::V6(ip)),
        Some(Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost")
        }
    }
}

/// Marker error: a hostname resolved only to blocked addresses.
#[derive(Debug)]
pub struct BlockedAddress;

impl std::fmt::Display for BlockedAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("host resolves to a disallowed address")
    }
}

impl StdError for BlockedAddress {}

/// DNS resolver that only ever hands reqwest public addresses.
pub struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| !is_blocked_ip(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(Box::new(BlockedAddress) as Box<dyn StdError + Send + Sync>);
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// The HTTP client used for imports.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(PublicResolver))
        .no_proxy()
        .timeout(TIMEOUT)
        .build()
        .expect("import HTTP client")
}

#[derive(Debug)]
pub enum FetchError {
    /// A redirect (or DNS answer) pointed somewhere disallowed.
    Disallowed,
    Timeout,
    /// Final response was not 2xx.
    Status(u16),
    TooLarge,
    Other(String),
}

fn classify(err: reqwest::Error) -> FetchError {
    if err.is_timeout() {
        return FetchError::Timeout;
    }
    let mut source: Option<&(dyn StdError + 'static)> = err.source();
    while let Some(s) = source {
        if s.is::<BlockedAddress>() {
            return FetchError::Disallowed;
        }
        source = s.source();
    }
    FetchError::Other(err.to_string())
}

/// GETs `url` following up to [`MAX_HOPS`] validated redirects; returns the
/// body text of the final 2xx response.
pub async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String, FetchError> {
    let mut current = Url::parse(url).map_err(|_| FetchError::Disallowed)?;
    let mut response = None;
    for _ in 0..MAX_HOPS {
        let res = client
            .get(current.clone())
            .header(
                USER_AGENT,
                "Mozilla/5.0 (compatible; PlatebookRecipeImporter/1.0)",
            )
            .header(ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await
            .map_err(classify)?;
        let location = res
            .headers()
            .get(LOCATION)
            .and_then(|l| l.to_str().ok())
            .map(str::to_string);
        if res.status().is_redirection()
            && let Some(loc) = location
        {
            let next = current.join(&loc).map_err(|_| FetchError::Disallowed)?;
            if is_url_blocked(next.as_str()) {
                return Err(FetchError::Disallowed);
            }
            current = next;
            continue;
        }
        response = Some(res);
        break;
    }
    // Still redirecting after MAX_HOPS: report the last status, like Node.
    let mut res = match response {
        Some(r) => r,
        None => client.get(current).send().await.map_err(classify)?,
    };
    if !res.status().is_success() {
        return Err(FetchError::Status(res.status().as_u16()));
    }
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(classify)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(FetchError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_the_node_list_and_more() {
        for url in [
            "http://localhost/x",
            "http://LOCALHOST:8080",
            "http://api.localhost/",
            "http://127.0.0.1/",
            "http://127.1/",
            "http://2130706433/",
            "http://0.0.0.0/",
            "http://10.1.2.3/",
            "http://172.16.0.1/",
            "http://172.31.255.255/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://[fe80::1]/",
            "http://[::ffff:127.0.0.1]/",
            // IPv4 inside IPv6: NAT64, 6to4, IPv4-compatible.
            "http://[64:ff9b::7f00:1]/",
            "http://[64:ff9b::a9fe:a9fe]/",
            "http://[2002:7f00:1::]/",
            "http://[2002:a9fe:a9fe::]/",
            "http://[::127.0.0.1]/",
            "http://[::a00:1]/",
            // Local-use NAT64, Teredo, site-local.
            "http://[64:ff9b:1::a00:1]/",
            "http://[2001:0:4136:e378:8000:63bf:80ff:fffe]/",
            "http://[fec0::1]/",
            // Reserved IPv4.
            "http://192.0.0.1/",
            "http://198.18.0.1/",
            "http://198.19.255.255/",
            "http://240.0.0.1/",
            "ftp://example.com/",
            "file:///etc/passwd",
            "not a url",
        ] {
            assert!(is_url_blocked(url), "{url} should be blocked");
        }
        for url in [
            "https://www.example.com/recipe",
            "http://172.32.0.1/",
            "http://8.8.8.8/",
            // Public IPv4 reached through NAT64 / 6to4 stays allowed.
            "http://[64:ff9b::808:808]/",
            "http://[2002:808:808::]/",
            "http://[2606:4700::1111]/",
            "http://192.0.2.1/",
            "http://198.20.0.1/",
            "https://fdafoods.com/",
        ] {
            assert!(!is_url_blocked(url), "{url} should be allowed");
        }
    }
}
