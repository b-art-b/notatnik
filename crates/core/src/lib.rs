//! Notatnik core: file format parser, block model, edit operations,
//! page-name/namespace resolution and the query language.
//!
//! This crate must stay free of I/O (filesystem, network, clock) so it can be
//! unit-tested in isolation and compiled to `wasm32-unknown-unknown` for the UI.
//! See `docs/architecture.md` §2.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
#![deny(missing_docs)]

pub mod edit;
pub mod encrypted;
pub mod frontmatter;
pub mod hash;
pub mod identity;
pub mod ids;
pub mod inline;
pub mod logbook;
pub mod mentions;
pub mod merge;
pub mod model;
pub mod names;
pub mod outline;
pub mod props;
pub mod query;
pub mod repeat;
pub mod span;
pub mod task;

pub use edit::{
    BlockOp, BlockRef, EditError, EditResult, TextPatch, apply, apply_patches, invert_patches,
    replace_block_id,
};
pub use identity::{BlockIdentity, Tracked};
pub use merge::{BlockConflict, Choice, ConflictKind, MergeError, MergeResult, merge, merge_with};
pub use model::*;
pub use names::{
    LinkError, PageName, fold_char, normalize_link, page_to_path, path_to_page, search_fold,
};
pub use outline::{parse_page, parse_page_with};
pub use query::{DateRange, Query, QueryError, WeekStart, parse_query, parse_query_partial};
pub use span::{Diagnostic, Severity, Span};
pub use task::{Moment, Priority, Repeat, RepeatRule, RepeatUnit, Task, TaskState};
