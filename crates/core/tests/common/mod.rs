#![allow(dead_code)]
use std::path::{Path, PathBuf};

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// All `.md` files under `fixtures/graphs`, sorted, as (relative path, bytes→string).
pub fn fixture_files() -> Vec<(String, String)> {
    let root = fixtures_dir().join("graphs");
    let mut out = Vec::new();
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.extension().is_some_and(|e| e == "md")
                && p.file_name().unwrap() != "README.md"
            {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, String::from_utf8(std::fs::read(&p).unwrap()).unwrap()));
            }
        }
    }
    walk(&root, &root, &mut out);
    out
}
