//! Fetching: `http` and `https` only, no cookies, no referrer, no login, limits on size, time and redirects, and **no
//! address that is not on the public internet** (docs/phases/web-annotation.md, "A clip must not become a way to reach
//! into the person's network"). Every connection goes through [`Guard`], which resolves the name itself and hands
//! `ureq` only addresses it has checked, so a name that answers differently a second time does not matter.

use std::collections::HashMap;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use url::Url;

use crate::error::ClipError;

pub const MAX_REDIRECTS: usize = 5;
pub const PAGE_TIMEOUT: Duration = Duration::from_secs(15);
pub const PICTURE_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_PAGE_BYTES: u64 = 5 * 1024 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (compatible; Notatnik web clip)";
/// Marks the error of a refused address inside the error of the HTTP library.
const REFUSED: &str = "notatnik-clip: address refused";

#[derive(Debug, Clone, Default)]
pub struct ClipPolicy {
    /// Fetch addresses on this computer and private networks. Only tests turn this on.
    pub allow_private: bool,
    /// Test hook: names that resolve to these addresses and are exempt from the check, so a redirect from a "public"
    /// name to a private address can be tested without a public server.
    pub pretend_public: HashMap<String, Vec<IpAddr>>,
    /// Test hook: shorter time limits than the ones above.
    pub page_timeout: Option<Duration>,
    pub picture_timeout: Option<Duration>,
}

/// An address on the public internet (not this computer, a private network, link-local, multicast, reserved or a
/// documentation range), also when it is written as an IPv4 address inside an IPv6 one.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => public_v4(a),
        IpAddr::V6(a) => public_v6(a),
    }
}

fn public_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    !(a.is_unspecified()
        || a.is_loopback()
        || a.is_private()
        || a.is_link_local() // 169.254/16, which holds the cloud metadata address
        || a.is_broadcast()
        || a.is_multicast()
        || a.is_documentation()
        || o[0] == 0 // "this network"
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64/10, carrier-grade NAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18/15, benchmarking
        || o[0] >= 240) // reserved
}

fn public_v6(a: Ipv6Addr) -> bool {
    if let Some(v4) = a.to_ipv4_mapped() {
        return public_v4(v4);
    }
    let s = a.segments();
    // IPv4-compatible (::a.b.c.d), NAT64 (64:ff9b::/96) and 6to4 (2002::/16) carry an IPv4 address: it has to be public
    if s[..6] == [0, 0, 0, 0, 0, 0] && !(a.is_unspecified() || a.is_loopback()) {
        return public_v4(Ipv4Addr::new(
            (s[6] >> 8) as u8,
            s[6] as u8,
            (s[7] >> 8) as u8,
            s[7] as u8,
        ));
    }
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return public_v4(Ipv4Addr::new(
            (s[6] >> 8) as u8,
            s[6] as u8,
            (s[7] >> 8) as u8,
            s[7] as u8,
        ));
    }
    if s[0] == 0x2002 {
        return public_v4(Ipv4Addr::new(
            (s[1] >> 8) as u8,
            s[1] as u8,
            (s[2] >> 8) as u8,
            s[2] as u8,
        ));
    }
    !(a.is_unspecified()
        || a.is_loopback()
        || a.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // fc00::/7, unique local
        || (s[0] & 0xffc0) == 0xfe80 // fe80::/10, link-local
        || (s[0] == 0x2001 && s[1] == 0x0db8)) // documentation
}

/// The only way a connection finds its address.
#[derive(Debug)]
struct Guard {
    policy: Arc<ClipPolicy>,
}

impl ureq::Resolver for Guard {
    fn resolve(&self, netloc: &str) -> io::Result<Vec<SocketAddr>> {
        let (host, port) = split_netloc(netloc);
        if let Some(addrs) = self.policy.pretend_public.get(host) {
            return Ok(addrs.iter().map(|ip| SocketAddr::new(*ip, port)).collect());
        }
        let addrs: Vec<SocketAddr> = netloc.to_socket_addrs()?.collect();
        if addrs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the name has no address",
            ));
        }
        if !self.policy.allow_private && addrs.iter().any(|a| !is_public(a.ip())) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, REFUSED));
        }
        Ok(addrs)
    }
}

fn split_netloc(netloc: &str) -> (&str, u16) {
    let (host, port) = netloc.rsplit_once(':').unwrap_or((netloc, "80"));
    (
        host.trim_start_matches('[').trim_end_matches(']'),
        port.parse().unwrap_or(80),
    )
}

/// What a fetch brings back.
pub struct Fetched {
    /// The address after the redirects.
    pub url: Url,
    /// The `Content-Type` header, lower case, with its parameters (`text/html; charset=utf-8`).
    pub content_type: String,
    pub body: Vec<u8>,
}

/// `http(s)://host/…` without a login; nothing else is ever requested.
pub fn check_address(url: &Url) -> Result<(), ClipError> {
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ClipError::BadAddress);
    }
    Ok(())
}

fn host_of(url: &Url) -> String {
    url.host_str().unwrap_or("the site").to_string()
}

fn refused(e: &ureq::Transport) -> bool {
    use std::error::Error;
    let mut cur: Option<&dyn Error> = Some(e);
    while let Some(err) = cur {
        if err.to_string().contains(REFUSED) {
            return true;
        }
        cur = err.source();
    }
    false
}

/// GET with the limits: at most `max_bytes` of body, `timeout` for the whole request, [`MAX_REDIRECTS`] redirects (each
/// followed here, so each is checked), no cookies and no referrer.
pub fn get(
    policy: &Arc<ClipPolicy>,
    start: &Url,
    accept: &str,
    max_bytes: u64,
    timeout: Duration,
) -> Result<Fetched, ClipError> {
    let agent = ureq::AgentBuilder::new()
        .resolver(Guard {
            policy: policy.clone(),
        })
        .redirects(0)
        .timeout_connect(Duration::from_secs(5))
        .timeout(timeout)
        .user_agent(USER_AGENT)
        .build();
    let mut url = start.clone();
    for _ in 0..=MAX_REDIRECTS {
        check_address(&url)?;
        // an address written as a number is checked here as well as in the resolver, so no request is made at all
        if !policy.allow_private
            && let Some(url::Host::Ipv4(_) | url::Host::Ipv6(_)) = url.host()
            && let Some(ip) = url
                .host_str()
                .and_then(|h| h.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            && !is_public(ip)
        {
            return Err(ClipError::Refused);
        }
        let resp = match agent.get(url.as_str()).set("Accept", accept).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) if (300..400).contains(&code) => {
                // with `redirects(0)` ureq may report a redirect as a status error: treat it like the Ok case
                r
            }
            Err(ureq::Error::Status(code, _)) => {
                return Err(ClipError::Status(host_of(&url), code));
            }
            Err(ureq::Error::Transport(t)) => {
                return Err(if refused(&t) {
                    ClipError::Refused
                } else {
                    ClipError::Unreachable(host_of(&url))
                });
            }
        };
        if (300..400).contains(&resp.status()) {
            let Some(location) = resp.header("Location") else {
                return Err(ClipError::Status(host_of(&url), resp.status()));
            };
            url = url.join(location).map_err(|_| ClipError::BadAddress)?;
            continue;
        }
        let content_type = resp
            .header("Content-Type")
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut body = Vec::new();
        resp.into_reader()
            .take(max_bytes + 1)
            .read_to_end(&mut body)
            .map_err(|_| ClipError::Unreachable(host_of(&url)))?;
        if body.len() as u64 > max_bytes {
            return Err(ClipError::TooBig);
        }
        return Ok(Fetched {
            url,
            content_type,
            body,
        });
    }
    Err(ClipError::TooManyRedirects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn public_addresses_are_public() {
        for a in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "2606:4700:4700::1111",
            "2a00:1450:4001::200e",
        ] {
            assert!(is_public(ip(a)), "{a}");
        }
    }

    #[test]
    fn this_computer_private_networks_and_metadata_are_not() {
        for a in [
            "127.0.0.1",
            "127.8.9.10",
            "0.0.0.0",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "169.254.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "192.0.0.8",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "192.0.2.1",
            "198.51.100.7",
            "203.0.113.9",
            "::1",
            "::",
            "fe80::1",
            "fc00::1",
            "fd12:3456::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:169.254.169.254",
            "::127.0.0.1",
            "64:ff9b::7f00:1",
            "2002:7f00:1::",
            "2002:c0a8:101::",
        ] {
            assert!(!is_public(ip(a)), "{a} must be refused");
        }
        // the edges of the private ranges
        assert!(is_public(ip("172.15.255.255")) && is_public(ip("172.32.0.0")));
        assert!(is_public(ip("100.63.255.255")) && is_public(ip("100.128.0.0")));
        assert!(is_public(ip("::ffff:8.8.8.8")) && is_public(ip("2002:0808:0808::")));
    }

    #[test]
    fn only_web_addresses_without_a_login_are_fetched() {
        for ok in ["http://example.org/", "https://example.org:8443/a?b#c"] {
            assert!(check_address(&Url::parse(ok).unwrap()).is_ok(), "{ok}");
        }
        for bad in [
            "file:///etc/passwd",
            "ftp://example.org/",
            "javascript:alert(1)",
            "data:text/html,hi",
            "https://user:pw@example.org/",
            "https://user@example.org/",
            "gopher://example.org/",
        ] {
            assert_eq!(
                check_address(&Url::parse(bad).unwrap()),
                Err(ClipError::BadAddress),
                "{bad}"
            );
        }
    }
}
