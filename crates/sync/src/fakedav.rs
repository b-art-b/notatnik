//! A small in-memory WebDAV server for tests: ETags that change like Nextcloud's (a folder's ETag changes when
//! anything below it does), `If-Match` and `If-None-Match`, Basic login, a request log, and switches to make it
//! behave badly. Not a real server: no locking, no properties beyond the three the client asks for.

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use tiny_http::{Header, Response, Server};

enum Node {
    Dir {
        etag: String,
        id: u64,
    },
    File {
        data: Vec<u8>,
        etag: String,
        /// Nextcloud's `oc:fileid`: it stays with the file when it is moved.
        id: u64,
    },
}

struct Inner {
    nodes: BTreeMap<String, Node>,
    counter: u64,
    next_id: u64,
    /// A change below a folder changes the folder's ETag (Nextcloud does).
    propagate: bool,
    /// Honour `If-Match` and `If-None-Match` (a bad server does not).
    conditions: bool,
    /// Answer every request with 503.
    down: bool,
    /// Answer `SEARCH` (a server that does not, answers 501).
    search: bool,
    log: Vec<String>,
}

impl Inner {
    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn next_etag(&mut self) -> String {
        self.counter += 1;
        format!("{:x}", self.counter * 7919 + 100_000)
    }

    /// Gives a new ETag to the folders above `path`.
    fn touch_parents(&mut self, path: &str) {
        if !self.propagate {
            return;
        }
        let mut p = path;
        while let Some((parent, _)) = p.rsplit_once('/') {
            self.retag(parent);
            p = parent;
        }
        self.retag("");
    }

    fn retag(&mut self, dir: &str) {
        let e = self.next_etag();
        if let Some(Node::Dir { etag, .. }) = self.nodes.get_mut(dir) {
            *etag = e;
        }
    }
}

/// The running server. Dropping it stops it.
pub struct FakeDav {
    /// `http://127.0.0.1:<port>/dav`: the folder `Notes/graph` below it is what a test syncs with.
    pub url: String,
    /// `http://127.0.0.1:<port>`: the server as a person types it; the account's files are below
    /// `/remote.php/dav/files/<user>`.
    pub server: String,
    /// The login.
    pub user: String,
    /// The login.
    pub password: String,
    inner: Arc<Mutex<Inner>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            seg.bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                        (b as char).to_string()
                    } else {
                        format!("%{b:02x}")
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn header(req: &tiny_http::Request, name: &'static str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str().to_string())
}

fn status(code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(Vec::new()).with_status_code(code)
}

fn handle(inner: &Mutex<Inner>, user: &str, password: &str, mut req: tiny_http::Request) {
    let method = req.method().as_str().to_string();
    let url = req.url().split('?').next().unwrap_or("").to_string();
    // reachable as `/dav/…` and, like Nextcloud, as `/remote.php/dav/files/<user>/…`
    let (mount, rest) = match url.strip_prefix("/remote.php/dav/files/") {
        Some(r) => {
            let (user, rest) = r.split_once('/').unwrap_or((r, ""));
            (format!("/remote.php/dav/files/{user}"), rest.to_string())
        }
        None => (
            "/dav".to_string(),
            url.strip_prefix("/dav").unwrap_or(&url).to_string(),
        ),
    };
    let path = decode(&rest).trim_matches('/').to_string();
    let mut body = Vec::new();
    let _ = req.as_reader().read_to_end(&mut body);
    let mut g = inner.lock().unwrap();
    g.log.push(format!("{method} {path}"));
    if g.down {
        let _ = req.respond(status(503));
        return;
    }
    let want = format!("Basic {}", base64(format!("{user}:{password}").as_bytes()));
    if header(&req, "authorization").as_deref() != Some(want.as_str()) {
        let _ = req.respond(status(401));
        return;
    }
    let parent = path.rsplit_once('/').map_or("", |(p, _)| p).to_string();
    let if_match = header(&req, "if-match").map(|s| s.trim_matches('"').to_string());
    let none_match = header(&req, "if-none-match");
    let conditions = g.conditions;
    let resp = match method.as_str() {
        "PROPFIND" => match g.nodes.get(&path) {
            None => status(404),
            Some(node) => {
                let depth1 = header(&req, "depth").as_deref() != Some("0");
                let mut xml = String::from(
                    "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\">",
                );
                let entry = |p: &str, n: &Node| -> String {
                    let href = format!(
                        "{mount}/{}{}",
                        encode(p),
                        if matches!(n, Node::Dir { .. }) && !p.is_empty() {
                            "/"
                        } else {
                            ""
                        }
                    );
                    // like Nextcloud (and unlike plain WebDAV), the `remote.php` mount numbers its items
                    let id = match n {
                        Node::Dir { id, .. } | Node::File { id, .. } => *id,
                    };
                    let fileid = if mount.starts_with("/remote.php") {
                        format!("<oc:fileid>{id}</oc:fileid>")
                    } else {
                        String::new()
                    };
                    match n {
                        Node::Dir { etag, .. } => format!(
                            "<d:response><d:href>{href}</d:href><d:propstat><d:prop><d:getetag>\"{etag}\"</d:getetag>{fileid}<d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
                        ),
                        Node::File { data, etag, .. } => format!(
                            "<d:response><d:href>{href}</d:href><d:propstat><d:prop><d:getetag>\"{etag}\"</d:getetag>{fileid}<d:getcontentlength>{}</d:getcontentlength><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                            data.len()
                        ),
                    }
                };
                xml.push_str(&entry(&path, node));
                if depth1 && matches!(node, Node::Dir { .. }) {
                    let prefix = if path.is_empty() {
                        String::new()
                    } else {
                        format!("{path}/")
                    };
                    for (p, n) in &g.nodes {
                        if p.len() > prefix.len()
                            && p.starts_with(&prefix)
                            && !p[prefix.len()..].contains('/')
                        {
                            xml.push_str(&entry(p, n));
                        }
                    }
                }
                xml.push_str("</d:multistatus>");
                Response::from_data(xml.into_bytes())
                    .with_status_code(207)
                    .with_header(Header::from_bytes("Content-Type", "application/xml").unwrap())
            }
        },
        "GET" => match g.nodes.get(&path) {
            Some(Node::File { data, etag, .. }) => Response::from_data(data.clone())
                .with_header(Header::from_bytes("ETag", format!("\"{etag}\"")).unwrap()),
            Some(Node::Dir { .. }) => status(405),
            None => status(404),
        },
        "PUT" => {
            if !matches!(g.nodes.get(&parent), Some(Node::Dir { .. })) {
                status(409)
            } else if matches!(g.nodes.get(&path), Some(Node::Dir { .. })) {
                status(405)
            } else {
                let existing = match g.nodes.get(&path) {
                    Some(Node::File { etag, .. }) => Some(etag.clone()),
                    _ => None,
                };
                let refused = conditions
                    && ((none_match.as_deref() == Some("*") && existing.is_some())
                        || (if_match.is_some() && existing != if_match));
                if refused {
                    status(412)
                } else {
                    let etag = g.next_etag();
                    let created = existing.is_none();
                    let id = match g.nodes.get(&path) {
                        Some(Node::File { id, .. }) => *id,
                        _ => g.new_id(),
                    };
                    g.nodes.insert(
                        path.clone(),
                        Node::File {
                            data: body,
                            etag: etag.clone(),
                            id,
                        },
                    );
                    g.touch_parents(&path);
                    status(if created { 201 } else { 204 })
                        .with_header(Header::from_bytes("ETag", format!("\"{etag}\"")).unwrap())
                }
            }
        }
        "MKCOL" => {
            if g.nodes.contains_key(&path) {
                status(405)
            } else if !path.is_empty() && !matches!(g.nodes.get(&parent), Some(Node::Dir { .. })) {
                status(409)
            } else {
                let etag = g.next_etag();
                let id = g.new_id();
                g.nodes.insert(path.clone(), Node::Dir { etag, id });
                g.touch_parents(&path);
                status(201)
            }
        }
        "DELETE" => match g.nodes.get(&path) {
            None => status(404),
            Some(node) => {
                let etag = match node {
                    Node::File { etag, .. } | Node::Dir { etag, .. } => etag.clone(),
                };
                if conditions && if_match.is_some() && if_match.as_deref() != Some(etag.as_str()) {
                    status(412)
                } else {
                    let prefix = format!("{path}/");
                    g.nodes.retain(|p, _| *p != path && !p.starts_with(&prefix));
                    g.touch_parents(&path);
                    status(204)
                }
            }
        },
        "MOVE" => {
            let dest = header(&req, "destination").map(|d| {
                let p = d
                    .split_once("://")
                    .and_then(|(_, r)| r.find('/').map(|i| r[i..].to_string()))
                    .unwrap_or(d);
                let p = decode(&p);
                match p.strip_prefix("/remote.php/dav/files/") {
                    Some(r) => r.split_once('/').map_or("", |(_, rest)| rest).to_string(),
                    None => p.strip_prefix("/dav").unwrap_or(&p).to_string(),
                }
                .trim_matches('/')
                .to_string()
            });
            match dest {
                Some(to)
                    if g.nodes.contains_key(&path)
                        && !g.nodes.contains_key(&to)
                        && matches!(
                            g.nodes.get(to.rsplit_once('/').map_or("", |(p, _)| p)),
                            Some(Node::Dir { .. })
                        ) =>
                {
                    let prefix = format!("{path}/");
                    let moving: Vec<String> = g
                        .nodes
                        .keys()
                        .filter(|p| **p == path || p.starts_with(&prefix))
                        .cloned()
                        .collect();
                    for old in moving {
                        let node = g.nodes.remove(&old).unwrap();
                        g.nodes.insert(format!("{to}{}", &old[path.len()..]), node);
                    }
                    g.touch_parents(&path);
                    g.touch_parents(&to);
                    status(201)
                }
                Some(_) => status(409),
                None => status(400),
            }
        }
        "SEARCH" => {
            if !g.search {
                status(501)
            } else {
                let text = String::from_utf8_lossy(&body).to_string();
                let wanted: Option<u64> = text
                    .split_once("<d:literal>")
                    .and_then(|(_, r)| r.split_once("</d:literal>"))
                    .and_then(|(n, _)| n.trim().parse().ok());
                let mut xml = String::from(
                    "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\">",
                );
                for (p, n) in &g.nodes {
                    if let Node::File { data, etag, id } = n
                        && Some(*id) == wanted
                    {
                        xml.push_str(&format!(
                            "<d:response><d:href>/remote.php/dav/files/{user}/{}</d:href><d:propstat><d:prop><d:getetag>\"{etag}\"</d:getetag><oc:fileid>{id}</oc:fileid><d:getcontentlength>{}</d:getcontentlength><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                            encode(p),
                            data.len()
                        ));
                    }
                }
                xml.push_str("</d:multistatus>");
                Response::from_data(xml.into_bytes())
                    .with_status_code(207)
                    .with_header(Header::from_bytes("Content-Type", "application/xml").unwrap())
            }
        }
        _ => status(405),
    };
    let _ = req.respond(resp);
}

fn base64(data: &[u8]) -> String {
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

impl FakeDav {
    /// Starts a server on a free local port, with the folder `Notes` already there (as on Nextcloud).
    pub fn start() -> FakeDav {
        let server = Server::http("127.0.0.1:0").expect("a free port");
        let port = server.server_addr().to_ip().map_or(0, |a| a.port());
        let mut nodes = BTreeMap::new();
        nodes.insert(
            String::new(),
            Node::Dir {
                etag: "1".into(),
                id: 1,
            },
        );
        nodes.insert(
            "Notes".into(),
            Node::Dir {
                etag: "2".into(),
                id: 2,
            },
        );
        let inner = Arc::new(Mutex::new(Inner {
            nodes,
            counter: 10,
            next_id: 100,
            propagate: true,
            conditions: true,
            down: false,
            search: true,
            log: Vec::new(),
        }));
        let (user, password) = ("tester".to_string(), "p@ss:wörd!".to_string());
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (inner, stop) = (inner.clone(), stop.clone());
            let (u, p) = (user.clone(), password.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(Some(req)) = server.recv_timeout(Duration::from_millis(50)) {
                        handle(&inner, &u, &p, req);
                    }
                }
            })
        };
        FakeDav {
            url: format!("http://127.0.0.1:{port}/dav"),
            server: format!("http://127.0.0.1:{port}"),
            user,
            password,
            inner,
            stop,
            thread: Some(thread),
        }
    }

    /// The URL of a folder below the server root, e.g. `folder_url("Notes/graph")`.
    pub fn folder_url(&self, path: &str) -> String {
        format!("{}/{}", self.url, path.trim_matches('/'))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// Writes a file as another device would have (parents are created).
    pub fn put_file(&self, path: &str, data: &[u8]) {
        let mut g = self.lock();
        let mut dir = String::new();
        let parts: Vec<&str> = path.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !dir.is_empty() {
                dir.push('/');
            }
            dir.push_str(part);
            if !g.nodes.contains_key(&dir) {
                let etag = g.next_etag();
                let id = g.new_id();
                g.nodes.insert(dir.clone(), Node::Dir { etag, id });
            }
        }
        let etag = g.next_etag();
        let id = match g.nodes.get(path) {
            Some(Node::File { id, .. }) => *id,
            _ => g.new_id(),
        };
        g.nodes.insert(
            path.to_string(),
            Node::File {
                data: data.to_vec(),
                etag,
                id,
            },
        );
        g.touch_parents(path);
    }

    /// Deletes a file or folder as another device would have.
    pub fn delete_path(&self, path: &str) {
        let mut g = self.lock();
        let prefix = format!("{path}/");
        g.nodes.retain(|p, _| p != path && !p.starts_with(&prefix));
        g.touch_parents(path);
    }

    /// A file's content.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        match self.lock().nodes.get(path) {
            Some(Node::File { data, .. }) => Some(data.clone()),
            _ => None,
        }
    }

    /// Paths of all files below `dir`.
    pub fn files(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{}/", dir.trim_matches('/'));
        self.lock()
            .nodes
            .iter()
            .filter(|(p, n)| matches!(n, Node::File { .. }) && p.starts_with(&prefix))
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Folder ETags change (or not) when something below changes.
    pub fn set_propagate(&self, on: bool) {
        self.lock().propagate = on;
    }

    /// `If-Match` and `If-None-Match` are honoured (or ignored, like a bad server does).
    pub fn set_conditions(&self, on: bool) {
        self.lock().conditions = on;
    }

    /// `SEARCH` is answered (or not: 501, like a server that does not search).
    pub fn set_search(&self, on: bool) {
        self.lock().search = on;
    }

    /// Moves a file or folder as another device would have; its Nextcloud number goes with it.
    pub fn move_path(&self, from: &str, to: &str) {
        let mut g = self.lock();
        let prefix = format!("{from}/");
        let moving: Vec<String> = g
            .nodes
            .keys()
            .filter(|p| *p == from || p.starts_with(&prefix))
            .cloned()
            .collect();
        for old in moving {
            let node = g.nodes.remove(&old).unwrap();
            g.nodes.insert(format!("{to}{}", &old[from.len()..]), node);
        }
        g.touch_parents(from);
        g.touch_parents(to);
    }

    /// The Nextcloud number of a file.
    pub fn file_id(&self, path: &str) -> Option<u64> {
        match self.lock().nodes.get(path) {
            Some(Node::File { id, .. }) => Some(*id),
            _ => None,
        }
    }

    /// Every request is answered with 503.
    pub fn set_down(&self, down: bool) {
        self.lock().down = down;
    }

    /// The requests so far, as `METHOD path`.
    pub fn requests(&self) -> Vec<String> {
        self.lock().log.clone()
    }

    /// Forgets the request log.
    pub fn clear_requests(&self) {
        self.lock().log.clear();
    }
}

impl Drop for FakeDav {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Reads a whole reader (helper for tests).
pub fn read_all(mut r: impl Read) -> Vec<u8> {
    let mut v = Vec::new();
    let _ = r.read_to_end(&mut v);
    v
}
