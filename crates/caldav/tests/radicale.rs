//! The sync against Radicale, a real CalDAV server (P4.1): `cargo test -p notatnik-caldav --features integration`.
//!
//! Which Radicale: one that is already running when `RADICALE_URL`, `RADICALE_USER` and `RADICALE_PASSWORD` are
//! set (it is left running); else `radicale` on the PATH, else `uvx radicale`. The last two are started on a free
//! port with a temporary storage folder and a test user, and stopped when the test ends. With none of them the
//! tests say so and pass.
#![cfg(feature = "integration")]

mod common;

use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use notatnik_caldav::HttpCalDav;

struct Radicale {
    child: Option<Child>,
    url: String,
    user: String,
    password: String,
    _dir: Option<tempfile::TempDir>,
}

impl Drop for Radicale {
    fn drop(&mut self) {
        if let Some(c) = self.child.as_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// How to start Radicale: `radicale` on the PATH, else the Python of an `uvx` environment that has it (not `uvx`
/// itself, which would start the server as a child of its own that we could not stop).
fn radicale_program() -> Option<(String, Vec<&'static str>)> {
    if Command::new("radicale")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Some(("radicale".into(), Vec::new()));
    }
    let out = Command::new("uvx")
        .args([
            "--from",
            "radicale",
            "python",
            "-c",
            "import sys; print(sys.executable)",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let python = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!python.is_empty()).then_some((python, vec!["-m", "radicale"]))
}

fn start() -> Option<Radicale> {
    if let (Ok(url), Ok(user), Ok(password)) = (
        std::env::var("RADICALE_URL"),
        std::env::var("RADICALE_USER"),
        std::env::var("RADICALE_PASSWORD"),
    ) {
        return Some(Radicale {
            child: None,
            url,
            user,
            password,
            _dir: None,
        });
    }
    let dir = tempfile::tempdir().unwrap();
    let users = dir.path().join("users");
    std::fs::write(&users, "tester:secret\n").unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let args = [
        "--server-hosts".to_string(),
        format!("127.0.0.1:{port}"),
        "--storage-filesystem-folder".into(),
        dir.path().join("collections").display().to_string(),
        "--auth-type".into(),
        "htpasswd".into(),
        "--auth-htpasswd-filename".into(),
        users.display().to_string(),
        "--auth-htpasswd-encryption".into(),
        "plain".into(),
    ];
    let (program, first) = radicale_program()?;
    // the server itself is our child, so stopping it stops all of it
    let child = Command::new(program)
        .args(first)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let up = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            up.elapsed() < Duration::from_secs(90),
            "Radicale did not start"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    Some(Radicale {
        child: Some(child),
        url: format!("http://127.0.0.1:{port}"),
        user: "tester".into(),
        password: "secret".into(),
        _dir: Some(dir),
    })
}

/// A calendar of its own, so runs against a server that stays up do not meet.
fn calendar(server: &Radicale) -> (HttpCalDav, String) {
    let client = HttpCalDav::new(&server.url, &server.user, &server.password).unwrap();
    let name = format!(
        "contract-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let href = client.make_calendar(&name, &["VTODO"]).unwrap();
    (client, href)
}

#[test]
fn radicale_contract() {
    let Some(server) = start() else {
        eprintln!("no Radicale found (radicale, uvx or RADICALE_URL): skipped");
        return;
    };
    let (client, href) = calendar(&server);
    common::contract(&client, &href);
}

#[test]
fn radicale_engine() {
    let Some(server) = start() else {
        eprintln!("no Radicale found (radicale, uvx or RADICALE_URL): skipped");
        return;
    };
    let (client, href) = calendar(&server);
    common::engine(&client, &href);
}
