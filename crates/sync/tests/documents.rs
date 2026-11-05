//! The document client (NA-2, NA-3, NA-5): stat, search by file id, bounded downloads, no redirects.

use notatnik_sync::fakedav::FakeDav;
use notatnik_sync::{Error, WebDavRemote};

fn client(dav: &FakeDav) -> WebDavRemote {
    WebDavRemote::for_documents(
        &format!("{}/remote.php/dav/files/{}", dav.server, dav.user),
        &dav.user,
        &dav.password,
    )
    .unwrap()
}

#[test]
fn stat_gives_etag_size_and_id_and_a_missing_file_is_none() {
    let dav = FakeDav::start();
    dav.put_file("Docs/Report.pdf", b"%PDF-1.4 hello");
    let c = client(&dav);
    let f = c.stat("Docs/Report.pdf").unwrap().unwrap();
    assert_eq!(f.name, "Report.pdf");
    assert_eq!(f.size, 14);
    assert!(!f.etag.is_empty());
    assert_eq!(
        f.file_id,
        dav.file_id("Docs/Report.pdf").unwrap().to_string()
    );
    assert!(c.stat("Docs/Nope.pdf").unwrap().is_none());
    // a folder is not a document
    assert!(c.stat("Docs").unwrap().is_none());
}

#[test]
fn a_file_is_found_by_its_number_also_after_it_was_renamed_and_moved() {
    let dav = FakeDav::start();
    dav.put_file("Docs/Report.pdf", b"x");
    let id = dav.file_id("Docs/Report.pdf").unwrap().to_string();
    let c = client(&dav);
    assert_eq!(c.find_by_id(&id).unwrap().unwrap().path, "Docs/Report.pdf");
    dav.put_file("Archive/placeholder", b"");
    dav.move_path("Docs/Report.pdf", "Archive/Final report.pdf");
    let f = c.find_by_id(&id).unwrap().unwrap();
    assert_eq!(f.path, "Archive/Final report.pdf");
    assert_eq!(f.name, "Final report.pdf");
    assert!(c.find_by_id("999999").unwrap().is_none());
    // not a number: never sent to the server
    dav.clear_requests();
    assert!(c.find_by_id("1 or 1=1").unwrap().is_none());
    assert!(dav.requests().is_empty());
}

#[test]
fn a_server_that_does_not_search_gives_none() {
    let dav = FakeDav::start();
    dav.put_file("a.pdf", b"x");
    let id = dav.file_id("a.pdf").unwrap().to_string();
    dav.set_search(false);
    assert!(client(&dav).find_by_id(&id).unwrap().is_none());
}

#[test]
fn a_download_is_written_whole_and_a_wrong_login_is_refused() {
    let dav = FakeDav::start();
    let body: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    dav.put_file("big.pdf", &body);
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("got.pdf");
    let got = client(&dav).download("big.pdf", 1 << 20, &dest).unwrap();
    assert_eq!(got.size, 5000);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert!(!tmp.path().join("got.pdf.part").exists());

    let bad = WebDavRemote::for_documents(
        &format!("{}/remote.php/dav/files/{}", dav.server, dav.user),
        &dav.user,
        "wrong",
    )
    .unwrap();
    assert!(matches!(
        bad.download("big.pdf", 1 << 20, &tmp.path().join("x.pdf")),
        Err(Error::Auth(_))
    ));
    assert!(matches!(
        client(&dav).download("none.pdf", 1 << 20, &tmp.path().join("y.pdf")),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn a_download_over_the_limit_is_refused_and_leaves_nothing() {
    let dav = FakeDav::start();
    dav.put_file("big.pdf", &vec![7u8; 4000]);
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("got.pdf");
    // announced by Content-Length
    let e = client(&dav).download("big.pdf", 1000, &dest).unwrap_err();
    assert!(matches!(e, Error::TooLarge(4000)), "{e}");
    assert!(std::fs::read_dir(tmp.path()).unwrap().next().is_none());
}

/// A server that does not tell the size (chunked body) and sends more than the limit.
#[test]
fn a_body_that_grows_past_the_limit_stops_the_download_and_leaves_no_file() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            );
            for _ in 0..20 {
                if stream.write_all(b"200\r\n").is_err() {
                    break;
                }
                let _ = stream.write_all(&[b'a'; 512]);
                let _ = stream.write_all(b"\r\n");
            }
            let _ = stream.write_all(b"0\r\n\r\n");
        }
    });
    let c = WebDavRemote::for_documents(
        &format!("http://127.0.0.1:{port}/remote.php/dav/files/me"),
        "me",
        "pw",
    )
    .unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let e = c
        .download("x.pdf", 2000, &tmp.path().join("x.pdf"))
        .unwrap_err();
    assert!(matches!(e, Error::TooLarge(_)), "{e}");
    assert!(std::fs::read_dir(tmp.path()).unwrap().next().is_none());
}

/// A redirect to another host: not followed, and the login is not sent there.
#[test]
fn a_redirect_is_not_followed_and_the_login_goes_nowhere_else() {
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};
    let other_hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let other = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let other_port = other.local_addr().unwrap().port();
    {
        let hits = other_hits.clone();
        std::thread::spawn(move || {
            for stream in other.incoming().flatten() {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                hits.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });
    }
    let first = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = first.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in first.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{other_port}/stolen\r\nContent-Length: 0\r\n\r\n"
            );
        }
    });
    let c = WebDavRemote::for_documents(
        &format!("http://127.0.0.1:{port}/remote.php/dav/files/me"),
        "me",
        "pw",
    )
    .unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let e = c
        .download("a.pdf", 1000, &tmp.path().join("a.pdf"))
        .unwrap_err();
    assert!(e.to_string().contains("redirect"), "{e}");
    assert!(c.stat("a.pdf").is_err());
    assert!(other_hits.lock().unwrap().is_empty());
}

#[test]
fn a_stopped_server_is_offline() {
    let dav = FakeDav::start();
    dav.put_file("a.pdf", b"x");
    dav.set_down(true);
    // 503 is a server error, not "offline" ...
    assert!(matches!(client(&dav).stat("a.pdf"), Err(Error::Server(_))));
    let url = format!("{}/remote.php/dav/files/me", dav.server);
    drop(dav);
    let c = WebDavRemote::for_documents(&url, "me", "pw").unwrap();
    assert!(matches!(c.stat("a.pdf"), Err(Error::Offline(_))));
}
