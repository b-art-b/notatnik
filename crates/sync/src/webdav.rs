//! WebDAV client for Nextcloud and other servers (SY-2): PROPFIND to list, GET, PUT with `If-Match` or
//! `If-None-Match`, MKCOL, DELETE.

use std::io::Read as _;
use std::time::Duration;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::remote::{Cond, Entry, Listing, Remote, norm_etag};
use crate::{Error, Result};

const PROPFIND_BODY: &str = r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns"><d:prop><d:getetag/><d:getcontentlength/><d:resourcetype/><oc:fileid/></d:prop></d:propfind>"#;

/// A WebDAV folder with a login.
pub struct WebDavRemote {
    agent: ureq::Agent,
    /// The folder, e.g. `http://localhost:8080/remote.php/dav/files/me/Notes/graph`, no trailing slash.
    base: String,
    auth: String,
}

/// Plain `http` is for this computer and the local network only; anything else needs `https` (SY-2), because the
/// password and the notes would cross the internet in the clear.
pub fn check_url(url: &str) -> Result<()> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("https://") {
        return Ok(());
    }
    let Some(rest) = lower.strip_prefix("http://") else {
        return Err(Error::Url(
            "the address must start with https:// (or http:// for this computer and the local network)".into(),
        ));
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = if host.starts_with('[') {
        host.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        host.split(':').next().unwrap_or("")
    };
    if is_local_host(host) {
        Ok(())
    } else {
        Err(Error::Url(format!(
            "`http://{host}` would send the password unprotected; use https:// (plain http only works for this computer and the local network)"
        )))
    }
}

/// The address of the folder `folder` of a Nextcloud account from what a person types: the server (`https://cloud.example.org`,
/// or the full WebDAV address `…/remote.php/dav/files/me`), the login name and a folder like `Notes/graph`.
pub fn folder_url(server: &str, user: &str, folder: &str) -> Result<String> {
    let server = server.trim().trim_end_matches('/');
    if server.is_empty() {
        return Err(Error::Url("the server address is empty".into()));
    }
    let mut url = if server.contains("/remote.php/dav/files/") {
        server.to_string()
    } else {
        if user.trim().is_empty() {
            return Err(Error::Url("the user name is empty".into()));
        }
        format!(
            "{server}/remote.php/dav/files/{}",
            encode_segment(user.trim())
        )
    };
    for seg in folder.split('/').map(str::trim).filter(|s| !s.is_empty()) {
        if seg == "." || seg == ".." {
            return Err(Error::Url(
                "the folder name cannot contain `.` or `..`".into(),
            ));
        }
        url.push('/');
        url.push_str(&encode_segment(seg));
    }
    check_url(&url)?;
    Ok(url)
}

/// The address of `path` (segments joined with `/`, below `root`) as a link, each segment percent-encoded.
pub fn item_url(root: &str, path: &str) -> String {
    let mut u = root.trim_end_matches('/').to_string();
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        u.push('/');
        u.push_str(&encode_segment(seg));
    }
    u
}

/// The address of a Nextcloud's web interface, from a WebDAV address of one of its accounts
/// (`https://cloud.example.org/remote.php/dav/files/me/Notes` gives `https://cloud.example.org`). `None` for a
/// server that is not laid out like Nextcloud.
pub fn web_base(dav_url: &str) -> Option<String> {
    dav_url
        .split_once("/remote.php/dav/files/")
        .map(|(base, _)| base.to_string())
}

fn is_local_host(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".local") || host == "::1" {
        return true;
    }
    let parts: Vec<u8> = host.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 4 || host.split('.').count() != 4 {
        return false;
    }
    matches!(
        (parts[0], parts[1]),
        (127, _) | (10, _) | (192, 168) | (172, 16..=31) | (169, 254)
    )
}

fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
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
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl WebDavRemote {
    /// Makes the folder at `url`; when the folder above it is missing, makes that first (at most a few levels, and
    /// never the account's root).
    fn make_with_parents(&self, url: &str, depth: u8) -> Result<()> {
        let resp = self.request("MKCOL", url, &[], &[])?;
        match resp.status() {
            200..=299 | 405 => Ok(()),
            409 => {
                let parent = url.trim_end_matches('/').rsplit_once('/').map(|(p, _)| p);
                let at_root = |p: &str| {
                    // `…/files/<user>` is the account's files; `scheme://host` has no folders to make
                    p.rsplit_once("/files/")
                        .is_some_and(|(_, rest)| !rest.contains('/'))
                        || p.matches('/').count() <= 2
                };
                match parent {
                    Some(p) if depth < 6 && !at_root(p) => {
                        self.make_with_parents(p, depth + 1)?;
                        self.make_with_parents(url, depth + 1)
                    }
                    _ => Err(Error::NotFound(format!(
                        "the folder above `{url}` does not exist on the server"
                    ))),
                }
            }
            s => Err(self.fail(s, url)),
        }
    }
    /// A client for the folder at `url`, logging in as `user` (an app password is best on Nextcloud).
    pub fn new(url: &str, user: &str, password: &str) -> Result<WebDavRemote> {
        check_url(url)?;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build();
        Ok(WebDavRemote {
            agent,
            base: url.trim().trim_end_matches('/').to_string(),
            auth: format!("Basic {}", b64(format!("{user}:{password}").as_bytes())),
        })
    }

    fn url(&self, path: &str, dir: bool) -> String {
        let mut u = self.base.clone();
        for seg in path.split('/').filter(|s| !s.is_empty()) {
            u.push('/');
            u.push_str(&encode_segment(seg));
        }
        if dir && !path.is_empty() {
            u.push('/');
        }
        u
    }

    /// Sends a request and returns the answer whatever its status; only a failure to talk to the server is an
    /// error here.
    fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<ureq::Response> {
        let mut req = self
            .agent
            .request(method, url)
            .set("Authorization", &self.auth);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        match req.send_bytes(body) {
            Ok(r) | Err(ureq::Error::Status(_, r)) => Ok(r),
            Err(ureq::Error::Transport(t)) => Err(Error::Offline(t.to_string())),
        }
    }

    /// Maps the status of a response that was not a success.
    fn fail(&self, status: u16, what: &str) -> Error {
        match status {
            401 | 403 => Error::Auth(format!("HTTP {status}")),
            404 => Error::NotFound(what.to_string()),
            409 | 412 => Error::Changed(what.to_string()),
            507 => Error::Server("the server has no space left for this account (HTTP 507)".into()),
            423 => Error::Server(format!("{what} is locked on the server (HTTP 423)")),
            s => Error::Server(format!("HTTP {s} for {what}")),
        }
    }

    fn etag_of(&self, path: &str) -> Result<String> {
        let parent = path.rsplit_once('/').map_or("", |(p, _)| p);
        let name = path.rsplit('/').next().unwrap_or(path);
        let l = self
            .list(parent)?
            .ok_or_else(|| Error::NotFound(path.into()))?;
        l.entries
            .into_iter()
            .find(|e| e.name == name)
            .map(|e| e.etag)
            .ok_or_else(|| Error::NotFound(path.into()))
    }
}

impl Remote for WebDavRemote {
    fn list(&self, dir: &str) -> Result<Option<Listing>> {
        let url = self.url(dir, true);
        let resp = self.request(
            "PROPFIND",
            &url,
            &[
                ("Depth", "1".into()),
                ("Content-Type", "application/xml".into()),
            ],
            PROPFIND_BODY.as_bytes(),
        )?;
        let status = resp.status();
        if status == 404 {
            return Ok(None);
        }
        if status != 207 {
            return Err(self.fail(status, dir));
        }
        let xml = resp
            .into_string()
            .map_err(|e| Error::Offline(e.to_string()))?;
        // the request path as the server writes it back in `href`
        let prefix = decode(
            url.split_once("://")
                .and_then(|(_, r)| r.find('/').map(|i| &r[i..]))
                .unwrap_or("/"),
        );
        parse_multistatus(&xml, &prefix).map(Some)
    }

    fn get(&self, path: &str) -> Result<(Vec<u8>, String)> {
        let resp = self.request("GET", &self.url(path, false), &[], &[])?;
        let status = resp.status();
        if status != 200 {
            return Err(self.fail(status, path));
        }
        let etag = resp.header("etag").map(norm_etag).unwrap_or_default();
        let mut bytes = Vec::new();
        resp.into_reader()
            .take(1 << 30)
            .read_to_end(&mut bytes)
            .map_err(|e| Error::Offline(e.to_string()))?;
        Ok((bytes, etag))
    }

    fn put(&self, path: &str, data: &[u8], cond: Cond) -> Result<String> {
        let header = match &cond {
            Cond::Create => ("If-None-Match", "*".to_string()),
            Cond::Match(e) => ("If-Match", format!("\"{}\"", norm_etag(e))),
        };
        let resp = self.request(
            "PUT",
            &self.url(path, false),
            &[header, ("X-OC-MTime", now_secs().to_string())],
            data,
        )?;
        let status = resp.status();
        if !(200..300).contains(&status) {
            return Err(self.fail(status, path));
        }
        match resp.header("etag").or_else(|| resp.header("oc-etag")) {
            Some(e) => Ok(norm_etag(e)),
            None => self.etag_of(path),
        }
    }

    fn mkdir(&self, dir: &str) -> Result<()> {
        // create the folders from the top; an existing one answers 405
        let mut sofar = String::new();
        let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
        if dir.is_empty() {
            parts.clear();
        }
        // the synced folder itself may be missing, and so may the folders above it that were chosen with it
        // (`Notes/Work/graph`); the account's own root is never made
        let create = |p: &str| -> Result<()> {
            let resp = self.request("MKCOL", &self.url(p, true), &[], &[])?;
            match resp.status() {
                200..=299 | 405 => Ok(()),
                409 => Err(Error::NotFound(format!(
                    "the folder above `{p}` does not exist on the server; create it first"
                ))),
                s => Err(self.fail(s, p)),
            }
        };
        if self.list("")?.is_none() {
            self.make_with_parents(&self.url("", true), 0)?;
        }
        for part in parts {
            if !sofar.is_empty() {
                sofar.push('/');
            }
            sofar.push_str(part);
            create(&sofar)?;
        }
        Ok(())
    }

    fn delete(&self, path: &str, if_match: Option<&str>) -> Result<()> {
        let mut headers = Vec::new();
        if let Some(e) = if_match {
            headers.push(("If-Match", format!("\"{}\"", norm_etag(e))));
        }
        let resp = self.request("DELETE", &self.url(path, false), &headers, &[])?;
        match resp.status() {
            200..=299 | 404 => Ok(()),
            s => Err(self.fail(s, path)),
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The `Authorization` header value for a login (HTTP Basic); the calendar client sends the same one.
pub fn basic_auth(user: &str, password: &str) -> String {
    format!("Basic {}", b64(format!("{user}:{password}").as_bytes()))
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Reads a `207 Multi-Status` answer of a `Depth: 1` PROPFIND. `prefix` is the decoded path of the folder that was
/// asked about; the response for that path gives the folder's own ETag, the others are its entries.
fn parse_multistatus(xml: &str, prefix: &str) -> Result<Listing> {
    #[derive(Default)]
    struct Item {
        href: String,
        etag: String,
        size: u64,
        dir: bool,
        file_id: String,
    }
    let mut reader = Reader::from_str(xml);
    let mut items: Vec<Item> = Vec::new();
    let mut cur: Option<Item> = None;
    let mut field = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if name == "response" {
                    cur = Some(Item::default());
                }
                field = name;
            }
            Ok(Event::Empty(e)) => {
                if e.local_name().as_ref() == b"collection" {
                    if let Some(c) = cur.as_mut() {
                        c.dir = true;
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let Ok(text) = t.decode() else { continue };
                let text = quick_xml::escape::unescape(&text)
                    .map_or_else(|_| text.to_string(), |c| c.into_owned());
                if let Some(c) = cur.as_mut() {
                    match field.as_str() {
                        "href" => c.href.push_str(text.trim()),
                        "getetag" => c.etag.push_str(&norm_etag(&text)),
                        "getcontentlength" => c.size = text.trim().parse().unwrap_or(0),
                        "fileid" => c.file_id.push_str(text.trim()),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"response" {
                    items.extend(cur.take());
                }
                field.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Server(format!("unreadable answer: {e}"))),
            _ => {}
        }
    }
    let own = prefix.trim_matches('/');
    let mut found_self = false;
    let mut listing = Listing {
        etag: String::new(),
        entries: Vec::new(),
    };
    for it in items {
        let href = decode(&it.href);
        // the href may be a full URL
        let path = href
            .split_once("://")
            .and_then(|(_, r)| r.find('/').map(|i| &r[i..]))
            .unwrap_or(&href);
        let path = path.trim_matches('/');
        if path == own {
            found_self = true;
            listing.etag = it.etag;
            continue;
        }
        let Some(rest) = path
            .strip_prefix(own)
            .map(|r| r.trim_start_matches('/'))
            .filter(|r| !r.is_empty() && !r.contains('/'))
        else {
            continue;
        };
        listing.entries.push(Entry {
            name: rest.to_string(),
            dir: it.dir,
            etag: it.etag,
            size: it.size,
            file_id: it.file_id,
        });
    }
    // an answer that does not mention the folder itself was not understood; taking it for an empty folder
    // would make the sync delete everything
    if !found_self {
        return Err(Error::Server(
            "the server's answer does not describe the folder that was asked for".into(),
        ));
    }
    Ok(listing)
}

/// What the connection test learned about a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// The ETag of a folder changes whenever something below it changes (Nextcloud does this): the engine can then
    /// skip folders whose ETag is unchanged.
    pub trusts_folder_etags: bool,
}

/// Checks that the folder can be written the way the engine needs, and what the server guarantees: creates the
/// folder if needed, writes and deletes a small file under `.notatnik/`, and refuses a server that ignores
/// `If-Match` (silently overwriting would lose edits).
pub fn probe(remote: &dyn Remote) -> Result<Probe> {
    remote.mkdir("")?;
    remote.mkdir(".notatnik")?;
    let before = remote
        .list("")?
        .ok_or_else(|| Error::NotFound(String::new()))?
        .etag;
    let path = ".notatnik/connection-test";
    // a leftover of an earlier test must not make the test fail
    remote.delete(path, None)?;
    let etag = remote.put(path, b"notatnik connection test", Cond::Create)?;
    let result = (|| {
        match remote.put(path, b"x", Cond::Create) {
            Err(Error::Changed(_)) => {}
            Ok(_) => return Err(Error::Server("the server overwrote a file that exists although the request said not to (If-None-Match is ignored); syncing with it could lose edits".into())),
            Err(e) => return Err(e),
        }
        match remote.put(path, b"x", Cond::Match("not-the-etag".into())) {
            Err(Error::Changed(_)) => {}
            Ok(_) => return Err(Error::Server("the server overwrote a file with a wrong ETag (If-Match is ignored); syncing with it could lose edits".into())),
            Err(e) => return Err(e),
        }
        let after = remote.list("")?.map(|l| l.etag).unwrap_or_default();
        Ok(Probe {
            trusts_folder_etags: !before.is_empty() && after != before,
        })
    })();
    let _ = remote.delete(path, Some(&etag));
    result
}

#[cfg(test)]
mod link_tests {
    use super::*;

    #[test]
    fn the_listing_carries_nextclouds_file_id_when_there_is_one() {
        let xml = r#"<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
<d:response><d:href>/remote.php/dav/files/me/D/</d:href><d:propstat><d:prop><d:getetag>"e0"</d:getetag><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response>
<d:response><d:href>/remote.php/dav/files/me/D/a.pdf</d:href><d:propstat><d:prop><d:getetag>"e1"</d:getetag><oc:fileid>209</oc:fileid><d:getcontentlength>5</d:getcontentlength><d:resourcetype/></d:prop></d:propstat></d:response>
<d:response><d:href>/remote.php/dav/files/me/D/b.pdf</d:href><d:propstat><d:prop><d:getetag>"e2"</d:getetag><d:getcontentlength>7</d:getcontentlength><d:resourcetype/></d:prop></d:propstat></d:response>
</d:multistatus>"#;
        let l = parse_multistatus(xml, "/remote.php/dav/files/me/D/").unwrap();
        let id = |n: &str| {
            l.entries
                .iter()
                .find(|e| e.name == n)
                .unwrap()
                .file_id
                .clone()
        };
        assert_eq!(id("a.pdf"), "209");
        assert_eq!(id("b.pdf"), "", "a server without file IDs leaves it empty");
    }

    #[test]
    fn links_by_path_are_encoded_and_the_web_address_comes_from_the_dav_address() {
        assert_eq!(
            item_url("http://h/remote.php/dav/files/me", "My Docs/a (1).pdf"),
            "http://h/remote.php/dav/files/me/My%20Docs/a%20%281%29.pdf"
        );
        assert_eq!(
            web_base("https://c.example.org/nc/remote.php/dav/files/me/Notes"),
            Some("https://c.example.org/nc".into())
        );
        assert_eq!(web_base("https://dav.example.org/files"), None);
    }
}
