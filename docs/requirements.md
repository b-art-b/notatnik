# Requirements

Status: **draft** · Last updated: 2026-10-03

Priorities:

- **MVP**: required for the first usable desktop version (daily driver on Linux).
- **V1**: required for the first "complete" release.
- **Later**: planned, not scheduled.

Each requirement has a stable ID so TODOs, commits and tests can refer to it.

---

## 1. Context and goals

A personal, local-first outliner/PKM app. Not a 1:1 Logseq clone and not compatible
with Logseq's file format. Goals:

1. Own the data: plain text files, readable and editable without the app.
2. Sync through existing infrastructure (Nextcloud), no custom sync server.
3. Integrate with daily life: calendar, tasks, Android access.
4. Extensible: plugins, scripting, custom views, AI.

Non-goals (for now): real-time multi-user collaboration, whiteboards, PDF annotation,
flashcards, Logseq/Obsidian compatibility, a hosted/web version.

## 2. Platforms

| ID | Requirement | Priority |
|---|---|---|
| PL-1 | Runs on Linux desktop (Ubuntu, X11 and Wayland). | MVP |
| PL-2 | Builds for Windows and macOS from the same codebase (no platform-specific features required). | Later |
| PL-3 | Android app sharing the same core and UI code, optimised for capture, journal, search and tasks. | Later |
| PL-4 | Fully functional offline; network only needed for sync, CalDAV and optional AI. | MVP |

## 3. Graph and storage

A **graph** is a folder of notes.

| ID | Requirement | Priority |
|---|---|---|
| ST-1 | Notes are stored as Markdown files in the graph folder, following [file-format.md](file-format.md). Files are the single source of truth. | MVP |
| ST-2 | All derived data (index, search, embeddings) lives in a rebuildable cache **outside** the graph folder (XDG data dir). Deleting it loses nothing. | MVP |
| ST-3 | Round-trip fidelity: files the app did not edit stay byte-for-byte unchanged. Editing a block rewrites only that block's text range. | MVP |
| ST-4 | Writes are atomic (temp file + rename) and never clobber a newer external change. If the file changed on disk since it was read, merge or raise a conflict instead of overwriting. | MVP |
| ST-5 | Detect external changes (Nextcloud client, text editor, git) and reindex them within ~1 s. | MVP |
| ST-6 | Detect Nextcloud conflict files (`* (conflicted copy *).md`) and show them in a "Conflicts" view with a side-by-side diff and merge helper. | V1 |
| ST-7 | Multiple graphs, switchable from the UI. | V1 |
| ST-8 | Assets (images, attachments) are stored in `assets/` and referenced by relative path. Supports paste and drag-and-drop. | V1 |
| ST-9 | Full reindex command plus an integrity check (broken links, duplicate block IDs, orphan assets). | V1 |
| ST-10 | Graph config is stored as a text file inside the graph (synced). Device-specific settings are stored outside it. | MVP |

## 4. Outliner editor

| ID | Requirement | Priority |
|---|---|---|
| ED-1 | Every page is a tree of blocks. Each block is a Markdown paragraph with optional children. | MVP |
| ED-2 | Logseq-style editing: the focused block shows raw Markdown in a CodeMirror editor; unfocused blocks render. | MVP |
| ED-3 | Keyboard-first: Enter = new block, Tab / Shift+Tab = indent / outdent, Alt+↑/↓ = move block, arrow keys move between blocks, Esc = leave editing. | MVP |
| ED-4 | Collapse / expand blocks; collapsed state persists (per device). | MVP |
| ED-5 | Multi-block selection (Shift+click / Shift+arrows), with copy, cut, delete, indent and move on the selection. | MVP |
| ED-6 | Undo / redo across block operations, not just within one block. | MVP |
| ED-7 | Autocomplete popup for `[[` (pages), `((` (blocks), `#` (tags) and `/` (slash commands). | MVP |
| ED-8 | Inline Markdown rendering: bold, italic, code, strikethrough, highlights, links, images, tables, code blocks with syntax highlighting. Raw HTML is never rendered; it is shown escaped, as literal text. | MVP |
| ED-9 | Block zoom: focus on a single block and its children, with breadcrumbs back to the page. | MVP |
| ED-10 | Drag-and-drop of blocks (within and between pages). | V1 |
| ED-11 | Paste of multi-line Markdown and HTML converts into a block tree. | V1 |
| ED-12 | Math (KaTeX) and Mermaid diagrams in code blocks. | Later |
| ED-13 | Command palette (Ctrl+K) for navigation and all commands. | MVP |
| ED-14 | Light and dark themes; user CSS override. | V1 |

## 5. Pages, links and namespaces

| ID | Requirement | Priority |
|---|---|---|
| LN-1 | `[[Page name]]` links to a page. Linking to a page that doesn't exist creates it lazily, on first content. | MVP |
| LN-2 | `#tag` and `#[[multi word tag]]` are links to the page of the same name. Tags may contain letters (including Polish), digits, `_`, `-` and `/`: `#123`, `#tag_name`, `#tagName`, `#tag-name` and `#work/alpha` are all tags. | MVP |
| LN-3 | Backlinks panel on every page, grouped by source page with context (parent breadcrumbs). | MVP |
| LN-4 | "Unlinked references": plain-text mentions of the page title, with a one-click link action. | V1 |
| LN-5 | Block references `((id))` render the referenced block inline (read-only), and embeds render it editable. Block IDs are created on demand. | MVP |
| LN-6 | Page aliases (frontmatter `aliases`); links via an alias resolve to the page. | V1 |
| LN-7 | Rename a page and update all references across the graph, with a preview of affected files. | MVP |
| LN-8 | Case-insensitive page name matching; the display name keeps its original case. | MVP |
| LN-9 | Graph view (pages and links) with namespace filtering. | Later |

### Namespaces (key feature)

| ID | Requirement | Priority |
|---|---|---|
| NS-1 | Page names may contain `/` to form namespaces, e.g. `[[work/projects/alpha]]`. Each segment maps to a folder on disk. | MVP |
| NS-2 | Parent namespace pages exist implicitly (virtual) and become real files once given content. | MVP |
| NS-3 | Sidebar namespace tree: expand/collapse, create a child page, rename/move a subtree, page counts. | MVP |
| NS-4 | Breadcrumbs on every namespaced page; each segment is clickable. | MVP |
| NS-5 | Namespace page shows its children (list or table view, with properties) above its own content. | MVP |
| NS-6 | ~~Relative links~~ **Removed (2026-10-03):** all links are global (full page name from the graph root). `[[./x]]`-style links are not supported. | — |
| NS-7 | Autocomplete ranks pages in the current namespace and its siblings first; typing `/` after a segment drills into it. | MVP |
| NS-8 | Moving or renaming a namespace moves the folder and rewrites all absolute links to pages inside it. | V1 |
| NS-9 | Namespace-scoped queries and search: `ns: work` = `work` and all pages below it (decided 2026-10-03). | MVP |
| NS-10 | Namespace templates: a per-namespace default template for new child pages (e.g. every `people/*` page starts with a set of fields). | V1 |

## 6. Journals

| ID | Requirement | Priority |
|---|---|---|
| JR-1 | Today's journal page opens on startup and is created on first edit. | MVP |
| JR-2 | Journals view: scrolling list of recent days. | MVP |
| JR-3 | Date links `[[2026-10-03]]` resolve to journal pages; natural-language date picker (`/today`, `/tomorrow`, `/next monday`). | MVP |
| JR-4 | Calendar widget for navigating journals; days with content are marked. | V1 |
| JR-5 | Journal page shows that day's calendar events (from CalDAV) and tasks scheduled for or due that day. | V1 |
| JR-6 | Configurable journal template. | V1 |

## 7. Tasks

| ID | Requirement | Priority |
|---|---|---|
| TK-1 | A block starting with `TODO`, `DOING`, `WAITING`, `DONE` or `CANCELED` is a task. Ctrl+Enter cycles TODO → DOING → DONE. | MVP |
| TK-2 | Tasks have optional `scheduled`, `deadline` (date or date+time), `priority` (high/medium/low) and `repeat` (recurrence). | MVP |
| TK-3 | Completing a repeating task appends the date to a hidden `completed::` property, resets it to TODO and moves `scheduled`/`deadline` to the next occurrence. | V1 |
| TK-4 | Agenda view: overdue, today, upcoming 7 days, no date. Filters by namespace, tag and priority. | MVP |
| TK-5 | Completion timestamp recorded when a task is marked DONE. | MVP |
| TK-6 | Subtask progress shown on parent tasks (e.g. 2/5). | V1 |

## 8. Queries and views

| ID | Requirement | Priority |
|---|---|---|
| QV-1 | Query blocks (```` ```query ```` fenced code blocks) render live results that update as the graph changes. | MVP |
| QV-2 | Query language filters: task state, tags, links-to, namespace, properties (=, !=, <, >, contains, exists), dates (relative: `today`, `+7d`, `-1w`), full-text, page vs. block, and boolean and/or/not. | MVP |
| QV-3 | Sorting, limits and grouping (by page, property, task state, date). | MVP |
| QV-4 | Built-in views: **list** (blocks with context) and **table** (columns = properties). | MVP |
| QV-5 | **Kanban** view (group by property or state; dragging a card updates the property in the file). | V1 |
| QV-6 | **Timeline / calendar** view over date properties. | V1 |
| QV-7 | Query results are editable in place (list view embeds blocks). | V1 |
| QV-8 | Saved queries usable as sidebar entries. | V1 |
| QV-9 | Plugins can register new view types (see PG-*). | V1 |
| QV-10 | Query errors are shown inline with position info; never crash the page. | MVP |

## 9. Properties

| ID | Requirement | Priority |
|---|---|---|
| PR-1 | Page properties in YAML frontmatter; block properties as `key:: value` lines. | MVP |
| PR-2 | Property values: text, number, date, boolean, page link(s), list. Type inferred, optionally declared in config. | MVP |
| PR-3 | Property autocomplete (keys and existing values). | V1 |
| PR-4 | Hidden/system properties (e.g. `id`) are not rendered in normal view. | MVP |

## 10. Search

| ID | Requirement | Priority |
|---|---|---|
| SR-1 | Full-text search over pages and blocks (SQLite FTS5) with highlights; diacritic-insensitive (Polish characters!). | MVP |
| SR-2 | Fuzzy page-title search in the command palette and autocomplete. | MVP |
| SR-3 | Semantic search (embeddings), merged with full-text results (hybrid ranking). | V1 |
| SR-4 | Search filters reuse the query language (`ns:`, `tag:`, `task:`). | V1 |

## 11. Calendar integration (CalDAV / Nextcloud)

| ID | Requirement | Priority |
|---|---|---|
| CA-1 | Connect to a CalDAV server (Nextcloud) with URL, user and app password. The password is stored in the OS keyring. | V1 |
| CA-2 | Choose which calendars to read events from and which calendar receives tasks. | V1 |
| CA-3 | Read events into the local cache; show them in the journal (JR-5) and timeline views. | V1 |
| CA-4 | Tasks with `scheduled` or `deadline` are pushed as VTODO (configurable: VTODO or VEVENT). | V1 |
| CA-5 | Two-way: changes made in the calendar or task app (Nextcloud Tasks, Thunderbird, DAVx⁵ on Android), such as completion, date, title or priority, are written back to the block in the Markdown file. | V1 |
| CA-6 | Stable mapping of block ↔ calendar item that survives reindex and works across devices (derived from the block ID, no local-only state). | V1 |
| CA-7 | Incremental sync (sync-token / ETag); periodic and on-demand. | V1 |
| CA-8 | Conflict policy: last-modified wins per field, with conflicts logged and shown in a sync log view. | V1 |
| CA-9 | Deleting a task block (or un-dating it) removes or archives the calendar item, following a config setting. | V1 |
| CA-10 | Create a note page from a calendar event (meeting notes) with a link back to the event. | Later |

## 12. Nextcloud file sync

| ID | Requirement | Priority |
|---|---|---|
| NC-1 | Desktop: the graph folder is synced by the Nextcloud desktop client. The app is sync-agnostic and relies on ST-4, ST-5 and ST-6. | MVP |
| NC-2 | Temp files used for atomic writes are excluded from Nextcloud sync (documented `.sync-exclude.lst` entry). | MVP |
| NC-3 | Built-in WebDAV sync (for Android and for desktops without the client), behind a sync abstraction. | Later |

## 13. Plugins and scripting

| ID | Requirement | Priority |
|---|---|---|
| PG-1 | TypeScript plugin API: register commands, slash commands, views, sidebar panels, block renderers (e.g. for fenced block languages) and event hooks (block changed, page opened, task completed). | V1 |
| PG-2 | Plugins run sandboxed (Web Worker for logic, iframe for UI) and talk to the app only through the API. | V1 |
| PG-3 | Permission model: graph read, graph write, network (per domain), calendar. Granted by the user at install time. | V1 |
| PG-4 | Plugins are stored in the graph (`.notatnik/plugins/`) so they sync, and can be enabled per device. | V1 |
| PG-5 | User scripts: small snippets triggered by commands or hooks, without packaging a full plugin. | V1 |
| PG-6 | WASM plugins (Extism) on the Rust side for heavy processing or non-JS languages (Rust, Python via extism-py). | Later |
| PG-7 | Plugins work on Android (no desktop-only APIs in the core plugin API). | Later |

## 14. AI features (optional, off by default)

| ID | Requirement | Priority |
|---|---|---|
| AI-1 | Provider abstraction: Ollama (local/home server) first; cloud providers optional and explicitly configured. | V1 |
| AI-2 | Background embedding of blocks/pages into sqlite-vec; incremental on change. | V1 |
| AI-3 | Semantic search and "related notes" panel. | V1 |
| AI-4 | "Chat with notes" (retrieval-augmented), answers cite source blocks as clickable references. | V1 |
| AI-5 | Block actions: summarise, rewrite, extract tasks, suggest tags/links. | Later |
| AI-6 | Privacy: no note content leaves the device unless a remote provider is configured; per-namespace exclusion (e.g. `private` and everything below it). | V1 |

## 15. Other integrations (via plugins where possible)

| ID | Requirement | Priority |
|---|---|---|
| IN-1 | Web clipper / share target (Android share sheet, then the Linux browser extension). | Later |
| IN-2 | RSS / read-later import into a namespace. | Later |
| IN-3 | Email-to-note (IMAP folder to inbox page). | Later |
| IN-4 | Nextcloud Deck / Notes import or export. | Later |
| IN-5 | Export: page or namespace to HTML / PDF; publish a static site from a namespace. | Later |

## 16. Android (later phase)

| ID | Requirement | Priority |
|---|---|---|
| AN-1 | Same core (Rust) and UI (Svelte) via Tauri Android. Mobile layout with touch-friendly block editing. | Later |
| AN-2 | Quick capture to today's journal (home-screen widget / share target). | Later |
| AN-3 | Graph stored in app storage, synced via built-in WebDAV (NC-3). | Later |
| AN-4 | Reads tasks and calendar through the same CalDAV sync (no dependency on DAVx⁵). | Later |

## 17. Non-functional requirements

| ID | Requirement | Target |
|---|---|---|
| NF-1 | Scale | 10,000 pages / 200,000 blocks without noticeable slowdown. |
| NF-2 | Cold full index | < 30 s for NF-1 size on a mid-range laptop; app usable during indexing. |
| NF-3 | Incremental reindex of one changed file | < 50 ms. |
| NF-4 | Typing latency in a block | < 16 ms (one frame). |
| NF-5 | Full-text search | < 100 ms for NF-1 size. |
| NF-6 | Startup to editable journal | < 1.5 s with a warm index. |
| NF-7 | Data safety | Never lose user input: atomic writes, crash-safe, conflicts surfaced, never silently resolved by deletion. |
| NF-8 | Privacy | No telemetry. Network calls only to user-configured endpoints. |
| NF-9 | Testability | Core (parser, index, query, sync logic) is UI-free and covered by unit and property tests. Golden-file tests guarantee format round-trip fidelity. |
| NF-10 | i18n | UI strings externalised; English first, Polish later. Unicode-correct everywhere (page names, search). |
| NF-11 | Accessibility | Full keyboard operation; sensible focus handling; screen-reader labels on controls. |

## 18. Open questions

- Should journals also be available as a namespace (`journal/2026/10/03`) for queries, or stay separate?
- Kanban drag: should moving a card between states rewrite the task marker, a property, or both (configurable)?
- ~~Should the plugin API expose raw file access, or only the block/page model?~~ Proposed in phase 6: model-level only in v1.
- ~~What should the conflict-merge UX look like?~~ Proposed in phase 5 (P5.1–P5.2): block-level 3-way merge using history snapshots as the base.
- Kanban drag and journals-as-namespace remain open; see phase 3 and P1.1.
