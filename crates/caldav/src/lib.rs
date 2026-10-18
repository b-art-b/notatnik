//! Calendar sync with a CalDAV server, e.g. Nextcloud (CA-1..CA-9).
//!
//! [`ical`] reads and writes the iCalendar text of a task (VTODO) without any other crate; [`CalDavClient`] is
//! what the sync engine needs from a server, and [`fake::FakeServer`] is an in-memory one for tests. Errors are
//! those of `notatnik-sync`, so the app reports a calendar and a file sync the same way.
pub mod fake;
pub mod ical;
pub mod map;
pub mod pull;
pub mod push;

mod client;
mod http;
mod xml;

pub use client::{CalDavClient, Calendar, Cond, ItemRef, ServerItem, SyncResult};
pub use http::{HttpCalDav, calendar_home_url};
pub use notatnik_sync::{Error, Result};
