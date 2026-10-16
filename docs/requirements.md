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
| BK-1 | A snapshot is one zip file of the graph folder (all files except `.notatnik/tmp/`), written next to nothing else of the graph: in a backup folder outside it. A snapshot restores to a folder that opens as a graph; the index is rebuilt. | V1 |
| BK-2 | The backup folder is a device setting (absolute path, never inside the graph folder); empty means `<data dir>/backups`. One folder serves all graphs: file names carry the graph id. | V1 |
| BK-3 | Automatic snapshots (device setting, on by default): shortly after a graph opens and then every hour a check takes one when the newest is at least a day old and the files changed. A failed run is shown in Settings; it never interrupts editing. | V1 |
| BK-4 | Retention: the newest snapshot of each of the last 7 days, 4 weeks and 6 months is kept, the rest is removed. A snapshot a page is being restored from is kept. | V1 |
| BK-5 | Settings → Backup: folder, automatic on/off, *Back up now*, the list of snapshots, restore of one page (the current file is replaced after a new snapshot of it was taken) or of everything into a new or empty folder. | V1 |
| BK-6 | Before a rename that touches several files or a delete of several pages, a restore point is saved (a snapshot named `...-before.zip`, only if automatic backups are on and the files changed since the newest snapshot). The newest ten are kept apart from the day/week/month scheme. Replacing a page from a snapshot saves one too. | V1 |
| ST-11 | Multi-file operations (rename with reference rewriting, namespace move, delete of several pages) are journaled outside the graph before their first step. If the app dies in the middle, the next open undoes the partial operation (or only drops the record when it had completed) and says so. | V1 |
| ST-12 | The app remembers the graphs opened on this device (device setting, never in a graph): the start screen lists them, the most recent first with the last opened selected (Enter opens it), shows a missing folder as such and removes an entry from the list on request (the folder, its notes, cache and backups are not touched). *Settings → Graph → Open the last graph at start* skips the start screen. *Switch graph…* (palette, Settings) shows it again with a way back. Opening a graph with the CLI does not add it to the history. | V1 |
| VH-1 | Every file the app writes, and every change from outside it notices, is recorded in a per-page version history on this device (outside the graph, never synced); the text before the first change is kept too. | V1 |
| VH-2 | Versions thin out: all of the last 10 minutes (bursts of typing are one), then one per 10 minutes for 3 hours, one per hour for 2 days, one per day for 30 days; the oldest is kept for 30 days. The history follows a renamed or moved page. | V1 |
| VH-3 | *History* under a page's title (journal days too) lists the versions, shows what restoring would change as a line diff and restores one; the text it replaces becomes a version. | V1 |

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
| ED-15 | The path to the focused block is highlighted: the guide line turns dark from the top of its siblings down to an elbow into each ancestor's and the block's own bullet. | V1 |
| ED-16 | The `/` menu is grouped (Tasks, Dates, Task details, Blocks) while empty and a relevance-ranked list once typed; rows show an icon, label, description and the `/name`; matches names, keywords and descriptions; shows key hints. | V1 |
| ED-17 | Date pickers (`/date`, `/scheduled`, `/deadline`, task chips) accept typed or natural-language dates, quick choices and an own calendar grid; every date in them is `YYYY-MM-DD`. | V1 |
| ED-23 | Right-click (or Alt+click) on a block's bullet opens a menu: copy a reference `((id))` (adding the block ID when missing), copy as embed `!((id))`, copy the text or the block with children as Markdown, zoom in. | V1 |
| ED-24 | New page through the command palette (Ctrl+K): a name that matches no page offers *Create page “name”* (the last row; Ctrl+Enter always creates the typed name; `a/b` for a namespace); the page opens with its first block ready for typing, no file exists until something is typed; an existing name is opened instead; an invalid name is explained in the palette. | V1 |
| ED-25 | Multi-block selection: range (Shift+click, Shift+↑/↓ over every visible block), toggle (Ctrl+click), drag across blocks, select all (Ctrl+A, twice in a block), with copy (no IDs), cut (keeps IDs), paste below the selection with the structure, duplicate (Ctrl+D), delete at any level, move and indent siblings together, copy references (Ctrl+Shift+C), cycle task state, and a right-click menu; a selected block takes its children along; each command is one undo step. | V1 |
| ED-18 | Settings page organised by topic (Appearance, Dates & language, Tasks, Graph), each pane stating whether its values are per device or per graph; changes apply at once and are confirmed inline. Device settings: theme, zoom, brackets around rendered links (off by default), date language, date format (presets or a custom pattern, [date-formats.md](date-formats.md)). Graph settings: week start, completion dates kept. | V1 |
| ED-19 | Mouse navigation like Firefox: back/forward mouse buttons and Alt+←/→ move through the page history; middle click or Ctrl+click on a page link opens it in the right sidebar; clicking a day's title in the journals feed opens that day as a page. | V1 |
| ED-20 | Blocks can be reordered with the mouse: drag a bullet (with all its sub-bullets) above, below or into another block of the same page; a drop line shows where it lands; undo restores it. | V1 |
| ED-21 | Emojis: `:shortcode` completion in the editor inserts the emoji character, `/emoji` opens a searchable picker; notes store plain Unicode, never shortcodes. | V1 |
| ED-22 | A "Recent" list in the sidebar with the pages opened last (per device, renames follow). | V1 |

## 5. Pages, links and namespaces

| ID | Requirement | Priority |
|---|---|---|
| LN-1 | `[[Page name]]` links to a page. Linking to a page that doesn't exist creates it lazily, on first content. | MVP |
| LN-2 | `#tag` and `#[[multi word tag]]` are links to the page of the same name. Tags may contain letters (including Polish), digits, `_`, `-` and `/`: `#123`, `#tag_name`, `#tagName`, `#tag-name` and `#work/alpha` are all tags. | MVP |
| LN-3 | Backlinks panel on every page, grouped by source page with context (parent breadcrumbs). | MVP |
| LN-4 | "Unlinked references": plain-text mentions of the page title (and aliases) outside markup, listed below the linked references, with a one-click link action. | V1 |
| LN-5 | Block references `((id))` render the referenced block inline (read-only), and embeds render it editable. Block IDs are created on demand. | MVP |
| LN-6 | Page aliases (frontmatter `aliases`); links via an alias resolve to the page. | V1 |
| LN-7 | Rename a page and update all references across the graph, with a preview of affected files. | MVP |
| LN-8 | Case-insensitive page name matching; the display name keeps its original case. | MVP |
| LN-9 | Graph view (pages and links) with namespace filtering. | Later |
| LN-10 | A linked reference shows the linking block with all its sub-bullets (editable in place) and its ancestor path as links that open the page at that block. A block below another referencing block of the same page is part of that one, not a second reference. | V1 |
| LN-11 | A link or tag on a block applies to everything below it: queries (`links:`, `tag:`), the page's "Mentioning this page" tasks and the agenda tag filter match descendants too. `direct: yes` limits a query to blocks that carry the link themselves. Dates and other properties are not inherited. | V1 |

### Namespaces (key feature)

| ID | Requirement | Priority |
|---|---|---|
| NS-1 | Page names may contain `/` to form namespaces, e.g. `[[work/projects/alpha]]`. Each segment maps to a folder on disk. | MVP |
| NS-2 | Parent namespace pages exist implicitly (virtual) and become real files once given content. | MVP |
| NS-3 | Sidebar namespace tree: expand/collapse, create a child page, rename/move a subtree, page counts. | MVP |
| NS-4 | Breadcrumbs on every namespaced page; each segment is clickable. | MVP |
| NS-5 | Namespace page shows its children: the **Hierarchy** section below the content lists every page below it (NS-15). | MVP |
| NS-6 | ~~Relative links~~ **Removed (2026-10-03):** all links are global (full page name from the graph root). `[[./x]]`-style links are not supported. | — |
| NS-7 | Autocomplete ranks pages in the current namespace and its siblings first; typing `/` after a segment drills into it. | MVP |
| NS-8 | Moving or renaming a namespace moves the folder and rewrites all absolute links to pages inside it. | V1 |
| NS-9 | Namespace-scoped queries and search: `ns: work` = `work` and all pages below it (decided 2026-10-03). | MVP |
| NS-10 | Namespace templates: a per-namespace default template for new child pages (e.g. every `people/*` page starts with a set of fields). | V1 |
| NS-11 | "All pages" view: a namespace tree and a flat list that can be filtered by name and sorted by name, block count, backlinks and modification time; journals and pages without a file can be hidden. | V1 |
| NS-12 | The left panel can group pages by a page property instead of by namespace (`type: person`, `type: client` → a *person* group and a *client* group); the property is picked from the keys in use and remembered per device; pages without it are in a folded last group; list values put a page in several groups. | V1 |
| NS-13 | Page properties can be added, changed and removed in the page header (they are the YAML frontmatter of the file); values already used for a key are suggested. | V1 |
| NS-14 | Clicking a page property in the page header opens a view of all pages that have it: the key lists them grouped by value, a value (one link per item of a list) lists the pages with that value; the other values are links. | V1 |
| NS-15 | A page shows a **Hierarchy** section (like Logseq): every page below it at any depth, each as its path with every segment a link. | V1 |
| NS-16 | Renaming a page with subpages moves them with it (all links follow); "Move the subpages too" can be switched off to rename only the page, which also allows a name below the old one (`a` → `a/b/c`). | V1 |
| NS-17 | Page icons: rules in the graph (`type = person` → 👤, synced) and a page's own `icon:` property give pages an emoji, shown next to the page in titles, lists and links. | V1 |
| NS-23 | A journal day has the default page property `type: journal` unless its frontmatter sets a `type`: derived in the index, never written to the file. Icon rules, page queries (`prop.type: journal`) and the properties section see it; the calendar glyph is only the fallback when no icon applies. Today's journal, which has no file until something is written, gets the journal rule's icon too, so the feed's titles match. | V1 |
| NS-18 | Every segment of a page name is an automatic, indirect tag: a segment that occurs in more than one place (`Renewal` in `P/Capgemini/Renewal/…`, `P/EY/Renewal/…`, `X/Renewal/…`) is marked with its count (in the Hierarchy, in the path above a page's title and next to its title) and opens a view of all its places, the pages below them, their open tasks and the explicit `#renewal` references. The query clause `path: Renewal` selects the same pages (and, in block queries, their blocks). | V1 |
| NS-19 | Changing one non-final segment of a page's name in the rename dialog offers (ticked by default) to rename that namespace for all pages below it; unticked, or with other changes, only the page moves. | V1 |
| NS-20 | Archive a page: `archived: true` in its frontmatter hides it and everything below it from the sidebar, property groups, Recent and All pages (unless asked for) and its tasks from the agenda; it is reversible and nothing moves. | V1 |
| NS-21 | Delete a page (optionally with its subpages): the file goes to a trash outside the graph folder (30 days), links stay, one Undo restores it. | V1 |

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
| TK-4 | Agenda view: overdue, today, upcoming 7 days, later (dated beyond 7 days), no date. Filters by namespace, tag and priority. | MVP |
| TK-5 | Completion timestamp recorded when a task is marked DONE. | MVP |
| TK-6 | Subtask progress shown on parent tasks (e.g. 2/5). | V1 |
| TK-7 | Task logbook: dated progress notes in a `:LOGBOOK:` drawer ([file-format.md](file-format.md) §3.6), added with `/log`, shown as a collapsible list and hidden from the block text. | V1 |
| TK-8 | A task in the agenda shows the note it lives in as a link; following it opens that note and flashes the task, like a search result. | V1 |
| TK-9 | A page has a **Tasks** section between its content and its references: its own open tasks, the tasks of its subpages, and the tasks elsewhere that link to it; done tasks on request; each task can be changed in place and links to its note. | V1 |
| TK-10 | The agenda can be shown through the query views (list, table, kanban, timeline) with the same filters, plus done tasks, grouping and sorting; the query it runs can be shown and saved. | V1 |
| TK-11 | The priority of a task is shown right after the task marker as `[#A]`; the labels are configurable in `config.toml` (`[tasks] priorities`) and in Settings, any number from one to nine (`A`, `B`, `C` by default); a task has no priority by default; each priority has a `/` command; the query clause `priority: A` and the agenda filter use the labels; `high`/`medium`/`low` in files mean the first, middle and last label. | V1 |
| TK-12 | A task listed outside its note (agenda sections, the page's Tasks section) shows its source note in the task's own first row, right-aligned, so it can only be read as belonging to that task; the full parent path is in the tooltip and a click opens the note with the task flashed. | V1 |
| TK-13 | Device setting *Task details: compact*: on pages and journals a task's date (the deadline, else the scheduled date; words for overdue, today and tomorrow, red when overdue), repeat and logbook are small indicators at the end of its first row; the usual chips and logbook open on hover, focus or click. The agenda, page tasks, query results and linked references always show everything. | V1 |
| TK-14 | `scheduled` and `deadline` can carry a start time and an end time (`2026-10-14 09:00-10:30`), none by default. The date picker (`/scheduled`, `/deadline`, the chips) has a *Time of day* switch with start, end and quick durations (24 h); an end not after the start is refused. Chips and compact indicators show the time, the agenda orders a day by start time (tasks without one last) and the timeline marks the time on the task (no duration drawn). Dates stay the unit of overdue, queries and sorting. | V1 |
| TK-15 | `/followup` marks the task done (completion time and repeat as usual) and creates the next task directly after it: open (`TODO`), with the tags, page links and priority of the finished one (not its dates or repeat), the editor in it, and `follows:: ((id))` pointing back (the finished task gets a block ID when it has none). Tasks linked by `follows::` form a chain: a task in one shows a chain button that lists the whole chain in order (branches indented, a gone task marked), and the query clause `chain: ((id))` finds all of it. | V1 |

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
| QV-11 | A query block in a note is edited by clicking anywhere on it except its results: the header (with a visible hint), the frame or the shown query text open the block for editing; a click on a result keeps its own meaning. | V1 |
| QV-12 | `{{page}}` in a query means the page the query block sits on, so one query (`type: page`, `prop.partner: {{page}}`) works unchanged on every partner page; `{{title}}`, `{{namespace}}` and `{{date}}` are filled in too. Outside a page (agenda, sidebar) `{{page}}`, `{{title}}` and `{{namespace}}` are an error. The empty link `[[]]` still means the page. | V1 |
| QV-13 | The placeholders of templates and queries are documented (`docs/placeholders.md`), listed in Settings → Templates, and offered by completion after `{{` in a query block. | V1 |

## 9. Properties

| ID | Requirement | Priority |
|---|---|---|
| PR-1 | Page properties in YAML frontmatter; block properties as `key:: value` lines. | MVP |
| PR-2 | Property values: text, number, date, boolean, page link(s), list. Type inferred, optionally declared in config. | MVP |
| PR-3 | Property autocomplete (keys and existing values). | V1 |
| PR-5 | A query can let block properties apply to everything below the block that has them: `inherit: client, type`. The nearest block with the key decides (its own value wins), the page's frontmatter is the root; `direct: yes` and page queries ignore it; dates, priority and task data are never inherited. | V1 |
| PR-6 | Block properties are shown as a compact card under the block: bold key, value (links open the page, other values open the view of all blocks with that value), the key opens the view of all blocks with the property (optionally with the blocks below them). Edit, remove and add happen in the card's row and change only that line of the file. | V1 |
| PR-7 | Typing `::` on a line after the first (or `key::`, or `/property`) opens a menu of known keys with a "Create property" row for a new one; the value menu then lists the values used for the key and pages (written as `[[Page]]`). | V1 |
| PR-8 | Device setting *Block properties: compact*: on pages and journals a block's property card becomes one small `⋯ n` indicator at the end of its first row (the properties as its tooltip); the card, with edit, remove and add, opens on hover, focus or click. Independent of *Task details*. The agenda, query results and linked references always show the card. | V1 |
| PR-9 | A property value can be a page: value fields of page properties and of the block card suggest used values and matching pages; a page is written as a link (quoted in frontmatter). Query blocks offer the same for `prop.<key>:`, and `prop.<key>: [[Page]]` filters by it. | V1 |
| PR-10 | A page type has a template, `templates/<type>.md` in the graph (name compared without regard to case). Setting `type` on a page adds the template's properties the page lacks (never changing a set value) and, for a page without blocks, its blocks, in the same edit (one undo step). A button in the page properties adds fields later added to the template. Variables `{{title}}`, `{{namespace}}`, `{{date}}`, `{{cursor}}`. A device setting turns the automatic step off. A page becomes the template of its type with **Save as template…** in its properties (fields empty or with the values as defaults; replacing asks first and keeps the old text in the history); *Settings → Templates* lists and deletes templates; a deleted template can be restored for 30 days. A new page below a namespace with a configured template, and a new journal day when a journal template is set, start from that template (its `type`, properties and blocks; the user's first block comes after the template's). `/template-<name>` inserts a template's blocks after the current block (in an empty block the first one takes its place); the page also gets the template's properties it lacks. | V1 |
| PR-4 | Hidden/system properties (e.g. `id`) are not rendered in normal view. | MVP |

## 10. Search

| ID | Requirement | Priority |
|---|---|---|
| SR-1 | Full-text search over pages and blocks (SQLite FTS5) with highlights; diacritic-insensitive (Polish characters!). | MVP |
| SR-4 | The search box takes boolean syntax: words (a space is AND, or `AND`, `&`), `OR` / `\|`, `-word` / `NOT` / `!word`, parentheses, "phrases" and `prefix*`. NOT binds tightest, then AND, then OR; `AND`, `OR`, `NOT` are operators only in capitals. Only the words that must match are highlighted. A mistake (unclosed parenthesis, only negative words, a negation inside an OR) is explained in the dialog while the last results stay. | MVP |
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
