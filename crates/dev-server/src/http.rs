//! A very small HTTP/1.1 server for the dev backend: one thread per connection, one request per connection
//! (`Connection: close`), no keep-alive.
//!
//! This replaces `tiny_http`, whose connection pool counts idle threads without locking: when a browser opens
//! several connections at once, one of them can wait in the queue for as long as another stays open, and the
//! request on it never gets an answer (a browser test then fails for no reason of its own).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

/// What the handler gets: the method, the address with its query, and the body.
pub struct Request {
    pub method: String,
    pub url: String,
    pub body: Vec<u8>,
}

/// What the handler answers.
pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(status: u16, v: &serde_json::Value) -> Reply {
        Reply {
            status,
            content_type: "application/json".into(),
            body: v.to_string().into_bytes(),
        }
    }

    pub fn text(status: u16, text: &str) -> Reply {
        Reply {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            body: text.as_bytes().to_vec(),
        }
    }

    pub fn empty(status: u16) -> Reply {
        Reply::text(status, "")
    }
}

/// Bodies larger than this are refused (the tests send a few kilobytes).
const MAX_BODY: usize = 64 * 1024 * 1024;

/// Reads one request from the stream; `None` when the client sent nothing usable.
fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, url) = (parts.next()?.to_string(), parts.next()?.to_string());
    let mut length = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().ok()?;
            }
        }
    }
    if length > MAX_BODY {
        return None;
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request { method, url, body })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Status",
    }
}

fn write_reply(mut stream: &TcpStream, r: &Reply) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\
         Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\r\n",
        r.status,
        reason(r.status),
        r.content_type,
        r.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&r.body)?;
    stream.flush()
}

/// Answers every request with `handler`, each connection on a thread of its own. Never returns.
pub fn serve<H>(listener: &TcpListener, handler: H)
where
    H: Fn(Request) -> Reply + Clone + Send + 'static,
{
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let handler = handler.clone();
        std::thread::spawn(move || {
            // a connection that sends nothing (a browser's spare one) must not keep its thread for ever
            let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
            if let Some(req) = read_request(&stream) {
                let _ = write_reply(&stream, &handler(req));
            }
            let _ = stream.shutdown(Shutdown::Both);
        });
    }
}
