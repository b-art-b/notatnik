//! WASM bindings: the UI uses the same parser as the backend (decision D7).
//! All functions take and return JSON text, so the boundary stays simple and the output can be
//! compared byte-for-byte with the Rust-side snapshots.

use notatnik_core as core;
use wasm_bindgen::prelude::*;

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Parses a page; returns the `Page` as JSON.
#[wasm_bindgen]
pub fn parse_page(text: &str) -> String {
    json(&core::parse_page(text))
}

/// Parses query text; returns `{"query":…,"errors":[…]}` (best-effort query plus all errors).
#[wasm_bindgen]
pub fn parse_query(text: &str) -> String {
    let (query, errors) = core::parse_query_partial(text);
    json(&serde_json::json!({ "query": query, "errors": errors }))
}

/// Normalises a link target; returns `{"ok":"name"}` or `{"error":"message"}`.
#[wasm_bindgen]
pub fn resolve_link(raw: &str) -> String {
    match core::normalize_link(raw) {
        Ok(n) => json(&serde_json::json!({ "ok": n.as_str() })),
        Err(e) => json(&serde_json::json!({ "error": e.to_string() })),
    }
}

/// Applies an op (JSON) to `source`; returns `{"new_source":…,"patches":[…]}` or `{"error":"…"}`.
#[wasm_bindgen]
pub fn apply_op(source: &str, op_json: &str) -> String {
    let op: core::BlockOp = match serde_json::from_str(op_json) {
        Ok(o) => o,
        Err(e) => return json(&serde_json::json!({ "error": format!("invalid op: {e}") })),
    };
    let page = core::parse_page(source);
    match core::apply(source, &page, &op) {
        Ok(r) => json(&r),
        Err(e) => json(&serde_json::json!({ "error": e.to_string() })),
    }
}

/// The text a block shows in the editor (`path` is a JSON array of child indices).
#[wasm_bindgen]
pub fn block_edit_text(source: &str, path_json: &str) -> String {
    let page = core::parse_page(source);
    let Ok(path) = serde_json::from_str::<Vec<usize>>(path_json) else {
        return String::new();
    };
    let mut blocks = &page.blocks;
    let mut found = None;
    for i in &path {
        found = blocks.get(*i);
        match found {
            Some(b) => blocks = &b.children,
            None => return String::new(),
        }
    }
    found.map(|b| b.edit_text(source)).unwrap_or_default()
}
