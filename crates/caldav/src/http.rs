//! CalDAV over HTTP (RFC 4791, RFC 6578) for Nextcloud and other servers.

use std::sync::Mutex;
use std::time::Duration;

use notatnik_sync::{basic_auth, check_url, norm_etag};

use crate::xml::{self, Resp};
use crate::{CalDavClient, Calendar, Cond, Error, ItemRef, Result, ServerItem, SyncResult};

/// The folder of an account's calendars on a Nextcloud, from the address the user gave: the server
/// (`https://cloud.example.org`) or any WebDAV address of it (`…/remote.php/dav/files/me/Notes`, the graph's own
/// sync address), plus the login name.
pub fn calendar_home_url(server: &str, user: &str) -> String {
    format!(
        "{}/remote.php/dav/calendars/{}/",
        cloud_root(server),
        encode(user.trim())
    )
}

/// The server's address up to where `/remote.php/` begins.
fn cloud_root(server: &str) -> &str {
    let s = server.trim();
    s.split_once("/remote.php/")
        .map_or_else(|| s.trim_end_matches('/'), |(b, _)| b.trim_end_matches('/'))
}

fn origin(url: &str) -> &str {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let end = rest.find('/').unwrap_or(rest.len());
            &url[..scheme.len() + 3 + end]
        }
        None => url,
    }
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| char::from(c).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The last part of an `href`, decoded: `a.ics`.
fn last_segment(href: &str) -> String {
    decode(href.trim_end_matches('/').rsplit('/').next().unwrap_or(""))
}

const DAV: &str = r#"xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav""#;

/// A CalDAV account with a login.
pub struct HttpCalDav {
    agent: ureq::Agent,
    /// `scheme://host:port`
    origin: String,
    /// The server's address up to `/remote.php/`.
    root: String,
    user: String,
    auth: String,
    /// Where the account's calendars are (an `href`), once found.
    home: Mutex<Option<String>>,
}

impl HttpCalDav {
    /// `server` is a Nextcloud address or any WebDAV address of it; `user` the login name (an app password is
    /// best as the password). Plain `http` only works for this computer and the local network.
    pub fn new(server: &str, user: &str, password: &str) -> Result<HttpCalDav> {
        let root = cloud_root(server).to_string();
        if root.is_empty() {
            return Err(Error::Url("the server address is empty".into()));
        }
        check_url(&root)?;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build();
        Ok(HttpCalDav {
            agent,
            origin: origin(&root).to_string(),
            root,
            user: user.trim().to_string(),
            auth: basic_auth(user.trim(), password),
            home: Mutex::new(None),
        })
    }

    fn abs(&self, href: &str) -> String {
        if href.starts_with("http://") || href.starts_with("https://") {
            href.to_string()
        } else {
            format!("{}{}", self.origin, href)
        }
    }

    fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, String)],
        body: &str,
    ) -> Result<ureq::Response> {
        let mut req = self
            .agent
            .request(method, url)
            .set("Authorization", &self.auth);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        match req.send_bytes(body.as_bytes()) {
            Ok(r) | Err(ureq::Error::Status(_, r)) => Ok(r),
            Err(ureq::Error::Transport(t)) => Err(Error::Offline(t.to_string())),
        }
    }

    fn fail(status: u16, what: &str) -> Error {
        match status {
            401 | 403 => Error::Auth(format!("HTTP {status}")),
            404 => Error::NotFound(what.to_string()),
            409 | 412 => Error::Changed(what.to_string()),
            423 => Error::Server(format!("{what} is locked on the server (HTTP 423)")),
            s => Error::Server(format!("HTTP {s} for {what}")),
        }
    }

    /// PROPFIND or REPORT that answers `207`.
    fn multi(&self, method: &str, url: &str, depth: &str, body: &str) -> Result<xml::Multi> {
        let resp = self.request(
            method,
            url,
            &[
                ("Depth", depth.into()),
                ("Content-Type", "application/xml; charset=utf-8".into()),
            ],
            body,
        )?;
        let status = resp.status();
        if status != 207 {
            return Err(Self::fail(status, url));
        }
        let text = resp
            .into_string()
            .map_err(|e| Error::Offline(e.to_string()))?;
        xml::parse(&text)
    }

    /// `current-user-principal` of the server's root, then that principal's `calendar-home-set`.
    fn discover_home(&self, home_set_body: &str) -> Result<Option<String>> {
        let body = format!(
            r#"<?xml version="1.0"?><d:propfind {DAV}><d:prop><d:current-user-principal/></d:prop></d:propfind>"#
        );
        let root = format!("{}/", self.root);
        let principal = match self.multi("PROPFIND", &root, "0", &body) {
            Ok(m) => m.principal,
            // a wrong password stops here; "forbidden" only means this is not where the account lives
            Err(Error::Auth(e)) if e.contains("401") => return Err(Error::Auth(e)),
            Err(Error::Offline(e)) => return Err(Error::Offline(e)),
            Err(_) => None,
        };
        let Some(principal) = principal else {
            return Ok(None);
        };
        Ok(self
            .multi("PROPFIND", &self.abs(&principal), "0", home_set_body)
            .ok()
            .and_then(|m| m.home))
    }

    /// Makes a calendar (`MKCALENDAR`, RFC 4791 §5.3.1) in the account's calendar home and returns its `href`.
    /// `components` are the item types it takes (`VEVENT`, `VTODO`). The app itself never creates calendars; the
    /// tests do.
    pub fn make_calendar(&self, name: &str, components: &[&str]) -> Result<String> {
        let home = self.home_href()?;
        let href = format!(
            "{}/{}/",
            home.trim_end_matches('/'),
            encode(&name.to_lowercase().replace(' ', "-"))
        );
        let comps: String = components
            .iter()
            .map(|c| format!(r#"<c:comp name="{c}"/>"#))
            .collect();
        let body = format!(
            r#"<?xml version="1.0"?><c:mkcalendar {DAV}><d:set><d:prop><d:displayname>{}</d:displayname><c:supported-calendar-component-set>{comps}</c:supported-calendar-component-set></d:prop></d:set></c:mkcalendar>"#,
            xml_escape(name)
        );
        let resp = self.request(
            "MKCALENDAR",
            &self.abs(&href),
            &[("Content-Type", "application/xml; charset=utf-8".into())],
            &body,
        )?;
        match resp.status() {
            200 | 201 => Ok(href),
            s => Err(Self::fail(s, &href)),
        }
    }

    /// Where the account's calendars are: asked of the account's principal, else the usual Nextcloud place.
    fn home_href(&self) -> Result<String> {
        if let Some(h) = self.home.lock().unwrap().clone() {
            return Ok(h);
        }
        let principal = format!(
            "{}/remote.php/dav/principals/users/{}/",
            self.root,
            encode(&self.user)
        );
        let body = format!(
            r#"<?xml version="1.0"?><d:propfind {DAV}><d:prop><c:calendar-home-set/></d:prop></d:propfind>"#
        );
        let found = match self.multi("PROPFIND", &principal, "0", &body) {
            Ok(m) => m.home,
            // a wrong password stops here; "forbidden" only means this is not where the account lives
            Err(Error::Auth(e)) if e.contains("401") => return Err(Error::Auth(e)),
            Err(Error::Offline(e)) => return Err(Error::Offline(e)),
            Err(_) => None,
        };
        // not a Nextcloud: ask the server's root who the user is (RFC 5397), then where that user's calendars are
        let found = match found {
            Some(h) => Some(h),
            None => self.discover_home(&body)?,
        };
        let home = found.unwrap_or_else(|| calendar_home_url(&self.root, &self.user));
        *self.home.lock().unwrap() = Some(home.clone());
        Ok(home)
    }
}

impl CalDavClient for HttpCalDav {
    fn calendars(&self) -> Result<Vec<Calendar>> {
        let home = self.home_href()?;
        let body = format!(
            r#"<?xml version="1.0"?><d:propfind {DAV}><d:prop><d:displayname/><d:resourcetype/><c:supported-calendar-component-set/></d:prop></d:propfind>"#
        );
        let m = self.multi("PROPFIND", &self.abs(&home), "1", &body)?;
        Ok(m.responses
            .into_iter()
            .filter(|r| r.is_calendar)
            .map(|r| Calendar {
                name: if r.name.is_empty() {
                    last_segment(&r.href)
                } else {
                    r.name
                },
                href: r.href,
                components: r.components,
            })
            .collect())
    }

    fn list(&self, calendar: &str) -> Result<Vec<ItemRef>> {
        let body = format!(
            r#"<?xml version="1.0"?><d:propfind {DAV}><d:prop><d:getetag/><d:resourcetype/></d:prop></d:propfind>"#
        );
        let m = self.multi("PROPFIND", &self.abs(calendar), "1", &body)?;
        Ok(m.responses
            .into_iter()
            .filter(|r| !r.is_collection && !r.etag.is_empty())
            .map(|r| ItemRef {
                name: last_segment(&r.href),
                etag: r.etag,
            })
            .collect())
    }

    fn get(&self, calendar: &str, names: &[String]) -> Result<Vec<ServerItem>> {
        let dir = format!("{}/", calendar.trim_end_matches('/'));
        let mut out = Vec::new();
        for chunk in names.chunks(50) {
            let hrefs: String = chunk
                .iter()
                .map(|n| {
                    format!(
                        "<d:href>{}</d:href>",
                        xml_escape(&format!("{dir}{}", encode(n)))
                    )
                })
                .collect();
            let body = format!(
                r#"<?xml version="1.0"?><c:calendar-multiget {DAV}><d:prop><d:getetag/><c:calendar-data/></d:prop>{hrefs}</c:calendar-multiget>"#
            );
            let m = self.multi("REPORT", &self.abs(calendar), "1", &body)?;
            for r in m.responses {
                let Resp {
                    href,
                    etag,
                    data,
                    status,
                    ..
                } = r;
                if status == Some(404) {
                    continue;
                }
                if let Some(ics) = data {
                    out.push(ServerItem {
                        name: last_segment(&href),
                        etag,
                        ics,
                    });
                }
            }
        }
        Ok(out)
    }

    fn put(&self, calendar: &str, name: &str, ics: &str, cond: &Cond) -> Result<String> {
        let url = format!(
            "{}/{}",
            self.abs(calendar.trim_end_matches('/')),
            encode(name)
        );
        let cond = match cond {
            Cond::Create => ("If-None-Match", "*".to_string()),
            Cond::IfMatch(e) => ("If-Match", format!("\"{}\"", norm_etag(e))),
        };
        let resp = self.request(
            "PUT",
            &url,
            &[
                cond,
                ("Content-Type", "text/calendar; charset=utf-8".into()),
            ],
            ics,
        )?;
        let status = resp.status();
        if !(200..300).contains(&status) {
            return Err(Self::fail(status, name));
        }
        if let Some(e) = resp.header("etag").or_else(|| resp.header("oc-etag")) {
            return Ok(norm_etag(e));
        }
        // some servers do not answer with the new ETag: ask
        Ok(self
            .list(calendar)?
            .into_iter()
            .find(|i| i.name == name)
            .map(|i| i.etag)
            .unwrap_or_default())
    }

    fn delete(&self, calendar: &str, name: &str, etag: Option<&str>) -> Result<()> {
        let url = format!(
            "{}/{}",
            self.abs(calendar.trim_end_matches('/')),
            encode(name)
        );
        let headers: Vec<(&str, String)> = etag
            .map(|e| vec![("If-Match", format!("\"{}\"", norm_etag(e)))])
            .unwrap_or_default();
        let status = self.request("DELETE", &url, &headers, "")?.status();
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(Self::fail(status, name))
        }
    }

    fn sync(&self, calendar: &str, token: Option<&str>) -> Result<SyncResult> {
        let tok = token.map(xml_escape).unwrap_or_default();
        let body = format!(
            r#"<?xml version="1.0"?><d:sync-collection {DAV}><d:sync-token>{tok}</d:sync-token><d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>"#
        );
        let resp = self.request(
            "REPORT",
            &self.abs(calendar),
            &[("Content-Type", "application/xml; charset=utf-8".into())],
            &body,
        )?;
        let status = resp.status();
        if status != 207 {
            // the server no longer knows the token (400, 403, 409, 410, 415, 507 depending on the server)
            return Err(
                if token.is_some() && matches!(status, 400 | 403 | 409 | 410 | 415 | 507) {
                    Error::Changed("the sync token is no longer valid".into())
                } else {
                    Self::fail(status, calendar)
                },
            );
        }
        let text = resp
            .into_string()
            .map_err(|e| Error::Offline(e.to_string()))?;
        let m = xml::parse(&text)?;
        let own = calendar.trim_end_matches('/');
        let (mut changed, mut deleted) = (Vec::new(), Vec::new());
        for r in m.responses {
            if r.href.trim_end_matches('/') == own || r.is_collection {
                continue;
            }
            let name = last_segment(&r.href);
            if r.status == Some(404) {
                deleted.push(name);
            } else if !r.etag.is_empty() {
                changed.push(ItemRef { name, etag: r.etag });
            }
        }
        Ok(SyncResult {
            token: m.sync_token.unwrap_or_default(),
            changed,
            deleted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_home_comes_from_any_address_of_the_server() {
        for s in [
            "http://localhost:8080",
            "http://localhost:8080/",
            "http://localhost:8080/remote.php/dav",
            "http://localhost:8080/remote.php/dav/files/notatkiA/Notes/graph",
        ] {
            assert_eq!(
                calendar_home_url(s, "notatkiA"),
                "http://localhost:8080/remote.php/dav/calendars/notatkiA/",
                "{s}"
            );
        }
        assert_eq!(
            calendar_home_url("https://c.example.org/nc", "a b"),
            "https://c.example.org/nc/remote.php/dav/calendars/a%20b/"
        );
    }

    #[test]
    fn plain_http_is_for_the_local_network_only() {
        assert!(HttpCalDav::new("http://localhost:8080", "u", "p").is_ok());
        assert!(matches!(
            HttpCalDav::new("http://cloud.example.org", "u", "p"),
            Err(Error::Url(_))
        ));
        assert!(matches!(HttpCalDav::new("", "u", "p"), Err(Error::Url(_))));
    }

    #[test]
    fn names_are_encoded_in_addresses_and_decoded_from_them() {
        assert_eq!(encode("a b+c.ics"), "a%20b%2Bc.ics");
        assert_eq!(last_segment("/cal/a%20b.ics"), "a b.ics");
        assert_eq!(last_segment("/cal/work/"), "work");
    }
}
