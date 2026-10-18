//! Sync of a graph folder (the replica) with a WebDAV folder, e.g. on Nextcloud (SY-1..SY-8).
//!
//! The app keeps working on ordinary files in the replica; this crate only moves files between that folder and
//! the remote. [`Remote`] is what the engine needs from a server, [`WebDavRemote`] speaks WebDAV, [`plan`] decides
//! what to do for each path and [`sync`] does it.
mod bases;
mod engine;
mod error;
mod local;
mod plan;
mod remote;
mod state;
mod webdav;

#[cfg(feature = "fakedav")]
pub mod fakedav;

pub use bases::Bases;
pub use engine::{Options, Report, sync};
pub use error::{Error, Result};
pub use plan::{Action, plan};
pub use remote::{Cond, Entry, Listing, Remote, norm_etag};
pub use state::{FileState, SyncState};
pub use webdav::{
    Probe, WebDavRemote, basic_auth, check_url, folder_url, item_url, probe, web_base,
};
