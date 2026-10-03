//! SQLite index (a rebuildable cache, ST-2): schema, indexing, backlinks, search, query execution.
//!
//! The index never decides where it lives; the caller passes the database path. A schema-version
//! mismatch deletes the database and rebuilds it (no migrations: it is only a cache).

mod error;
mod fuzzy;
mod ingest;
mod qsql;
mod read;
mod schema;
pub mod walk;

pub use error::{Error, Result};
pub use fuzzy::FuzzyHit;
pub use ingest::{DuplicateId, FileStat, ReconcileStats, ReindexOutcome};
pub use qsql::{CompileError, CompiledQuery, QueryContext, QueryResults, QueryRow, compile};
pub use read::{Backlink, Counts, IssueRow, NamespaceRow, PageInfo, SearchHit, SearchKind};
pub use schema::SCHEMA_VERSION;

use std::collections::BTreeMap;
use std::path::Path;

use notatnik_core::{ParseOptions, PropType};
use rusqlite::Connection;

/// Options that influence how files are parsed and indexed.
#[derive(Debug, Clone, Default)]
pub struct IndexOptions {
    /// Declared property types (`[properties] types` in config).
    pub property_types: BTreeMap<String, PropType>,
}

impl IndexOptions {
    pub(crate) fn parse_options(&self) -> ParseOptions {
        ParseOptions {
            property_types: self.property_types.clone(),
        }
    }
}

/// What happened when the database was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenInfo {
    /// The schema version didn't match, so the old database was deleted and recreated empty.
    pub rebuilt: bool,
}

/// The index.
pub struct Index {
    pub(crate) conn: Connection,
    pub(crate) options: IndexOptions,
    pub(crate) fuzzy: fuzzy::FuzzyCache,
}

impl Index {
    /// Opens (or creates) the index at `path`. On a schema-version mismatch the database is
    /// deleted and recreated.
    pub fn open(path: &Path, options: IndexOptions) -> Result<(Index, OpenInfo)> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut rebuilt = false;
        {
            let conn = Connection::open(path)?;
            let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            let tables: i64 =
                conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))?;
            drop(conn);
            if tables > 0 && v != SCHEMA_VERSION {
                for suffix in ["", "-wal", "-shm"] {
                    let mut p = path.as_os_str().to_owned();
                    p.push(suffix);
                    let _ = std::fs::remove_file(p);
                }
                rebuilt = true;
            }
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let idx = Self::init(conn, options)?;
        Ok((idx, OpenInfo { rebuilt }))
    }

    /// An in-memory index (tests, one-off commands).
    pub fn open_in_memory(options: IndexOptions) -> Result<Index> {
        Self::init(Connection::open_in_memory()?, options)
    }

    fn init(conn: Connection, options: IndexOptions) -> Result<Index> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let tables: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))?;
        if tables == 0 {
            conn.execute_batch(schema::SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Index {
            conn,
            options,
            fuzzy: fuzzy::FuzzyCache::default(),
        })
    }

    /// The options used for parsing.
    pub fn options(&self) -> &IndexOptions {
        &self.options
    }

    /// Direct access to the connection (tests, benchmarks, `EXPLAIN QUERY PLAN`).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}
