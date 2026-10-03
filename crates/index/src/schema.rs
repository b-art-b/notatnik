//! Database schema. Bump [`SCHEMA_VERSION`] on any change: old databases are deleted and rebuilt.

/// Current schema version (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 2;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE files(
    path TEXT PRIMARY KEY,
    page_id INTEGER,
    mtime_ns INTEGER NOT NULL,
    size INTEGER NOT NULL,
    hash TEXT NOT NULL
);
CREATE TABLE pages(
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    key TEXT NOT NULL UNIQUE,
    title TEXT,
    ns_parent_id INTEGER REFERENCES pages(id) ON DELETE SET NULL,
    is_journal INTEGER NOT NULL DEFAULT 0,
    journal_date TEXT,
    is_virtual INTEGER NOT NULL DEFAULT 1,
    file_path TEXT
);
CREATE INDEX pages_parent ON pages(ns_parent_id);
CREATE TABLE blocks(
    id INTEGER PRIMARY KEY,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    parent_id INTEGER,
    ord INTEGER NOT NULL,
    path TEXT NOT NULL,
    level INTEGER NOT NULL,
    uid TEXT,
    text TEXT NOT NULL,
    task_state TEXT,
    scheduled TEXT,
    scheduled_time TEXT,
    deadline TEXT,
    deadline_time TEXT,
    priority TEXT,
    priority_rank INTEGER,
    span_start INTEGER NOT NULL,
    span_end INTEGER NOT NULL,
    is_preamble INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX blocks_page ON blocks(page_id, id);
CREATE INDEX blocks_uid ON blocks(uid) WHERE uid IS NOT NULL;
CREATE INDEX blocks_task ON blocks(task_state) WHERE task_state IS NOT NULL;
CREATE INDEX blocks_scheduled ON blocks(scheduled) WHERE scheduled IS NOT NULL;
CREATE INDEX blocks_deadline ON blocks(deadline) WHERE deadline IS NOT NULL;
CREATE TABLE props(
    owner_kind TEXT NOT NULL,
    owner_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    ord INTEGER NOT NULL DEFAULT 0,
    value_text TEXT,
    value_fold TEXT,
    value_num REAL,
    value_date TEXT
);
CREATE INDEX props_owner ON props(owner_kind, owner_id, key);
CREATE INDEX props_key ON props(key, value_fold);
CREATE TABLE refs(
    src_page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    src_block_id INTEGER,
    kind TEXT NOT NULL,
    target_key TEXT,
    target_uid TEXT,
    raw TEXT
);
CREATE INDEX refs_target ON refs(target_key);
CREATE INDEX refs_block ON refs(src_block_id);
CREATE INDEX refs_page ON refs(src_page_id);
CREATE INDEX refs_uid ON refs(target_uid);
CREATE TABLE aliases(
    alias_key TEXT NOT NULL,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE
);
CREATE INDEX aliases_key ON aliases(alias_key);
CREATE INDEX aliases_page ON aliases(page_id);
CREATE TABLE issues(
    path TEXT NOT NULL,
    severity TEXT NOT NULL,
    code TEXT NOT NULL,
    message TEXT NOT NULL,
    offset INTEGER
);
CREATE INDEX issues_path ON issues(path);
CREATE VIRTUAL TABLE fts USING fts5(title, content, tokenize = 'unicode61 remove_diacritics 2');
"#;
