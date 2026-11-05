//! W1: clipping against fixture pages served by a tiny HTTP server on this computer (`allow_private`), and the refusals
//! with it switched off. No test reaches the internet.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use notatnik_clip::{ClipError, ClipPolicy, Note, Progress, clip};

// ------------------------------------------------------------------------------------------------ a small server

#[derive(Clone)]
enum Route {
    Ok(&'static str, Vec<u8>),
    Redirect(String),
    Status(u16),
    /// Answers after this long.
    Slow(Duration),
    /// This many bytes with the given type.
    Big(&'static str, usize),
}

struct Server {
    addr: SocketAddr,
    hits: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn start(routes: HashMap<String, Route>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes.clone();
                let log = log.clone();
                std::thread::spawn(move || serve(stream, &routes, &log));
            }
        });
        Server { addr, hits }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.addr.port())
    }

    fn hits(&self) -> Vec<String> {
        self.hits.lock().unwrap().clone()
    }
}

fn serve(mut stream: TcpStream, routes: &HashMap<String, Route>, log: &Mutex<Vec<String>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let path = head
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    log.lock().unwrap().push(path.clone());
    let write = |s: &mut TcpStream, status: &str, extra: &str, body: &[u8]| {
        let _ = write!(
            s,
            "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = s.write_all(body);
    };
    match routes.get(&path) {
        Some(Route::Ok(ct, body)) => write(
            &mut stream,
            "200 OK",
            &format!("Content-Type: {ct}\r\n"),
            body,
        ),
        Some(Route::Redirect(to)) => write(
            &mut stream,
            "302 Found",
            &format!("Location: {to}\r\n"),
            b"",
        ),
        Some(Route::Status(c)) => write(&mut stream, &format!("{c} Error"), "", b"error"),
        Some(Route::Slow(d)) => {
            std::thread::sleep(*d);
            write(
                &mut stream,
                "200 OK",
                "Content-Type: text/html\r\n",
                b"<html>late</html>",
            );
        }
        Some(Route::Big(ct, n)) => {
            let mut body = if *ct == "image/png" {
                PNG[..8].to_vec()
            } else {
                Vec::new()
            };
            body.resize(*n, b'a');
            write(
                &mut stream,
                "200 OK",
                &format!("Content-Type: {ct}\r\n"),
                &body,
            );
        }
        None => write(
            &mut stream,
            "404 Not Found",
            "Content-Type: text/plain\r\n",
            b"not found",
        ),
    }
}

/// A 1x1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xfc, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x85, 0x02, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];
const JPEG: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0xff, 0xd9,
];

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/web")
            .join(name),
    )
    .unwrap()
}

fn html(name: &str) -> Route {
    Route::Ok("text/html; charset=utf-8", fixture(name))
}

fn local() -> ClipPolicy {
    ClipPolicy {
        allow_private: true,
        ..ClipPolicy::default()
    }
}

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 5, h, m, 0).unwrap()
}

fn run(url: &str, policy: &ClipPolicy) -> Result<notatnik_clip::Clip, ClipError> {
    clip(url, policy, at(10, 0), &mut |_| true)
}

// ------------------------------------------------------------------------------------------------ reading a book

/// The entries of the zip, in order, with their contents.
fn entries(epub: &[u8]) -> Vec<(String, Vec<u8>, bool)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(epub.to_vec())).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            (
                f.name().to_string(),
                data,
                f.compression() == zip::CompressionMethod::Stored,
            )
        })
        .collect()
}

fn entry(epub: &[u8], name: &str) -> String {
    String::from_utf8(
        entries(epub)
            .into_iter()
            .find(|e| e.0 == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .1,
    )
    .unwrap()
}

/// Every XML document in the book is well formed (the tags nest, the entities are known).
fn assert_well_formed(xml: &str, what: &str) {
    let mut r = quick_xml::Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(quick_xml::events::Event::GeneralRef(r)) => {
                // XML knows five entities and numbers; `&nbsp;` and the like would break an EPUB
                let name = r
                    .decode()
                    .unwrap_or_else(|e| panic!("{what}: bad reference: {e}"));
                assert!(
                    matches!(name.as_ref(), "amp" | "lt" | "gt" | "quot" | "apos")
                        || name.starts_with('#'),
                    "{what}: unknown entity &{name};"
                );
            }
            Ok(_) => {}
            Err(e) => panic!("{what} is not well formed: {e}\n{xml}"),
        }
    }
}

/// A page too big for Readability (more than 30 000 elements): the fallback path.
fn words_page() -> Vec<u8> {
    let mut p = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Just words</title></head><body><div>",
    );
    for _ in 0..31_000 {
        p.push_str("<span>word</span> ");
    }
    p.push_str("<span>and nothing like an article</span></div></body></html>");
    p.into_bytes()
}

fn pages(extra: &[(&str, Route)]) -> HashMap<String, Route> {
    let mut m: HashMap<String, Route> = HashMap::new();
    m.insert("/article".into(), html("article.html"));
    m.insert("/img/pic1.png".into(), Route::Ok("image/png", PNG.to_vec()));
    m.insert(
        "/img/pic2.jpg".into(),
        Route::Ok("image/jpeg", JPEG.to_vec()),
    );
    for (k, v) in extra {
        m.insert((*k).into(), v.clone());
    }
    m
}

// ------------------------------------------------------------------------------------------------ the article

#[test]
fn an_article_becomes_a_valid_book_without_the_page_around_it() {
    let s = Server::start(pages(&[]));
    let c = run(&s.url("/article"), &local()).unwrap();
    // `CLIP_DUMP=/some/dir cargo test …` keeps the book, to open it in a reader
    if let Ok(dir) = std::env::var("CLIP_DUMP") {
        std::fs::write(std::path::Path::new(&dir).join(&c.file_name), &c.epub).unwrap();
    }
    assert_eq!(c.title, "The Lighthouse Keeper Nobody Asked For");
    assert_eq!(c.byline.as_deref(), Some("Anna Kowalska"));
    assert_eq!(c.date.as_deref(), Some("2026-03-14"));
    assert_eq!(c.language, "en");
    assert_eq!(c.source, s.url("/article"));
    assert!(
        c.file_name
            .starts_with("the-lighthouse-keeper-nobody-asked-for-")
            && c.file_name.ends_with(".epub"),
        "{}",
        c.file_name
    );
    assert!(c.notes.is_empty(), "{:?}", c.notes);

    let list = entries(&c.epub);
    // the first entry is `mimetype`, stored
    assert_eq!(list[0].0, "mimetype");
    assert_eq!(list[0].1, b"application/epub+zip");
    assert!(list[0].2, "mimetype must be stored");
    let names: Vec<&str> = list.iter().map(|e| e.0.as_str()).collect();
    for want in [
        "META-INF/container.xml",
        "OEBPS/content.opf",
        "OEBPS/nav.xhtml",
        "OEBPS/chapter.xhtml",
        "OEBPS/style.css",
    ] {
        assert!(names.contains(&want), "{want} missing in {names:?}");
    }
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("OEBPS/images/"))
            .count(),
        2,
        "{names:?}"
    );

    for xml in [
        "META-INF/container.xml",
        "OEBPS/content.opf",
        "OEBPS/nav.xhtml",
        "OEBPS/chapter.xhtml",
    ] {
        assert_well_formed(&entry(&c.epub, xml), xml);
    }
    let opf = entry(&c.epub, "OEBPS/content.opf");
    for want in [
        "<dc:title>The Lighthouse Keeper Nobody Asked For</dc:title>",
        "<dc:creator>Anna Kowalska</dc:creator>",
        "<dc:language>en</dc:language>",
        &format!("<dc:source>{}</dc:source>", s.url("/article")),
        "<dc:date>2026-03-14</dc:date>",
        "dcterms:modified\">2026-11-05T10:00:00Z<",
        "urn:uuid:",
    ] {
        assert!(opf.contains(want), "{want} not in {opf}");
    }

    let chapter = entry(&c.epub, "OEBPS/chapter.xhtml");
    // the story is there ...
    for text in [
        "hundred and forty steps",
        "nobody asked",
        "The council reversed its decision in May",
    ] {
        assert!(chapter.contains(text), "{text}");
    }
    // ... and the navigation, the sidebar, the comments, the footer and the scripts are not
    for junk in [
        "Most read",
        "Ten tips",
        "Comments (3)",
        "Subscribe to my channel",
        "All rights reserved",
        "Privacy",
        "tracker.invalid",
        "<script",
    ] {
        assert!(!chapter.contains(junk), "{junk} should have been left out");
    }
    assert!(chapter.contains("Clipped from"), "{chapter}");
    // the pictures are inside the book and the chapter points at them
    assert!(
        chapter.contains("src=\"images/img0.png\"") && chapter.contains("src=\"images/img1.jpg\""),
        "{chapter}"
    );
    assert!(
        !chapter.contains("127.0.0.1/img") && !chapter.contains("/img/pic1.png"),
        "{chapter}"
    );
}

#[test]
fn the_same_input_gives_the_same_bytes_and_a_later_time_changes_only_its_meta() {
    let s = Server::start(pages(&[]));
    let a = clip(&s.url("/article"), &local(), at(10, 0), &mut |_| true).unwrap();
    let b = clip(&s.url("/article"), &local(), at(10, 0), &mut |_| true).unwrap();
    assert_eq!(a.epub, b.epub, "same input, same bytes");
    assert_eq!(a.file_name, b.file_name);
    // a minute later, the same day
    let c = clip(&s.url("/article"), &local(), at(10, 1), &mut |_| true).unwrap();
    assert_ne!(a.epub, c.epub);
    assert_eq!(
        a.file_name, c.file_name,
        "the name comes from the address, not the time"
    );
    let (ea, ec) = (entries(&a.epub), entries(&c.epub));
    assert_eq!(ea.len(), ec.len());
    for (x, y) in ea.iter().zip(&ec) {
        if x.0 == "OEBPS/content.opf" {
            assert_eq!(
                String::from_utf8_lossy(&x.1).replace("10:00:00Z", "10:01:00Z"),
                String::from_utf8_lossy(&y.1),
                "only the time of the clip differs"
            );
        } else {
            assert_eq!(x.1, y.1, "{} must not change", x.0);
        }
    }
}

#[test]
fn another_address_gives_another_file_name_and_a_fragment_does_not() {
    let mut routes = pages(&[]);
    routes.insert("/article2".into(), html("article.html"));
    let s = Server::start(routes);
    let a = run(&s.url("/article"), &local()).unwrap();
    let a2 = run(&format!("{}#section", s.url("/article")), &local()).unwrap();
    let b = run(&s.url("/article2"), &local()).unwrap();
    assert_eq!(a.file_name, a2.file_name);
    assert_ne!(a.file_name, b.file_name);
    // the same title, so the same slug, told apart by the hash of the address
    assert_eq!(
        a.file_name.rsplit_once('-').unwrap().0,
        b.file_name.rsplit_once('-').unwrap().0
    );
}

#[test]
fn a_page_in_another_character_set_comes_out_right() {
    let s = Server::start(HashMap::from([(
        "/pl".to_string(),
        Route::Ok("text/html", fixture("latin2.html")),
    )]));
    let c = run(&s.url("/pl"), &local()).unwrap();
    assert_eq!(c.title, "Żółć i gęślą");
    assert_eq!(c.language, "pl");
    let chapter = entry(&c.epub, "OEBPS/chapter.xhtml");
    assert!(chapter.contains("Zażółć gęślą jaźń"), "{chapter}");
}

#[test]
fn relative_links_and_pictures_follow_the_page_and_its_base() {
    let s = Server::start(HashMap::from([
        ("/dir/base".to_string(), html("base.html")),
        (
            "/media/pic.png".to_string(),
            Route::Ok("image/png", PNG.to_vec()),
        ),
    ]));
    let c = run(&s.url("/dir/base"), &local()).unwrap();
    let chapter = entry(&c.epub, "OEBPS/chapter.xhtml");
    // `<base href="/media/">`: `next.html` is /media/next.html and `../up/page.html` is /up/page.html
    assert!(
        chapter.contains(&format!("href=\"{}\"", s.url("/media/next.html"))),
        "{chapter}"
    );
    assert!(
        chapter.contains(&format!("href=\"{}\"", s.url("/up/page.html"))),
        "{chapter}"
    );
    assert!(
        s.hits().contains(&"/media/pic.png".to_string()),
        "the picture was fetched from the base: {:?}",
        s.hits()
    );
    assert!(chapter.contains("src=\"images/img0.png\""), "{chapter}");
    // every link leaves with noopener noreferrer
    assert!(
        chapter.matches("rel=\"noopener noreferrer\"").count() >= 2,
        "{chapter}"
    );
}

#[test]
fn a_blog_with_a_sidebar_and_comments_gives_the_post() {
    let s = Server::start(HashMap::from([("/blog".to_string(), html("blog.html"))]));
    let c = run(&s.url("/blog"), &local()).unwrap();
    assert_eq!(c.title, "Notatki o herbacie | Blog"); // a short title keeps its site name, as in Readability.js
    assert_eq!(c.language, "pl");
    let chapter = entry(&c.epub, "OEBPS/chapter.xhtml");
    assert!(chapter.contains("hundred and forty steps"));
    assert!(
        !chapter.contains("Kategorie") && !chapter.contains("Świetny wpis"),
        "{chapter}"
    );
}

#[test]
fn a_page_without_an_article_keeps_its_body_with_a_note_and_a_shell_that_needs_scripts_is_refused()
{
    let s = Server::start(HashMap::from([
        ("/links".to_string(), html("noarticle.html")),
        ("/words".to_string(), Route::Ok("text/html", words_page())),
        ("/app".to_string(), html("shell.html")),
    ]));
    // a page of links and a table: little to read, and the book says so
    let c = run(&s.url("/links"), &local()).unwrap();
    assert!(
        c.notes
            .iter()
            .any(|n| matches!(n, Note::WholePage | Note::Short)),
        "{:?}",
        c.notes
    );
    // Readability gives up on a page with too many elements: the cleaned body is used and the note says so
    let c = run(&s.url("/words"), &local()).unwrap();
    assert!(c.notes.contains(&Note::WholePage), "{:?}", c.notes);
    assert!(entry(&c.epub, "OEBPS/chapter.xhtml").contains("nothing like an article"));
    assert_eq!(
        run(&s.url("/app"), &local()).unwrap_err(),
        ClipError::NeedsScripts
    );
}

// ------------------------------------------------------------------------------------------------ what a page may carry

#[test]
fn a_hostile_page_leaves_nothing_that_runs_and_its_bad_pictures_are_dropped_with_their_alt_text() {
    let secret = Server::start(HashMap::from([(
        "/secret.png".to_string(),
        Route::Ok("image/png", PNG.to_vec()),
    )]));
    let page = String::from_utf8(fixture("hostile.html"))
        .unwrap()
        .replace("{{base}}", &secret.url(""));
    let s = Server::start(HashMap::from([
        (
            "/trap".to_string(),
            Route::Ok("text/html", page.into_bytes()),
        ),
        (
            "/img/pic1.png".to_string(),
            Route::Ok("image/png", PNG.to_vec()),
        ),
        (
            "/img/vector.svg".to_string(),
            Route::Ok(
                "image/svg+xml",
                b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>"
                    .to_vec(),
            ),
        ),
        (
            "/img/fake.png".to_string(),
            Route::Ok(
                "image/png",
                b"<html><script>alert(1)</script></html>".to_vec(),
            ),
        ),
        (
            "/img/real-but-text-plain.png".to_string(),
            Route::Ok("text/plain", PNG.to_vec()),
        ),
    ]));
    // this server is on this computer, so the "private picture" is only refused when the policy says so: here it is the
    // page itself that is allowed, and the picture on the other server is refused by the pretend-public rule below
    let mut policy = ClipPolicy::default();
    policy
        .pretend_public
        .insert("trap.test".into(), vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);
    let url = format!("http://trap.test:{}/trap", s.addr.port());
    let c = run(&url, &policy).unwrap();
    let chapter = entry(&c.epub, "OEBPS/chapter.xhtml");
    assert_well_formed(&chapter, "chapter");
    let lower = chapter.to_ascii_lowercase();
    for bad in [
        "<script",
        "<iframe",
        "<object",
        "<embed",
        "<form",
        "<input",
        "<button",
        "<svg",
        "onclick",
        "onmouseover",
        "onerror",
        "javascript:",
        "data:",
        "evil.invalid",
        "alert(",
    ] {
        assert!(!lower.contains(bad), "{bad} left in the chapter");
    }
    // the private picture was never requested
    assert!(
        secret.hits().is_empty(),
        "a picture on a private address was fetched: {:?}",
        secret.hits()
    );
    // what was dropped keeps its alt text; what is good is inside the book
    for alt in [
        "private picture",
        "a vector picture",
        "not really a picture",
        "a missing picture",
        "inline data",
    ] {
        assert!(chapter.contains(alt), "{alt} should stay as text");
    }
    assert!(!chapter.contains("<img src=\"http"), "{chapter}");
    // the type comes from the bytes: the png served as text/plain is kept, the html served as image/png is not
    let images: Vec<String> = entries(&c.epub)
        .into_iter()
        .map(|e| e.0)
        .filter(|n| n.starts_with("OEBPS/images/"))
        .collect();
    assert_eq!(
        images.len(),
        2,
        "good one, the png sent as text/plain, and nothing else: {images:?}"
    );
    assert!(
        c.notes
            .iter()
            .any(|n| matches!(n, Note::PicturesLeftOut(5))),
        "{:?}",
        c.notes
    );
}

#[test]
fn pictures_are_capped_by_number_by_size_and_in_total() {
    // 45 pictures: 40 are taken
    let mut body = String::from(
        "<html lang=\"en\"><head><title>Many pictures</title></head><body><article><h1>Many</h1>",
    );
    for p in fixture("article.html")
        .split(|b| *b == b'\n')
        .filter(|l| l.starts_with(b"<p>"))
        .take(8)
    {
        body.push_str(&String::from_utf8_lossy(p));
    }
    for i in 0..45 {
        body.push_str(&format!("<img src=\"/p/{i}.png\" alt=\"pic {i}\">"));
    }
    body.push_str("</article></body></html>");
    let mut routes = HashMap::from([(
        "/many".to_string(),
        Route::Ok("text/html", body.into_bytes()),
    )]);
    for i in 0..45 {
        routes.insert(format!("/p/{i}.png"), Route::Ok("image/png", PNG.to_vec()));
    }
    let s = Server::start(routes);
    let c = run(&s.url("/many"), &local()).unwrap();
    let images = entries(&c.epub)
        .into_iter()
        .filter(|e| e.0.starts_with("OEBPS/images/"))
        .count();
    assert_eq!(images, 40);
    assert!(c.notes.contains(&Note::PicturesLeftOut(5)), "{:?}", c.notes);
    assert_eq!(
        s.hits().iter().filter(|h| h.starts_with("/p/")).count(),
        40,
        "the other five were not even requested"
    );

    // one picture over 5 MB is dropped; five of 4.9 MB fit in 25 MB and the sixth does not
    let mut body = String::from(
        "<html lang=\"en\"><head><title>Big pictures</title></head><body><article><h1>Big</h1>",
    );
    for p in fixture("article.html")
        .split(|b| *b == b'\n')
        .filter(|l| l.starts_with(b"<p>"))
        .take(8)
    {
        body.push_str(&String::from_utf8_lossy(p));
    }
    body.push_str("<img src=\"/huge.png\" alt=\"huge\">");
    for i in 0..6 {
        body.push_str(&format!("<img src=\"/b/{i}.png\" alt=\"big {i}\">"));
    }
    body.push_str("</article></body></html>");
    let mut routes = HashMap::from([
        (
            "/big".to_string(),
            Route::Ok("text/html", body.into_bytes()),
        ),
        (
            "/huge.png".to_string(),
            Route::Big("image/png", 5 * 1024 * 1024 + 10),
        ),
    ]);
    for i in 0..6 {
        routes.insert(format!("/b/{i}.png"), Route::Big("image/png", 4_900_000));
    }
    let s = Server::start(routes);
    let c = run(&s.url("/big"), &local()).unwrap();
    let images = entries(&c.epub)
        .into_iter()
        .filter(|e| e.0.starts_with("OEBPS/images/"))
        .count();
    assert_eq!(images, 5, "{:?}", c.notes);
    assert!(c.notes.contains(&Note::PicturesLeftOut(2)), "{:?}", c.notes);
}

// ------------------------------------------------------------------------------------------------ refused addresses

fn refused_without_a_request(addr: &str, s: &Server) {
    let e = run(addr, &ClipPolicy::default()).unwrap_err();
    assert_eq!(e, ClipError::Refused, "{addr}");
    assert!(
        s.hits().is_empty(),
        "{addr}: a request was made: {:?}",
        s.hits()
    );
}

#[test]
fn this_computer_and_private_networks_are_refused_before_any_request() {
    let s = Server::start(pages(&[]));
    let port = s.addr.port();
    refused_without_a_request(&format!("http://127.0.0.1:{port}/article"), &s);
    refused_without_a_request(&format!("http://localhost:{port}/article"), &s); // a name that resolves to loopback
    refused_without_a_request(&format!("http://[::1]:{port}/article"), &s);
    refused_without_a_request(&format!("http://[::ffff:127.0.0.1]:{port}/article"), &s);
    refused_without_a_request(&format!("http://0.0.0.0:{port}/article"), &s);
    refused_without_a_request(&format!("http://2130706433:{port}/article"), &s); // 127.0.0.1 as one number
    refused_without_a_request(&format!("http://0x7f.0.0.1:{port}/article"), &s);
    for a in [
        "http://10.0.0.1/",
        "http://172.16.0.1/",
        "http://192.168.1.1/",
        "http://169.254.169.254/latest/meta-data/",
        "http://100.64.0.1/",
        "http://[fe80::1]/",
        "http://[fd00::1]/",
        "http://[::ffff:169.254.169.254]/",
    ] {
        assert_eq!(
            run(a, &ClipPolicy::default()).unwrap_err(),
            ClipError::Refused,
            "{a}"
        );
    }
}

#[test]
fn only_web_addresses_without_a_login_are_fetched() {
    let s = Server::start(pages(&[]));
    for a in [
        "file:///etc/passwd",
        "ftp://example.org/f",
        "javascript:alert(1)",
        "data:text/html,hi",
        "gopher://example.org/",
        "https://user:secret@example.org/",
        "https://user@example.org/",
        "not an address",
        "",
    ] {
        assert_eq!(
            run(a, &local()).unwrap_err(),
            ClipError::BadAddress,
            "{a:?}"
        );
    }
    assert!(s.hits().is_empty());
}

#[test]
fn a_redirect_to_a_private_address_is_refused_and_never_followed() {
    let private = Server::start(HashMap::from([(
        "/secret".to_string(),
        html("article.html"),
    )]));
    let public = Server::start(HashMap::from([
        (
            "/to-ip".to_string(),
            Route::Redirect(private.url("/secret")),
        ),
        (
            "/to-name".to_string(),
            Route::Redirect(format!("http://localhost:{}/secret", private.addr.port())),
        ),
        (
            "/to-metadata".to_string(),
            Route::Redirect("http://169.254.169.254/latest/meta-data/".into()),
        ),
        (
            "/to-file".to_string(),
            Route::Redirect("file:///etc/passwd".into()),
        ),
        ("/fine".to_string(), Route::Redirect("/article".into())),
        ("/article".to_string(), html("article.html")),
    ]));
    // "pub.test" stands for a public name; the server behind it is on this computer, and only that name is exempt
    let mut policy = ClipPolicy::default();
    policy
        .pretend_public
        .insert("pub.test".into(), vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);
    let at = |p: &str| format!("http://pub.test:{}{p}", public.addr.port());
    assert_eq!(run(&at("/to-ip"), &policy).unwrap_err(), ClipError::Refused);
    assert_eq!(
        run(&at("/to-name"), &policy).unwrap_err(),
        ClipError::Refused
    );
    assert_eq!(
        run(&at("/to-metadata"), &policy).unwrap_err(),
        ClipError::Refused
    );
    assert_eq!(
        run(&at("/to-file"), &policy).unwrap_err(),
        ClipError::BadAddress
    );
    assert!(
        private.hits().is_empty(),
        "the private server was reached: {:?}",
        private.hits()
    );
    // a redirect to the same public name is followed
    assert!(run(&at("/fine"), &policy).is_ok());
}

// ------------------------------------------------------------------------------------------------ what is not a page

#[test]
fn a_big_page_a_non_page_a_404_and_a_slow_site_are_refused_with_their_reasons() {
    let s = Server::start(HashMap::from([
        (
            "/huge".to_string(),
            Route::Big("text/html", 5 * 1024 * 1024 + 100),
        ),
        (
            "/pdf".to_string(),
            Route::Ok("application/pdf", b"%PDF-1.4".to_vec()),
        ),
        ("/gone".to_string(), Route::Status(404)),
        ("/boom".to_string(), Route::Status(500)),
        ("/slow".to_string(), Route::Slow(Duration::from_secs(3))),
        ("/loop".to_string(), Route::Redirect("/loop".into())),
    ]));
    assert_eq!(
        run(&s.url("/huge"), &local()).unwrap_err(),
        ClipError::TooBig
    );
    assert_eq!(
        run(&s.url("/pdf"), &local()).unwrap_err(),
        ClipError::NotHtml("application/pdf".into())
    );
    assert_eq!(
        run(&s.url("/gone"), &local()).unwrap_err(),
        ClipError::Status("127.0.0.1".into(), 404)
    );
    assert_eq!(
        run(&s.url("/boom"), &local()).unwrap_err(),
        ClipError::Status("127.0.0.1".into(), 500)
    );
    let fast = ClipPolicy {
        page_timeout: Some(Duration::from_millis(400)),
        ..local()
    };
    assert_eq!(
        run(&s.url("/slow"), &fast).unwrap_err(),
        ClipError::Unreachable("127.0.0.1".into())
    );
    assert_eq!(
        run(&s.url("/loop"), &local()).unwrap_err(),
        ClipError::TooManyRedirects
    );
    // the messages name the reason and not the address
    for e in [
        ClipError::TooBig,
        ClipError::Refused,
        ClipError::NeedsScripts,
        ClipError::Status("h.org".into(), 404),
    ] {
        assert!(!e.to_string().contains("http"), "{e}");
    }
}

#[test]
fn the_callback_can_cancel_and_sees_the_steps_in_order() {
    let s = Server::start(pages(&[]));
    let mut seen = Vec::new();
    clip(&s.url("/article"), &local(), at(10, 0), &mut |p| {
        seen.push(p);
        true
    })
    .unwrap();
    assert_eq!(seen.first(), Some(&Progress::Fetching));
    assert_eq!(seen.get(1), Some(&Progress::Cleaning));
    assert_eq!(seen.last(), Some(&Progress::Writing));
    assert!(
        seen.iter()
            .any(|p| matches!(p, Progress::Picture { total: 2, .. })),
        "{seen:?}"
    );
    // cancelling in the middle of the pictures
    let hits_before = s.hits().len();
    let r = clip(&s.url("/article"), &local(), at(10, 0), &mut |p| {
        !matches!(p, Progress::Picture { .. })
    });
    assert_eq!(r.unwrap_err(), ClipError::Cancelled);
    assert!(s.hits().len() > hits_before); // the page was fetched again, no picture after the cancel
    assert!(
        !s.hits()
            .iter()
            .rev()
            .take(1)
            .any(|h| h.starts_with("/img/")),
        "{:?}",
        s.hits()
    );
}
