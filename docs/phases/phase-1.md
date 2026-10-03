# Phase 1: Core without UI (format, parser, edits, index, graph service, CLI)

**Goal:** all the logic that touches notes works and is proven correct *without any UI*. At the end,
you can keep a real graph in a text editor and use the `notatnik` CLI to index, search, query, check
and rename, with the guarantee that the app never damages files.

**Requirements covered:** ST-1–5, ST-9 (partial), ST-10, ED-1 (model), LN-1–3, LN-5, LN-7, LN-8,
NS-1, NS-2, NS-9, TK-1, TK-2 (model), PR-1, PR-2, PR-4, QV-2, QV-3, QV-10,
SR-1, SR-2, NF-1–3, NF-5, NF-7, NF-9.
**Depends on:** phase 0 (including spike S2's parser decision).

## Exit criteria

1. Every task below is checked, with every **Verify** item passing.
2. `scripts/check.sh` passes; `scripts/check.sh --full` passes, including benchmarks.
3. Benchmarks meet NF-2 (cold index < 30 s), NF-3 (reindex one file < 50 ms) and NF-5 (search < 100 ms) on the synthetic graph.
4. `docs/file-format.md` is marked **v0 frozen**.
5. The demo script at the end of this document runs as written.

## Milestones

| | Milestone | Tasks |
|---|---|---|
| 1A | Format frozen, fixtures written | P1.1–P1.3 |
| 1B | Parser | P1.4–P1.11 |
| 1C | Page names and namespaces | P1.12–P1.13 |
| 1D | Edit operations | P1.14–P1.15 |
| 1E | Query language (parsing) | P1.16–P1.17 |
| 1F | WASM bindings | P1.18 |
| 1G | SQLite index | P1.19–P1.26 |
| 1H | Query execution | P1.27–P1.29 |
| 1I | Graph service (files, watcher) | P1.30–P1.36 |
| 1J | CLI | P1.37 |
| 1K | Synthetic graph and benchmarks | P1.38–P1.39 |

Suggested order: 1A → 1B → 1C → 1D → 1E → 1G → 1H → 1I → 1J → 1K, with 1F any time after 1E.

---

## 1A: Format frozen, fixtures written

### P1.1 Resolve the open points in the format spec
- [x] Decide and document in `file-format.md` (all decided 2026-10-03):
  - ✅ Block IDs: 6 chars `[a-z0-9]`, collision and duplicate handling in file-format.md §3.4
  - ✅ No `created` timestamps (file-format.md §3.5)
  - ✅ Line endings: keep the file's existing style; new files use LF
  - ✅ Indentation: 2 spaces for new files; existing files keep their detected style
  - ✅ One journal file per day
  - ✅ Decided 2026-10-03: `#123` **is** a tag, as are `#tag_name`, `#tagName`, `#tag-name` (tag rules in file-format.md §4)
  - ✅ Decided 2026-10-03: raw HTML is **escaped**, never rendered
  - ✅ Decided 2026-10-03: **no relative links**; all links are global
  - ✅ Decided 2026-10-03: repeating tasks log completions in a hidden `completed::` property (file-format.md §5)
  - ✅ Decided 2026-10-03: `ns: work` = `work` + all pages below it
  - ✅ Week start: Monday, configurable (`[graph] week_start`)
- [ ] Change status to "v0 frozen"; later changes need a format version bump and a migration note

**Verify**
- [x] The §9 "Open points" section is empty (2026-10-03)
- [ ] You have read and accepted the spec

### P1.2 Specify edge cases
Add an "Edge cases" section to `file-format.md` with the expected behaviour for each:
- [ ] CRLF files, a UTF-8 BOM, a missing trailing newline, an empty file, a file that is only frontmatter
- [ ] Tabs, 4-space indentation, mixed indentation, inconsistent indent jumps (child indented by 6 under a parent at 0)
- [ ] `*` and `+` bullets, numbered lists, an empty bullet `-`, a bullet with only properties
- [ ] Code fences (``` and ~~~) containing `- `, `key:: v`, `[[x]]`, `TODO` (none of these may be parsed as syntax)
- [ ] Invalid YAML frontmatter (the page still loads, the frontmatter is treated as text, and a diagnostic is produced); `---` used as a horizontal rule later in the file
- [ ] Text between bullets, headings (`# Title` outside bullets, `- ## Heading` inside one)
- [ ] Very long lines (100 KB), deeply nested blocks (depth 50)
- [ ] `[[` without a closing bracket, nested brackets `[[a [[b]]]]`, escaped `\[[not a link]]`
- [ ] `#` inside URLs (`https://x.y/#frag`), in inline code, in headings

**Verify**
- [ ] Every edge case has a fixture file in `fixtures/graphs/edge-cases/` (P1.3) and an expected result stated in the spec

### P1.3 Fixture graphs
- [ ] `fixtures/graphs/basic`, `edge-cases`, `namespaces`, `refs`, `tasks`, `queries` (layout as in [testing.md §2](../testing.md#2-fixtures))
- [ ] Each has a `README.md` listing what it covers
- [ ] A coverage table in `fixtures/README.md`: every syntax element in file-format.md §3–§6 → the fixture file(s) using it

**Verify**
- [ ] The coverage table has no empty rows
- [ ] Every fixture file opens and reads sensibly in a plain text editor (format goal 1)

## 1B: Parser (`notatnik-core`)

The parser **never fails**: for any input it returns a `Page` plus a list of `Diagnostic { span, severity, message }`.

### P1.4 Model types
- [ ] `Page { frontmatter, preamble, blocks, diagnostics }`, `Block { content_span, first_line, properties, children, block_id, task, refs, span }`,
      `Property { key, raw, value: PropertyValue }`, `PropertyValue { Text, Number, Bool, Date, DateTime, Links(Vec<PageRef>), List }`,
      `Ref { kind: Page | Tag | Block | BlockEmbed | PageEmbed, target, span }`, `Task { state, scheduled, deadline, priority, repeat, done }`
- [ ] All types `serde::Serialize` (needed for snapshots, IPC and WASM)
- [ ] `Span` = byte range into the original source; helper `span.slice(&source)`

**Verify**
- [ ] `cargo doc -p notatnik-core --no-deps` builds with no warnings, and every public type has a doc comment
- [ ] `cargo check -p notatnik-core --target wasm32-unknown-unknown` still passes (no I/O or OS-dependent crates added)

### P1.5 Frontmatter
- [ ] Detect `---` only at byte 0 (after an optional BOM); parse YAML with `serde_yaml`-compatible crate (check it builds for WASM)
- [ ] Reserved keys typed (`title`, `aliases`, `tags`, `template`, `icon`); others become properties

**Verify** (`cargo test -p notatnik-core frontmatter`)
- [ ] Valid frontmatter → properties with correct types
- [ ] Empty frontmatter `---\n---` → no properties, no diagnostic
- [ ] Invalid YAML → page parses, diagnostic with a span pointing into the frontmatter, and the frontmatter text is preserved
- [ ] `---` on line 5 → not frontmatter (horizontal rule in the preamble)

### P1.6 Block tree with source spans
- [ ] Line-based outline parsing: bullets, indentation levels, continuation lines, code fences (fence state machine so bullets inside fences are content)
- [ ] Preamble block for text before the first bullet
- [ ] Indentation normalised to *levels* in the model; raw indentation kept for editing

**Verify** (`cargo test -p notatnik-core block_tree`)
- [ ] insta snapshots of the parsed tree for **every** fixture file; you review and accept each snapshot once
- [ ] Invariant test over all fixtures: each block's `span` slice starts with its bullet; child spans lie inside the parent span; sibling spans don't overlap; spans are in document order
- [ ] Coverage test: frontmatter + preamble + all top-level block spans + whitespace between them = the entire file
- [ ] Property test: random block tree → serialize canonically → parse → identical tree (1,000 cases)

### P1.7 Block properties
- [ ] `key:: value` lines directly after a block's first line; stops at the first non-property line
- [ ] Type inference: date `2026-10-05`, datetime `2026-10-05 14:00`, number, `true`/`false`, link list `[[a]], [[b]]`, otherwise text
- [ ] Property types declared in config override inference (config is passed in as a parameter; core does no I/O)

**Verify** (`cargo test -p notatnik-core properties`)
- [ ] Table test with ≥ 15 rows covering every value type and near-misses (`2026-13-01` is text, `1.5` is a number, `True` is text unless decided otherwise in P1.1)
- [ ] A `key:: value` after a normal content line → part of the content, not a property
- [ ] `key:: value` inside a code fence → not a property

### P1.8 Inline syntax
- [ ] Page links, labelled links, tags (`#x`, `#a/b`, `#123`, `#tag_name`, `#tagName`, `#tag-name`, `#zażółć`, `#[[multi word]]`), block refs, block embeds, page embeds, highlights
- [ ] Skip inline code, code fences and escaped brackets; tags only after whitespace or line start

**Verify** (`cargo test -p notatnik-core inline`)
- [ ] Table test, at least one row per syntax element in file-format.md §4
- [ ] Negative rows: `` `[[x]]` `` (inline code), `\[[x]]`, `https://a.b/#frag`, `a#b` (no preceding whitespace), `# Heading`, `[[unclosed`
- [ ] Boundary rows: `#tag.` → `tag`; `(#tag)` → `tag`; `#tag-` → `tag`; `#123` → `123`
- [ ] Raw HTML (`<b>x</b>`, `<script>`) is parsed as plain text, not as an HTML node
- [ ] Every ref's span slices exactly the syntax in the source (e.g. `[[a|label]]`)

### P1.9 Block IDs
- [ ] Parse ` ^id` only at the end of the first line
- [ ] `generate_block_id(existing: &HashSet<BlockId>, rng)` (RNG injected, so it's deterministic in tests)

**Verify**
- [ ] `^abc123` in the middle of a line → not an ID
- [ ] With a seeded RNG and a pre-filled set that collides on the first draw → a different ID is returned
- [ ] The ID is excluded from the content used for search and display (it is a hidden part of the first line)
- [ ] Duplicate rule (file-format.md §3.4), integration test in `notatnik-graph`: copy a block with `^abc123` into another file → after reindex, the original keeps `abc123`, the copy gets a new ID written to its file, `((abc123))` still resolves to the original, and the health check reports the re-ID
- [ ] Cold index with the same duplicate → the block in the file that sorts first by path keeps the ID

### P1.10 Tasks
- [ ] Markers at the start of the first line (case-sensitive, followed by a space): `TODO DOING WAITING DONE CANCELED`
- [ ] Task properties parsed into `Task` (scheduled, deadline, priority, repeat, done)

**Verify** (`cargo test -p notatnik-core tasks`)
- [ ] `TODO x` is a task; `todo x`, `TODOx` and `- [ ] x` are not (unless you decide to support GFM checkboxes)
- [ ] An invalid date in `deadline::` → a diagnostic; the task still parses with `deadline: None`
- [ ] `repeat::` values from file-format.md §5 all parse; an unknown value → diagnostic

### P1.11 Robustness
- [ ] No `unwrap()`/`expect()`/indexing panics on user input in the parser (clippy lint `unwrap_used` enabled for the parser module)

**Verify**
- [ ] Property test: 10,000 arbitrary strings (incl. invalid UTF-8 boundaries via `&[u8]` → lossy) → no panic, and all spans are within bounds and on char boundaries
- [ ] Optional: `cargo fuzz run parse_page` for 30 minutes with no crash

## 1C: Page names and namespaces

### P1.12 Page name ⇔ file path
- [ ] `page_to_path(name) -> RelPath` and `path_to_page(path) -> Option<PageName>`; journals included
- [ ] Percent-encoding of unsafe characters, NFC normalisation, case-folded lookup key

**Verify** (`cargo test -p notatnik-core names`)
- [ ] Table: `work/projects/alpha` ↔ `pages/work/projects/alpha.md`; `a: b` ↔ `pages/a%3A b.md`; `100%` ↔ `pages/100%25.md`; `x.` (trailing dot) is encoded; `2026-10-03` ↔ `journals/2026/2026-10-03.md`
- [ ] `Work/Alpha` and `work/alpha` produce the same lookup key; NFC- and NFD-encoded "zażółć" produce the same key
- [ ] Property test: any valid page name → path → name round-trips exactly
- [ ] Paths outside `pages/` and `journals/` (e.g. `assets/x.md`, `.notatnik/…`) → `None`

### P1.13 Link target normalisation
All links are global (decided 2026-10-03), so a link target is always a full page name.

- [ ] `normalize_link(raw) -> Result<PageName, Diagnostic>`: trim whitespace around the name and around each `/` segment; collapse `//`; strip leading/trailing `/`; NFC-normalise

**Verify** (table test)
- [ ] `[[ work / projects/alpha ]]` → `work/projects/alpha`
- [ ] `[[/work/alpha/]]` → `work/alpha`; `[[work//alpha]]` → `work/alpha`
- [ ] `[[./x]]`, `[[../x]]`, `[[]]`, `[[/]]` → diagnostic ("relative links are not supported" / "empty page name")
- [ ] The same target written from pages in different namespaces resolves to the same page

## 1D: Edit operations

Edits are computed against the **source text** using spans, so untouched bytes never change (ST-3).

### P1.14 Edit operation set
- [ ] `BlockOp` enum: `Insert { parent, index, text }`, `UpdateText { block, text }`, `Delete { block }` (with children),
      `Indent`, `Outdent`, `Move { block, new_parent, index }`, `SetProperty`, `RemoveProperty`, `SetTaskState`,
      `EnsureBlockId`, `SetPageProperty`, `RemovePageProperty`
- [ ] `apply(source, page, op) -> Result<EditResult { new_source, patches: Vec<TextPatch> }, EditError>`
- [ ] `TextPatch { range, old_text, new_text }`, so every edit is reversible exactly (basis for undo, ED-6)
- [ ] New lines respect the file's line-ending and indentation style
- [ ] Ops address blocks by **path** (child indices) plus an expected content hash, so a stale op is rejected rather than applied to the wrong block

**Verify**
- [ ] Golden tests: `fixtures/edits/<op>/<case>/{before.md, op.json, after.md}`, byte-exact, ≥ 3 cases per op, including a CRLF file, a 4-space file and blocks with properties and children
- [ ] Property test (model-based): random tree + random sequence of 20 ops →
      (a) the new source reparses with no *new* diagnostics,
      (b) the reparsed tree equals the result of applying the same ops to a simple in-memory reference tree,
      (c) bytes outside the touched blocks are unchanged
- [ ] Inverse test: apply op → apply the inverse patches → byte-identical original
- [ ] A stale op (content hash mismatch) → `EditError::Stale`, source unchanged

### P1.15 No-op fidelity
- [ ] `parse` + "rebuild from spans" utility used by tests

**Verify**
- [ ] For every fixture file: rebuilt bytes == original bytes
- [ ] Same for every file of the synthetic graph (P1.38), run in `--full`

## 1E: Query language (parsing)

### P1.16 Query AST and parser
- [ ] Clauses from file-format.md §6: `task tag links ns page prop.<key> scheduled deadline text type sort limit group view columns`, `-key:` negation, comma = OR
- [ ] Errors carry line/column and a human-readable message, plus a suggestion where possible (e.g. "unknown key `tags`; did you mean `tag`?")
- [ ] `ns:` semantics (decided 2026-10-03): `ns: work` = the page `work` plus all pages below it at any depth; `page: work` = only that page. No wildcard syntax

**Verify** (`cargo test -p notatnik-core query_parse`)
- [ ] Table test: valid queries → expected AST (insta snapshots)
- [ ] Error table: unknown key, bad date, bad operator, empty value, duplicate `sort` → an error with the correct line/col
- [ ] Round trip: AST → canonical text → AST is identical (property test)

### P1.17 Relative dates
- [ ] `today yesterday tomorrow ±Nd ±Nw ±Nm ±Ny this-week next-week this-month`, resolved against an injected `today` and week start

**Verify**
- [ ] Table with fixed `today = 2026-10-03` (Saturday): `+7d` → 2026-10-10; `this-week` → 2026-09-28..2026-10-04 (Monday start); `-1m` from 2026-03-31 → 2026-02-28 (clamped)
- [ ] Leap year: `+1y` from 2028-02-29 → 2029-02-28

## 1F: WASM bindings

### P1.18 `crates/core-wasm`
- [ ] Separate crate that depends on `notatnik-core` + `wasm-bindgen`; exports `parse_page(text) -> JSON`, `parse_query(text) -> JSON`, `resolve_link(page, raw)`
- [ ] Added to `scripts/check.sh`

**Verify**
- [ ] `wasm-pack build crates/core-wasm --target web --release` succeeds
- [ ] Node/Vitest smoke test: `parse_page(fixture)` JSON equals the Rust-side snapshot JSON for 3 fixtures
- [ ] `.wasm` size recorded in `docs/benchmarks.md` (soft target: < 600 KB gzipped)

## 1G: SQLite index (`notatnik-index`)

### P1.19 Schema and cache versioning
- [ ] Schema from architecture.md §4, created on open; WAL mode; foreign keys on
- [ ] `PRAGMA user_version` = schema version. **On mismatch, delete and rebuild**: it's a cache, so no migrations are needed
- [ ] Cache path: `~/.local/share/notatnik/graphs/<graph-id>/index.sqlite` (passed in; the index crate doesn't decide paths)

**Verify**
- [ ] Open a new DB → tables exist (`sqlite3 index.sqlite .tables`)
- [ ] Set `user_version` to an old number manually → next open rebuilds (a test asserts a rebuild flag/log)

### P1.20 Full index
- [ ] Walk `pages/` and `journals/`, parse, insert pages/blocks/props/refs/aliases/FTS in one transaction per batch
- [ ] Parsing parallelised (`rayon`), writing on one thread

**Verify**
- [ ] For each fixture graph: an `expected-counts.json` (pages, virtual pages, blocks, refs by kind, tasks by state) matches the index
- [ ] Indexing the same graph twice → identical row contents (deterministic)

### P1.21 Incremental reindex
- [ ] `reindex_file(path)`: reparse one file, diff its blocks against the stored rows, apply the changes in one transaction
- [ ] File deleted → its rows removed; pages that are only referenced become virtual

**Verify**
- [ ] Integration test: index `basic`, change one block in one file, reindex → only that page's rows changed (row checksums of all other pages unchanged)
- [ ] Delete a file that other pages link to → the page still exists as virtual, and backlinks still work
- [ ] Benchmark: reindexing one 100-block file in the synthetic graph < 50 ms (NF-3)

### P1.22 Link resolution
- [ ] Resolve page refs (normalised via P1.13), tags and aliases to page IDs; create virtual pages for missing targets
- [ ] Namespace parents: `work/projects/alpha` creates virtual `work/projects` and `work` if missing, linked via `ns_parent_id`
- [ ] Virtual pages with no refs, no children and no file are garbage-collected

**Verify** (fixture `namespaces`, `refs`)
- [ ] A link to a non-existent page → a virtual page row exists
- [ ] Removing the last ref to a virtual page → the row disappears
- [ ] A deep namespace page creates all virtual ancestors; a recursive query from `work` returns all descendants
- [ ] A link via an alias resolves to the real page

### P1.23 Backlinks
- [ ] `backlinks(page) -> Vec<{source_page, block, breadcrumbs}>` (direct refs + tags + aliases)

**Verify**
- [ ] For fixture `refs`: an expected backlinks JSON for 5 pages matches, including breadcrumb paths (parent block texts)

### P1.24 Full-text search
- [ ] FTS5 with `unicode61 remove_diacritics 2`; block content without syntax noise (no `^id`, links as their label text)
- [ ] Results with highlighted snippets; title matches ranked above body matches

**Verify**
- [ ] `zolw` finds `żółw`; `ZAŻÓŁĆ` finds `zażółć`; `łódź` finds `lodz`
- [ ] Prefix search `proj*` finds `projekt`
- [ ] A page titled `Alpha` ranks above a page that only mentions alpha in a block
- [ ] Benchmark: a common-word search on the synthetic graph < 100 ms (NF-5)

### P1.25 Fuzzy page-title search
- [ ] Fuzzy matcher (e.g. `nucleo-matcher`) over page names, loaded in memory and updated on reindex

**Verify**
- [ ] `wpa` matches `work/projects/alpha`; `alpha` ranks `work/projects/alpha` above `alphabet-soup` when both exist
- [ ] Benchmark: 10,000 titles, a 3-character query < 10 ms

### P1.26 Stable runtime block identity
The UI needs stable keys for blocks without `^id`, across edits and external changes.

- [ ] Runtime IDs assigned per session; on reparse, match old → new blocks via a tree diff (same path + same content first, then content-hash matching, then position)
- [ ] Documented in architecture.md as a decision

**Verify**
- [ ] Editing one block's text keeps every other block's runtime ID
- [ ] Inserting a block at the top keeps all existing IDs
- [ ] Moving a block keeps its ID when its content didn't change
- [ ] An external change that rewrites the whole file still matches unchanged blocks to their old IDs

## 1H: Query execution

### P1.27 Query AST → SQL
- [ ] Compile to **parameterised** SQL only (values are never interpolated into SQL text)
- [ ] `today` injected by the caller

**Verify**
- [ ] `fixtures/graphs/queries/queries/*.query` each with `expected/*.json` (hand-verified result IDs, in order), ≥ 25 queries covering every clause and negation
- [ ] Injection test: `tag: x'); DROP TABLE blocks; --` → treated as a literal tag, no error, tables intact
- [ ] insta snapshots of generated SQL for 10 representative queries (catches accidental query-plan changes)

### P1.28 Namespace and date clauses
- [ ] `ns:` per the P1.16 decision; date ranges and comparisons; tasks without dates handled explicitly (`deadline: none`)

**Verify**
- [ ] `ns: work` returns blocks from `work` and every descendant page, and none from `workshop` or `workshop/x` (prefix trap)
- [ ] `page: work` returns only blocks on the page `work`
- [ ] `ns: work/projects` returns `work/projects` and `work/projects/alpha`, but not `work`
- [ ] `deadline: < +7d` with `today = 2026-10-03` returns exactly the expected tasks, including overdue ones

### P1.29 Sort, group, limit, performance
- [ ] Multi-key sort, grouping (page, property, task state, date), limit

**Verify**
- [ ] Expected-order tests for sort and group
- [ ] Benchmark: 5 typical queries on the synthetic graph, each < 100 ms (`EXPLAIN QUERY PLAN` shows index usage, recorded in benchmarks.md)

## 1I: Graph service (`notatnik-graph`)

### P1.30 Open and create a graph, config
- [ ] `Graph::create(path)`: folders + `.notatnik/config.toml` with a random graph ID
- [ ] `Graph::open(path)`: read config (unknown keys → warnings), open or rebuild the index, start reconciliation
- [ ] Device settings in `~/.config/notatnik/settings.toml` (list of graphs)

**Verify**
- [ ] `create` then `open` in a temp dir → works; `config.toml` has an ID; nothing except `.notatnik/` is created inside the graph
- [ ] Opening an arbitrary folder of plain `.md` files (not created by notatnik) works: it creates `.notatnik/` and indexes the files

### P1.31 Atomic, guarded writes
- [ ] `write_page(path, new_text, base_hash)`: compare the current file hash with `base_hash` → `Err(Conflict)` if different; else write the temp file in `.notatnik/tmp/`, fsync, rename, fsync the directory
- [ ] Leftover temp files are cleaned at startup
- [ ] Hash: `blake3` of the file bytes

**Verify**
- [ ] Test: read → an external write changes the file → `write_page` with the old hash → `Conflict`, and the file contains the external content
- [ ] Test: kill simulation, i.e. a temp file is left in `.notatnik/tmp/` → on next open it's removed and the original is intact
- [ ] Test: the target file is never observed partially written (a reader thread loops during 1,000 writes and never sees a truncated file)
- [ ] `.notatnik/tmp/` documented as a Nextcloud exclude (NC-2)

### P1.32 File watcher
- [ ] `notify` + debouncer (≈200 ms); events → `reindex_file`; emits `GraphChanged { pages }` on a channel
- [ ] Self-write suppression: ignore events whose resulting hash equals our last written hash for that path
- [ ] Handles the temp-file-plus-rename pattern used by Nextcloud and editors (vim, VS Code)

**Verify** (integration tests, temp dir)
- [ ] An external `std::fs::write` → `GraphChanged` within 1 s and the index updated (ST-5)
- [ ] Our own `write_page` → no `GraphChanged` event caused by the watcher
- [ ] 50 rapid writes to one file → coalesced into ≤ 3 reindexes, and the final state is correct
- [ ] Write a temp file then rename it over the target → a single update, no "deleted page" flicker
- [ ] Manual: edit a page in vim and in VS Code while `notatnik watch` runs → one update per save

### P1.33 Startup reconciliation
- [ ] On open: compare `(mtime, size)` per file with the `files` table; hash only when they differ; reindex changed, new and deleted files

**Verify**
- [ ] Index, close, then offline: modify 3 files, delete 1, add 1, touch 1 (mtime changed, content same) → on open exactly 5 files are reindexed (the touched file is hashed but not reindexed); asserted via counters

### P1.34 Rename page (with reference rewriting)
- [ ] `plan_rename(old, new) -> RenamePlan { file_moves, edits: Vec<(path, patches)> }` (preview, LN-7) and `apply(plan)`
- [ ] Rewrites `[[old]]`, `[[old|label]]`, `#old`, `#[[old]]` (any spelling that normalises/case-folds to the old name); if the new name contains characters not allowed in a plain tag (e.g. a space), `#old` becomes `#[[new name]]`
- [ ] Each file is written with a base-hash guard; if any write conflicts, stop and report which files were done (no silent partial state)

**Verify**
- [ ] Fixture: rename `people/anna` → `team/anna` → `notatnik check` reports 0 broken links; files that had no reference are byte-identical
- [ ] Renaming a namespace parent with children is rejected in this phase with a clear message (subtree moves come in NS-8, phase 5)
- [ ] Rename onto an existing page name → error, nothing changed

### P1.35 Conflict-file detection
- [ ] Files matching Nextcloud's conflict pattern (`* (conflicted copy *).md`) are listed separately and **not** indexed as pages

**Verify**
- [ ] Put `alpha (conflicted copy 2026-10-03 120000).md` in `pages/work/` → `notatnik check` lists it under conflicts; no page named like that appears in search

### P1.36 History snapshots (merge base)
- [ ] After every successful read-at-index and write, store the file content in `history/<path-hash>` (outside the graph); keep only the latest snapshot per file

**Verify**
- [ ] After a write, the snapshot equals the file on disk
- [ ] After an external change and reindex, the snapshot equals the new content
- [ ] Total history size ≈ graph size (no unbounded growth: 100 writes to one file → still 1 snapshot)

## 1J: CLI

### P1.37 `notatnik` commands
- [ ] `new-graph <path>`, `index <graph> [--rebuild]`, `search <graph> <text>`, `query <graph> <query|@file>`, `backlinks <graph> <page>`,
      `page <graph> <page> [--json]` (prints the parsed tree), `check <graph>`, `rename <graph> <old> <new> [--dry-run]`, `watch <graph>`
- [ ] `--json` output on all read commands; exit codes: 0 ok, 1 error, 2 check found issues
- [ ] `clap` with help text for every command

**Verify**
- [ ] The CLI snapshot test (`insta_cmd` or `assert_cmd`) runs each command against `fixtures/graphs/basic` and matches the snapshots
- [ ] `notatnik --help` and `notatnik <cmd> --help` are readable and complete
- [ ] The demo script (below) works by hand

## 1K: Synthetic graph and benchmarks

### P1.38 Synthetic graph generator
- [ ] `notatnik dev gen-graph --seed N --pages 10000 --out DIR` as described in [testing.md §3](../testing.md#3-synthetic-graph-performance)

**Verify**
- [ ] Same seed twice → identical tree hash (`find DIR -type f | sort | xargs sha256sum | sha256sum`)
- [ ] `notatnik check` on the output → 0 parse diagnostics, 0 broken links (except intentional ones the generator reports)

### P1.39 Benchmarks
- [ ] `criterion` benches: cold index, reindex one file, search, 5 queries, fuzzy title search, parse a 2,000-block file
- [ ] `docs/benchmarks.md` with a results table: date, commit, CPU/RAM/disk

**Verify**
- [ ] NF-2: cold index of 10k pages < 30 s
- [ ] NF-3: reindex one file < 50 ms
- [ ] NF-5: search < 100 ms
- [ ] Results committed to `docs/benchmarks.md`

---

## Demo script (manual acceptance)

```bash
notatnik new-graph ~/notatnik-test
# In a text editor, create pages/work/projects/alpha.md with blocks, a TODO with a deadline,
# a link to [[people/anna]], a #tag and a query block.
notatnik watch ~/notatnik-test &          # leave running in a second terminal
notatnik search ~/notatnik-test zolw      # diacritic-insensitive
notatnik query ~/notatnik-test 'task: todo
deadline: < +7d'
notatnik backlinks ~/notatnik-test people/anna
notatnik rename ~/notatnik-test people/anna team/anna --dry-run
notatnik rename ~/notatnik-test people/anna team/anna
notatnik check ~/notatnik-test            # 0 issues
git -C ~/notatnik-test diff               # if the test graph is in git: only the expected lines changed
```

- [ ] Each step behaves as described; the `watch` terminal shows an update for each save in the editor
- [ ] Put the test graph in your Nextcloud folder, edit a file on another device or in the Nextcloud web UI → `watch` picks it up

Record: date / commit / notes:
