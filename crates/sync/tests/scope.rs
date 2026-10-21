//! Encrypting everything except a list (EN-27): a new file anywhere is stored encrypted, the exceptions and the settings
//! stay plain, switching the scope moves the files both ways, and a locked device syncs only what is plain.
//! Against the in-memory WebDAV server.
use std::path::Path;
use std::sync::Arc;

use notatnik_core::encrypted::EncryptedPaths;
use notatnik_crypt::{KdfParams, Keys, Vault};
use notatnik_sync::fakedav::FakeDav;
use notatnik_sync::{Bases, Encryption, Options, Report, WebDavRemote, sync};

const SECRET: &str = "SECRETMARKER-4711";

struct Device {
    replica: tempfile::TempDir,
    state: tempfile::TempDir,
}

impl Device {
    fn new() -> Device {
        Device {
            replica: tempfile::tempdir().unwrap(),
            state: tempfile::tempdir().unwrap(),
        }
    }
    fn root(&self) -> &Path {
        self.replica.path()
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.root().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root().join(rel)).unwrap()
    }
    fn has(&self, rel: &str) -> bool {
        self.root().join(rel).exists()
    }
}

struct Env {
    dav: FakeDav,
    keys: Arc<Keys>,
}

impl Env {
    fn new() -> Env {
        let (_, keys, _) = Vault::create("pw", KdfParams::insecure_for_tests()).unwrap();
        Env {
            dav: FakeDav::start(),
            keys: Arc::new(keys),
        }
    }
    fn run(
        &self,
        d: &Device,
        rules: &EncryptedPaths,
        hidden: bool,
        keys: Option<Arc<Keys>>,
    ) -> Report {
        let remote = WebDavRemote::new(
            &self.dav.folder_url("Notes/g"),
            &self.dav.user,
            &self.dav.password,
        )
        .unwrap();
        let mut o = Options::new(d.state.path().join("state.json"));
        o.bases = Bases::new(Some(d.state.path().join("bases")));
        o.encryption = Some(Encryption {
            paths: rules.clone(),
            keys,
            hidden,
        });
        let r = sync(d.root(), &remote, &o).unwrap();
        assert!(r.errors.is_empty(), "{r:?}");
        r
    }
    fn names(&self) -> Vec<String> {
        self.dav
            .files("Notes/g")
            .into_iter()
            .map(|p| p.trim_start_matches("Notes/g/").to_string())
            .collect()
    }
    fn objects(&self, hidden: bool) -> usize {
        self.names()
            .iter()
            .filter(|n| {
                if hidden {
                    n.starts_with(".notatnik-enc/")
                } else {
                    n.ends_with(".enc")
                }
            })
            .count()
    }
}

fn everything_but_public() -> EncryptedPaths {
    EncryptedPaths::everything_except(&["pages/public/".to_string()])
}

fn seed(a: &Device) {
    a.write("pages/a.md", &format!("- {SECRET} a\n"));
    a.write("journals/2026-10-03.md", &format!("- {SECRET} j\n"));
    a.write("top.md", &format!("- {SECRET} top\n"));
    a.write("assets/pic.bin", SECRET);
    a.write("pages/public/pub.md", "- open\n");
    a.write(".notatnik/config.toml", "[encryption]\nscope = \"all\"\n");
}

#[test]
fn everything_is_stored_encrypted_except_the_exceptions_and_the_settings() {
    for hidden in [true, false] {
        let e = Env::new();
        let a = Device::new();
        seed(&a);
        let rules = everything_but_public();
        let r = e.run(&a, &rules, hidden, Some(e.keys.clone()));
        assert!(r.complete && r.pushed.len() == 6, "{r:?}");
        assert_eq!(e.objects(hidden), 4, "{:?}", e.names());
        let names = e.names();
        assert!(
            names.contains(&"pages/public/pub.md".to_string()),
            "{names:?}"
        );
        assert!(
            names.contains(&".notatnik/config.toml".to_string()),
            "{names:?}"
        );
        for n in &names {
            assert!(
                !hidden || (!n.contains("top") && !n.contains("2026")),
                "{n}"
            );
            if let Some(bytes) = e.dav.file(&format!("Notes/g/{n}")) {
                assert!(
                    !bytes.windows(SECRET.len()).any(|w| w == SECRET.as_bytes()),
                    "{n} holds the text"
                );
            }
        }
        // a file made later, in a folder nobody listed, is encrypted too
        a.write("brand new/idea.md", &format!("- {SECRET} new\n"));
        let r = e.run(&a, &rules, hidden, Some(e.keys.clone()));
        assert_eq!(r.pushed.len(), 1, "{r:?}");
        assert_eq!(e.objects(hidden), 5);
        assert!(!e.names().iter().any(|n| n.contains("brand")) || !hidden);

        // another device with the key gets everything back under the real names
        let b = Device::new();
        let r = e.run(&b, &rules, hidden, Some(e.keys.clone()));
        assert_eq!(r.pulled.len(), 7, "{r:?}");
        assert_eq!(b.read("brand new/idea.md"), format!("- {SECRET} new\n"));
        assert_eq!(b.read("pages/public/pub.md"), "- open\n");
    }
}

#[test]
fn switching_the_scope_moves_the_files_both_ways_and_loses_none() {
    let e = Env::new();
    let a = Device::new();
    seed(&a);
    let none = EncryptedPaths::default();
    let all = everything_but_public();
    let k = || Some(e.keys.clone());
    // plain first
    let r = e.run(&a, &none, true, k());
    assert!(r.complete, "{r:?}");
    assert_eq!(e.objects(true), 0);
    // everything: the five files that are not exceptions or settings are stored again as objects
    let r = e.run(&a, &all, true, k());
    assert_eq!(r.recoded.len(), 4, "{r:?}");
    assert_eq!(e.objects(true), 4);
    assert!(!e.names().contains(&"top.md".to_string()));
    // and back: plain copies again, the objects are gone, the local files never changed
    let r = e.run(&a, &none, true, k());
    assert_eq!(r.recoded.len(), 4, "{r:?}");
    assert_eq!(e.objects(true), 0);
    assert!(e.names().contains(&"top.md".to_string()));
    assert_eq!(a.read("top.md"), format!("- {SECRET} top\n"));
    // adding an exception moves just that file back
    e.run(&a, &all, true, k());
    let more =
        EncryptedPaths::everything_except(&["pages/public/".to_string(), "top.md".to_string()]);
    let r = e.run(&a, &more, true, k());
    assert_eq!(r.recoded, ["top.md"], "{r:?}");
    assert_eq!(e.objects(true), 3);
    assert!(e.names().contains(&"top.md".to_string()));
}

#[test]
fn a_locked_device_syncs_only_what_is_plain_and_deletes_nothing() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    let rules = everything_but_public();
    e.run(&a, &rules, true, Some(e.keys.clone()));
    // B has no key: it gets the exception and the settings, leaves the four objects alone
    let r = e.run(&b, &rules, true, None);
    assert_eq!(r.left_alone, 4, "{r:?}");
    assert!(b.has("pages/public/pub.md") && b.has(".notatnik/config.toml"));
    assert!(!b.has("top.md") && !b.has("pages/a.md"));
    // a quiet sync of B deletes nothing on the server and the next sync of A still sees all
    let r = e.run(&b, &rules, true, None);
    assert!(
        r.deleted_remote.is_empty() && r.deleted_local.is_empty(),
        "{r:?}"
    );
    assert_eq!(e.objects(true), 4);
    // B unlocks and gets the rest
    let r = e.run(&b, &rules, true, Some(e.keys.clone()));
    assert_eq!(r.pulled.len(), 4, "{r:?}");
    assert_eq!(b.read("top.md"), format!("- {SECRET} top\n"));
}
