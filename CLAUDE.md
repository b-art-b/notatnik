# Notatnik

Local-first block outliner (Logseq-inspired, own format). Markdown files are the source of truth;
SQLite is a rebuildable cache. Linux desktop first (Tauri 2 + Svelte 5), Android later.

## Docs (read before larger changes)

- `docs/requirements.md`: requirements with IDs (e.g. NS-3, CA-5); reference them in code comments, tests and commits
- `docs/architecture.md`: stack, crate boundaries, data flow, decisions log
- `docs/file-format.md`: on-disk format spec (draft); update it together with parser changes
- `docs/testing.md`: how things are verified (test levels, fixtures, check script)
- `docs/TODO.md`: phase overview and status
- `docs/phases/phase-N.md`: detailed tasks for each phase; every task has a **Verify** block, and a task is done only when all of its Verify items pass. Tick items off there

## Layout

- `crates/core` (`notatnik-core`): parser, model, edits, names, query language. **No I/O**; must build for `wasm32-unknown-unknown`.
- `crates/index`: SQLite schema, indexing, search, query → SQL.
- `crates/graph`: file I/O, atomic writes, watcher, conflicts.
- `crates/cli`: `notatnik` binary.
- `src-tauri/`, `ui/`: desktop app (not created yet; phase 2).

## Commands

```bash
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo check -p notatnik-core --target wasm32-unknown-unknown   # core must stay wasm-clean
cargo run -p notatnik-cli -- <args>
```

## Rules

- Don't start implementing a phase until the user asks; follow the task order and Verify steps in the phase doc.
- Never rewrite a whole user file when one block changed (ST-3); edits are minimal text replacements using source spans.
- Never store anything in the graph folder that can't be derived from or isn't meant to be synced (ST-2).
- Round-trip fidelity is tested with golden files: parse + no-op edit must be byte-identical.
- The user is new to Svelte: keep UI code idiomatic Svelte 5 (runes) and explain non-obvious idioms.
