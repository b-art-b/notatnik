#![allow(dead_code)]
use std::path::{Path, PathBuf};

use notatnik_index::{Index, IndexOptions};

pub fn graphs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/graphs")
}

pub fn fixture(name: &str) -> PathBuf {
    graphs_dir().join(name)
}

pub fn indexed(name: &str) -> Index {
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(&fixture(name)).unwrap();
    idx
}

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// Copies a fixture graph into a temp dir so tests can modify it.
pub fn temp_graph(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture(name), dir.path());
    dir
}

/// All rows of all tables (except `files`, whose mtimes vary), as text, for equality checks.
pub fn dump(idx: &Index) -> String {
    let mut out = String::new();
    for (table, order) in [
        ("pages", "id"),
        ("blocks", "id"),
        ("props", "owner_kind, owner_id, key, ord"),
        (
            "refs",
            "src_page_id, src_block_id, kind, target_key, target_uid, raw",
        ),
        ("aliases", "alias_key, page_id"),
        ("issues", "path, code, offset"),
        ("fts", "rowid"),
    ] {
        let sql = format!("SELECT * FROM {table} ORDER BY {order}");
        let mut st = idx.connection().prepare(&sql).unwrap();
        let n = st.column_count();
        let rows = st
            .query_map([], |r| {
                let mut s = String::new();
                for i in 0..n {
                    let v: rusqlite::types::Value = r.get(i)?;
                    s.push_str(&format!("{v:?}|"));
                }
                Ok(s)
            })
            .unwrap();
        out.push_str(&format!("== {table}\n"));
        for r in rows {
            out.push_str(&r.unwrap());
            out.push('\n');
        }
    }
    out
}

/// Checksum of everything stored for one page (blocks, props, refs).
pub fn page_checksum(idx: &Index, page: &str) -> String {
    let c = idx.connection();
    let id: i64 = c
        .query_row("SELECT id FROM pages WHERE name = ?1", [page], |r| r.get(0))
        .unwrap();
    let mut s = String::new();
    let mut st = c.prepare("SELECT path, text, uid, task_state, scheduled, deadline, span_start, span_end FROM blocks WHERE page_id = ?1 ORDER BY id").unwrap();
    for r in st
        .query_map([id], |r| {
            Ok(format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?
            ))
        })
        .unwrap()
    {
        s.push_str(&r.unwrap());
        s.push('\n');
    }
    let mut st = c
        .prepare(
            "SELECT kind, target_key, target_uid FROM refs WHERE src_page_id = ?1 ORDER BY rowid",
        )
        .unwrap();
    for r in st
        .query_map([id], |r| {
            Ok(format!(
                "{:?}|{:?}|{:?}",
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?
            ))
        })
        .unwrap()
    {
        s.push_str(&r.unwrap());
        s.push('\n');
    }
    s
}
