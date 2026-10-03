//! Notatnik core: file format parser, block model, edit operations,
//! page-name/namespace resolution and the query language.
//!
//! This crate must stay free of I/O (filesystem, network, clock) so it can be
//! unit-tested in isolation and compiled to `wasm32-unknown-unknown` for the UI.
//! See `docs/architecture.md` §2.
