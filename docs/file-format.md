# File format (draft spec)

Status: **draft v0**. It will change until the parser exists. Not Logseq-compatible by design.

Design goals:

1. Readable and editable in any text editor or the Nextcloud web UI.
2. Plain CommonMark/GFM as far as possible; extensions degrade gracefully.
3. Minimal noise: metadata is added only when needed (e.g. block IDs only when referenced).
4. Merge-friendly: line-oriented, so Nextcloud conflicts and git diffs stay small.

---

## 1. Graph folder layout

```
<graph>/
├── journals/
│   └── 2026/
│       └── 2026-10-03.md        # one file per day, grouped by year
├── pages/
│   ├── inbox.md                 # page "inbox"
│   └── work/
│       ├── projects.md          # page "work/projects" (optional, namespace page)
│       └── projects/
│           └── alpha.md         # page "work/projects/alpha"
├── assets/
│   └── 2026/10/diagram-a1b2c3d4.png   # <name>-<first 8 hex of the blake3 of the content>.<ext>; pasted: pasted-<hash8>.png
└── .notatnik/                   # synced graph config
    ├── config.toml
    ├── templates/               # page/block templates (Markdown)
    ├── plugins/                 # installed plugins
    └── tmp/                     # atomic-write temp files (excluded from sync)
```

Outside the graph (per device, never synced):

```
~/.local/share/notatnik/graphs/<graph-id>/
├── index.sqlite                 # blocks, links, properties, FTS5, sqlite-vec
├── history/                     # last-synced copies for 3-way merge
└── state.json                   # collapsed blocks, open tabs, window layout
~/.config/notatnik/settings.toml # device settings; the history of graphs opened in the app (`[[graphs]]` id + path, the most recent last), `last_graph` (its id), a `color` (0 to 7) per graph if the person chose one (the default comes from the graph's id) and `[ui] open_last_graph`; written by the app only, the CLI does not add graphs
~/.local/share/notatnik/replicas/<name>/  # a graph kept on a WebDAV server: its local copy (WebDAV, SY-1); <name>.sync-state.json beside it holds the sync state
~/.config/notatnik/backup-vault.json # the public descriptor of the backup encryption vault (EN-2, no secret that works without the password or recovery key); `backup-vault.pending.json` until the recovery key is confirmed
~/.local/share/notatnik/backups/ # snapshots (default folder; the device setting `backup_dir` can change it)
~/.local/share/notatnik/graphs/<graph-id>/journal/pending.json   # operation in progress (D38); exists only after a crash
~/.local/share/notatnik/graphs/<graph-id>/versions/<hash>/<ms>.md # page history (D39); `<ms>-w.md` = written by the app
```

A snapshot is `notatnik-<graph id>-<YYYYMMDD>-<HHMMSS>.zip` (local time) with the graph folder's files under their
relative paths, minus `.notatnik/tmp/`. The zip comment is `notatnik-backup v1 <fingerprint>`. Unzipping one gives a
folder that opens as the graph (BK-1). Besides the graph's files a snapshot holds the device's configuration under `.notatnik-device/` (`settings.toml`, `state.json`; BK-8), which unpacking leaves out. Entries are deflated or only stored (BK-7). The device setting `backup_mirror_dir` names an optional second folder that gets a copy of every snapshot (BK-9). Settings → Backup compares a snapshot with the current files line by line before a restore (BK-10). A snapshot taken before a bulk change ends in `-before.zip` (BK-6).

## 2. Page names and file paths

- A page name is its path relative to `pages/`, without `.md`, using `/` as namespace separator:
  `pages/work/projects/alpha.md` ⇔ `work/projects/alpha`.
- Matching is case-insensitive and Unicode NFC-normalised. The display name comes from the
  frontmatter `title` if present, else the file name.
- Characters unsafe on Windows, Android or Nextcloud (`\ : * ? " < > |`, plus a trailing dot or space)
  are percent-encoded in file names (`%3A`) and decoded in page names.
- Journal pages: `journals/YYYY/YYYY-MM-DD.md` ⇔ page name `YYYY-MM-DD`. Exactly **one file per day**;
  if two devices edit the same day while offline, the result is a normal sync conflict, handled by
  the conflict merge (phase 5).
- A namespace page (e.g. `work/projects`) may exist as `pages/work/projects.md` _alongside_
  the folder `pages/work/projects/`. If the file doesn't exist, the page is virtual.

## 3. Page file structure

```markdown
---
title: Project Alpha
aliases: [alpha]
tags: [project, client-x]
status: active
started: 2026-09-01
---

- Goal: ship the MVP by [[2026-12-01]] #milestone
  - Owner: [[people/anna]]
- TODO Write the spec ^k3x9qa
  scheduled:: 2026-10-05
  priority:: high
  - DONE Collect requirements
    done:: 2026-10-02T18:20
- Notes from [[meetings/2026-09-01 alpha kickoff]], see ((k3x9qa))
```

### 3.1 Frontmatter (page properties)

- Optional YAML block at the top, delimited by `---`.
- Reserved keys: `title`, `aliases`, `tags`, `template`, `icon` (an emoji shown next to the page everywhere; it wins over the
  `[[page_icons]]` rules of the config).
- All other keys are user properties (queryable).

> **Page properties from the UI:** the page header's _properties_ section edits this frontmatter (`set_page_property`,
> `remove_page_property`); a page without frontmatter gets one (plus a blank line) on the first property. Values may be YAML flow lists
> (`teams: [red, blue]`); each item is indexed separately, and the left panel can group pages by any of these keys.

### 3.2 Blocks

- The page body is a Markdown bullet list. Each `- ` list item is a block, and nesting is by
  indentation (**2 spaces** canonical; tabs and 4 spaces are accepted when reading).
- A block's content is the first line plus any continuation lines indented to the
  block's content column (multi-line paragraphs, code fences, tables).
- Text before the first bullet (after frontmatter) is a "preamble" block (kept, rendered
  as a normal block). This makes plain Markdown files written elsewhere still usable.
- **Text between bullets.** An unindented line that is not a bullet and follows a blank line ends the bullet above
  (as in CommonMark) and starts a _text block_ (`BlockKind::Text`): a heading, a table, a code fence, paragraphs,
  footnote definitions, as Nextcloud Text or Obsidian write them. It runs to the next unindented bullet; indented
  list items inside it belong to it. A text block is a top-level block without bullet, ID, properties or task
  marker; it is shown and edited as raw Markdown (Enter adds a line), kept byte for byte, never nested and nothing
  nests under it. An unindented line _without_ a blank line before it stays a lazy continuation of the bullet. The
  app puts a blank line between a text block and a bullet it moves or inserts next to it, so the two stay apart. Display only: `::: kind` ... `:::`, `<details>` ... `</details>` (each on its own line, outside code fences) and `[^1]` footnotes are drawn as containers by the renderer; the parser sees ordinary text.
- **Indentation when writing:** new files use 2 spaces per level. When editing an existing file,
  the app detects the file's indentation style (2 spaces, 4 spaces or tabs, from its existing
  nested blocks) and uses that style for new or moved lines, so a file never ends up with mixed styles
  because of the app.
- **Line endings:** the file's existing style (LF or CRLF) is kept on every edit. New files use LF.
  A UTF-8 BOM, if present, is kept.

### 3.3 Block properties

- Lines of the form `key:: value` at the start of a block's continuation lines
  (directly after the first line) are block properties.
- Keys: `[a-z0-9_-]+`. Values: parsed as date, datetime, number, boolean, link list
  (`[[a]], [[b]]`) or text.
- System properties (hidden in normal view): `done`, `completed`, plus any property
  listed in `config.toml` → `[properties] hidden`.

### 3.4 Block IDs

- Added **only when needed** (a block is referenced, embedded or synced to the calendar).
- Syntax: ` ^id` at the **end of the block's first line**, `id` = 6 characters `[a-z0-9]`
  (36⁶ ≈ 2.2 billion possible IDs), unique within the graph.
- **Generating:** random ID, checked against all IDs in the index; on a collision, draw again.
- **Duplicates** (e.g. a block copy-pasted with its ID in a text editor, or two devices
  generating IDs offline): the block already known to the index keeps the ID, and the other block
  gets a new ID (a guarded write to its file). On a cold index where neither is known, the block in the
  file that sorts first by path keeps it. Refs `((id))` therefore keep pointing to the original block.
  Every re-ID is reported in the health check.
- Hidden in rendered view.

### 3.5 Timestamps

- Blocks have **no** `created` or `modified` timestamps in the file, which keeps files clean and diffs small.
- The only automatic timestamps are `done::` (non-repeating tasks) and `completed::` (repeating tasks).
- File modification times are available in the index for "recently changed" views.

### 3.6 Task logbook

A block can keep dated progress notes in a **drawer**: continuation lines between `:LOGBOOK:` and `:END:`
(each on its own line, indented like the other continuation lines). Put it after the properties and
before the children:

```markdown
- DOING Write the spec
  deadline:: 2026-10-10
  :LOGBOOK:
  2026-10-04 14:30 Outline done
  2026-10-05 Reviewed with [[Anna]]
  :END:
  - child task
```

- An entry is `YYYY-MM-DD[ HH:MM] text`. A line without a leading date is still listed (without a date).
  Blank lines are ignored. Entries are single lines.
- The drawer is ordinary text for the parser: it is not a property, it round-trips byte for byte, and its text is
  searchable. The renderer hides it from the block text and the app lists it as a collapsible **Logbook (n)**.
- A `:LOGBOOK:` without a later `:END:` is plain text.
- Nothing is written automatically: entries are added by the user (`/log` in the editor adds a line with the
  current date and time, creating the drawer if needed). Logging does not change the task state.
- Links in entries count as references of the block (they show up in backlinks).

## 4. Inline syntax

| Syntax                                 | Meaning                                                                                                                                   |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `[[page]]`                             | Page link. **All links are global**: the full page name from the graph root, e.g. `[[work/projects/alpha]]`. There are no relative links. |
| `[[page\|label]]`                      | Page link with custom label                                                                                                               |
| `[[2026-10-03]]`                       | Journal link                                                                                                                              |
| `#tag` `#work/alpha` `#[[multi word]]` | Tag = page link                                                                                                                           |
| `((k3x9qa))`                           | Block reference (rendered inline)                                                                                                         |
| `!((k3x9qa))`                          | Block embed (rendered with children, editable)                                                                                            |
| `![[page]]`                            | Page embed                                                                                                                                |
| `==text==`                             | Highlight                                                                                                                                 |
| Standard GFM                           | Bold, italic, `code`, ~~strike~~, links, images, tables, task-less lists inside code                                                      |
| Raw HTML (`<b>`, `<script>`, …)        | **Not rendered.** Shown escaped, as literal text                                                                                          |

- **Emojis** are ordinary Unicode characters in the text. `:shortcode:` is only an input aid in the editor (`:rocket` suggests 🚀 and inserts the character); a shortcode is never stored, so any other tool reads the file as it is.

### Tag rules

- A tag starts with `#` at the start of a line or after whitespace or `(`, followed by one or more
  tag characters: Unicode letters (incl. `ąćęłńóśźż`), digits, `_`, `-`, `/`.
- Examples that are tags: `#123`, `#tag_name`, `#tagName`, `#tag-name`, `#work/alpha`, `#zażółć`.
- Not tags: `# Heading` (space after `#`), `a#b` (no whitespace before), `https://x.y/#frag`, anything in code.
- Trailing punctuation ends the tag: `#tag.` `#tag,` `#tag)` → tag `tag`. A trailing `-` or `/` is not part of the tag.
- Tags are case-insensitive like all page names (`#tagName` = `#tagname`), but `_` and `-` are
  significant: `#tag_name` and `#tag-name` are different pages.
- Multi-word tags: `#[[multi word]]`.

## 5. Tasks

- A block whose first line starts with a marker is a task:
  `TODO`, `LATER`, `DOING`, `NOW`, `WAITING`, `DONE`, `CANCELED`. `LATER` and `NOW` are Logseq's words for to do and in
  progress; they are states of their own (kept as written), and the app treats them like `TODO` and `DOING`: open
  tasks (agenda, page tasks), sort order, the cycle, repeating tasks (a repeating `LATER`/`NOW` task starts over as `LATER`).
  `task: todo` matches only `TODO`; use `task: todo, later`.
- A Markdown **checkbox** at the start of a bullet is a task too: `[ ]` is `TODO`, `[x]` and `[X]` are `DONE` (`[]`,
  `[ ]text` and a checkbox in a text block are not). The app keeps the style: ticking writes `[x]`, unticking `[ ]`,
  and no `done::` line is added, so other editors keep showing a plain checkbox. A state a checkbox cannot show
  (`DOING`, ...) is written as the word.
- Task properties:

| Property    | Format                                                                                                                                                           | Notes                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scheduled` | `YYYY-MM-DD`, `YYYY-MM-DD HH:MM` or `YYYY-MM-DD HH:MM-HH:MM`                                                                                                     | Start / planned date, with an optional start time and an optional end time on the same day (24 h; a hyphen or an en dash, spaces allowed when reading; the app writes `09:00-10:30`). The end must be after the start, otherwise the value is not a date. No time by default.                                                                                                            |
| `deadline`  | same                                                                                                                                                             | Due date; the same time forms (a point `17:00`, or a range for "between")                                                                                                                                                                                                                                                                                                                |
| `priority`  | one of the graph's labels (`[tasks] priorities`, `A`, `B`, `C` by default; one to nine labels); `high`, `medium` and `low` mean the first, middle and last label | Priority; shown after the marker as `[#A]`; none by default                                                                                                                                                                                                                                                                                                                              |
| `follows`   | `((abc123))`: a block reference                                                                                                                                  | The task this one is a follow-up of (written by `/followup`, or by hand). Tasks linked this way form a chain; a task may have several follow-ups (a branch). The referenced block needs an ID (`^abc123`); a reference to a block that does not exist is shown as a missing task in the chain. Query: `chain: ((abc123))` (whole chain), `prop.follows: ((abc123))` (direct follow-ups). |
| `repeat`    | `daily`, `weekly`, `monthly`, `yearly`, `every 2w`, `weekdays`, or a raw `RRULE:...`                                                                             | Maps to iCalendar RRULE                                                                                                                                                                                                                                                                                                                                                                  |
| `done`      | `YYYY-MM-DDTHH:MM`                                                                                                                                               | Set automatically on DONE (non-repeating tasks)                                                                                                                                                                                                                                                                                                                                          |
| `completed` | `YYYY-MM-DD, YYYY-MM-DD, …`                                                                                                                                      | Repeating tasks only: completion log, appended automatically each time the task is completed (hidden)                                                                                                                                                                                                                                                                                    |
| `cal`       | `no`                                                                                                                                                             | Opt a dated task out of calendar sync                                                                                                                                                                                                                                                                                                                                                    |

### Repeating tasks

When a task with `repeat::` is marked DONE, it is **not** left as DONE. Instead:

1. The completion date is appended to the hidden `completed::` property.
2. The task is reset to `TODO`, and `scheduled`/`deadline` move to the next occurrence.

```markdown
Before (marked DONE on 2026-10-03): After:

- TODO Water plants - TODO Water plants
  scheduled:: 2026-10-03 scheduled:: 2026-10-10
  repeat:: weekly repeat:: weekly
  completed:: 2026-09-26 completed:: 2026-09-26, 2026-10-03
```

## 6. Queries

Fenced code block with language `query`. One `key: value` clause per line, clauses ANDed.
Comma-separated values are ORed. `-key:` negates.

`ns: work` matches the page `work` **and every page below it** at any depth (`work/projects`,
`work/projects/alpha`), but never `workshop`. Use `page: work` to match only the single page.

`path: renewal` matches every page that has `renewal` **as a segment anywhere in its name** (`p/ey/renewal/apotheke`,
`x/renewal`), no matter what comes before or after; `path: ey/renewal` needs the two segments next to each other. Whole
segments only (`renewals` does not match), case-insensitive. `-path:` negates, commas are alternatives.

````markdown
```query
task: todo, doing
ns: work
tag: urgent
-tag: someday
deadline: < +7d
sort: deadline asc
view: kanban
group: task
```
````

Planned clauses (v0): `task`, `tag`, `links`, `ns`, `page`, `path`, `prop.<key>`, `scheduled`,
`deadline`, `priority` (`priority: A, B`, a label of the graph), `text`, `type` (block | page), `sort`, `limit`, `group`, `view`, `columns`, `direct`, `children`, `properties`, `inherit`.

**Inheritance.** In block queries `links:` and `tag:` also match every descendant of a block that has the link or tag (a link applies to the subtree): `- [[Client One]]: call` followed by an indented `- TODO prepare offer` finds the todo under `links: Client One`. `direct: yes` matches only blocks that carry the link themselves. Link-valued properties (`client:: [[X]]`) count as links. Scheduled/deadline dates, priority and other properties are **not** inherited by default. Page queries are unaffected.

**Templates.** `templates/<type>.md` (graph root, outside `pages/`, so not a page) holds the frontmatter skeleton and the starting blocks for pages with `type: <type>`; the file name is compared without regard to case. Only single-line frontmatter values are used; `type` itself is ignored. `{{title}}` is the last segment of the page name, `{{namespace}}` the part before it, `{{date}}` today (ISO), `{{cursor}}` is removed. Applying adds only keys the page does not have.

**Snippets.** `templates/snippets/<name>.md` holds blocks saved from a page (_Save as template…_): no frontmatter, no page type, the plain outline (`- text`, children indented). `/template` inserts them after the current block with the same variables. They are never applied to a page by its `type`; the subfolder keeps their names apart from the page types.

**Template for new pages.** `[graph] journal_template = "templates/journal.md"` and `[namespaces."people"] template = "templates/person.md"` (also `person.md` or `person`) name a template for new journal days and for new pages below a namespace (not the namespace page itself; longest match wins). It is applied when the file is first written: the template's `type` and missing properties, then its blocks, then the blocks of that edit. For a journal day `{{date}}` and `{{title}}` are the day.

**This page.** In a query block `{{page}}` is replaced by the link to the page the block sits on before the query is parsed (`prop.partner: {{page}}`, `page: {{page}}`, `links: {{page}}`; `[[{{page}}]]` is the same), `{{title}}` by its last name segment, `{{namespace}}` by the part before it and `{{date}}` by today (ISO). Nothing is written to the file. Without a page (agenda, saved queries) `{{page}}`, `{{title}}` and `{{namespace}}` are an error; the empty link `[[]]` is accepted as `{{page}}`.

**Page-valued properties.** A property whose value is a page is written with the link quoted: `partner: "[[part. Capgemini]]"`, several as `partner: ["[[A]]", "[[B]]"]`. The link counts as a reference of the page (backlinks) and `prop.partner: [[part. Capgemini]]` finds it (the same clause finds block properties `partner:: [[part. Capgemini]]`). Written by hand, `partner: [[part. Capgemini]]` and `partner: [[A]], [[B]]` are also read as links, although YAML itself would see a list in a list and an error. The app parses a copy in which such lines are quoted and never changes the file; it writes the quoted form itself.

**Sub-blocks of a result.** A block result shows only the block itself. `children: yes` shows it with all its sub-blocks (editable in place; in the `list` view and under the text of the `table` view); a sub-block that matches too is also listed on its own. Page queries are unaffected. In a **page** query (`type: page`) `children: yes` gives each row the page's own blocks, shown in the `text` column of a table (`columns: page, prop.when, text`) or under the page name in a list; without it a page row has only its name and properties. `properties:` limits the property lines of those blocks as well.

**Property lines of a result.** A block result shows the block's properties as a card under it. `properties: none` hides the card, `properties: status, owner` shows only those keys (`properties: all` is the default). It applies to the sub-blocks shown by `children: yes` too; table columns (`columns:`) and the filters are not affected, and the file is never changed.\n\n**Property inheritance.** `inherit: client, type` (a list of property keys) makes `prop.client:` and `prop.type:` also match every descendant of a block that has the property. The nearest block with the key decides (a child's own value wins over its parent's); for blocks with no value in their chain the page's frontmatter value counts. `direct: yes` turns it off. Task data (`scheduled`, `deadline`, `priority`, `repeat`) is never inherited this way; nothing is written to the files.

**Columns of a table.** `columns:` takes `text`, `page`, `task`, `scheduled`, `deadline`, `prop.<key>` and `linked-from`. A property written as `[[Page]]` is a link to that page in its cell (plain text stays text), and `linked-from` lists the pages that link to the row's page, the journal days that mention it among them.
Relative dates: `today`, `yesterday`, `+3d`, `-2w`, `+1m`, `this-week`. Weeks start on **Monday**
by default (`[graph] week_start` in config). Adding months clamps to the end of the month
(`+1m` from 31 January → 28/29 February).

An advanced boolean form (`where: (tag:a or tag:b) and not task:done`) is planned for later.

Other fenced languages are reserved for renderers (`mermaid`, `math`) and plugins.

## 7. Calendar mapping

- The calendar item UID is derived from the block ID: `notatnik-<graph-id>-<block-id>`.
  The mapping therefore lives in the files and works across devices with no extra state.
- Dated tasks get a block ID automatically when calendar sync is enabled.
- Calendar events (not tasks) are never written into Markdown. They live in the index cache only.

## 8. Config (`.notatnik/config.toml`) — sketch

```toml
favorites = ["Notatki", "P/Capgemini"]   # pages pinned at the top of the sidebar, in this order (at most 20); before the tables
hidden = ["Docs", "ref/old"]              # names hidden with everything below them: for names without a page file (a page with one uses `archived: true` in its front matter); before the tables

[graph]
id = "g7f3k2"                # random, generated once
journal_template = "templates/journal.md"
week_start = "monday"        # or "sunday"

[properties]
hidden = ["done", "completed"]
types = { rating = "number", started = "date", owner = "page" }

[namespaces."people"]          # applies to new pages under people/
template = "templates/person.md"

[tasks]
completed_log_limit = 0       # 0 = keep all completion dates; N = keep the last N
priorities = ["A", "B", "C"]  # one to nine labels, most important first; shown as [#A] after the marker; a task has none by default

[[page_icons]]               # pages with `type: person` get 👤 (key/value compared without regard to case)
key = "type"
value = "person"
icon = "👤"

[[saved_queries]]            # pinned to the sidebar
name = "Open renewals"
query = "task: todo\npath: Renewal"

[calendar]
tasks_as = "vtodo"           # or "vevent"
on_delete = "complete"       # or "delete"
```

## 9. Open points

None. Resolved on 2026-10-03:

- Block IDs: 6 characters `[a-z0-9]`, with collision and duplicate handling (§3.4).
- No `created` timestamps per block (§3.5).
- One journal file per day; concurrent edits go through the conflict merge (§2).
- Line endings and indentation: keep the file's existing style; new files use LF and 2 spaces (§3.2).
- Weeks start on Monday, configurable (§6).
- All links are global (§4); tag rules (§4); raw HTML escaped (§4); repeating tasks use `completed::` (§5); `ns:` includes everything below (§6).

Remaining before **v0 frozen**: your review of the whole spec (the edge-case section, §10, is written).

## 10. Edge cases

The parser never fails: any input gives a page plus diagnostics. Each rule below has a fixture in
`fixtures/graphs/edge-cases/pages/` and a test in `crates/core/tests/parse_fixtures.rs`.

| Case                                                                                             | Behaviour                                                                                                                                                                                          | Fixture                              |
| ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| CRLF                                                                                             | Detected from the first line break; kept on every edit. Spans exclude the `\r\n`.                                                                                                                  | `crlf.md`                            |
| UTF-8 BOM                                                                                        | Skipped when parsing (frontmatter may follow it), kept on edits.                                                                                                                                   | `bom.md`                             |
| No trailing newline                                                                              | Kept: appends add a line break before the new line and none after it; deleting the last block removes the preceding break.                                                                         | `no-trailing-newline.md`             |
| Empty file                                                                                       | A page with no blocks, no diagnostics.                                                                                                                                                             | `empty.md`                           |
| Only frontmatter                                                                                 | Frontmatter, no blocks.                                                                                                                                                                            | `only-frontmatter.md`                |
| Tabs                                                                                             | A tab counts as width 4 (next multiple of 4). Nesting is decided by comparing widths with a stack, so any consistent style works.                                                                  | `tabs.md`, `four-space.md`           |
| Mixed indentation                                                                                | Same stack rule: a line is a child of the nearest earlier bullet with a smaller width.                                                                                                             | `mixed-indent.md`                    |
| Indent jumps                                                                                     | A child indented by 6 under a parent at 0 is simply a child; levels are by tree depth, not by width.                                                                                               | `indent-jump.md`                     |
| Nesting deeper than 100                                                                          | Deeper bullets become siblings at level 100, with a `too-deep` warning.                                                                                                                            | (generated in tests)                 |
| `*` and `+` bullets                                                                              | Accepted as bullets; kept as written; new lines use `-`.                                                                                                                                           | `bullets.md`                         |
| Numbered lists (`1. x`)                                                                          | Not blocks: plain continuation text of the block above.                                                                                                                                            | `bullets.md`                         |
| Empty bullet `-`                                                                                 | A block with empty text. It may have properties.                                                                                                                                                   | `bullets.md`                         |
| Code fences (` ``` ` and `~~~`)                                                                  | Everything inside is opaque: no bullets, properties, links, tags or task markers. A fence closes on a line with the same character and at least the same length.                                   | `fences.md`                          |
| Fence opener inside text on the first line (` ```x``` `)                                         | Not a fence (the info string of a backtick fence can't contain backticks).                                                                                                                         | `fences.md`                          |
| Unclosed fence                                                                                   | Ends at the next bullet at the same or a lower indentation than its block, or at the end of the file. `unclosed-fence` warning.                                                                    | `fences.md`                          |
| Invalid YAML frontmatter                                                                         | The page loads; the frontmatter span is preserved; nothing is extracted from it; `invalid-frontmatter` error pointing at the offending line. Structural edits work; frontmatter edits are refused. | `bad-frontmatter.md`                 |
| Frontmatter never closed                                                                         | Not frontmatter: the `---` is ordinary preamble text.                                                                                                                                              | `unclosed-frontmatter.md`            |
| `---` later in the file                                                                          | Ordinary text (a horizontal rule). Frontmatter is only recognised at byte 0 (after a BOM).                                                                                                         | `hr-later.md`                        |
| Text before the first bullet                                                                     | The preamble block (headings, paragraphs, fences).                                                                                                                                                 | `text-between.md`                    |
| Non-bullet lines right after a bullet (including `# Heading` at column 0), no blank line between | Lazy continuation: part of the block above.                                                                                                                                                        | `text-between.md`                    |
| Unindented text after a blank line, between bullets                                              | A text block of its own (see 3.2); the bullets after it start at the top level.                                                                                                                    | `nextcloud-text.md`                  |
| `- [ ] x`, `* [x] x`                                                                             | A task (TODO / DONE) written as a checkbox.                                                                                                                                                        | `nextcloud-text.md`                  |
| `- ## Heading`                                                                                   | An ordinary block whose text is a heading.                                                                                                                                                         | `text-between.md`                    |
| Very long lines                                                                                  | No limit; linear time.                                                                                                                                                                             | (generated in tests)                 |
| `[[` without `]]`                                                                                | Plain text.                                                                                                                                                                                        | `brackets.md`                        |
| Nested `[[a [[b]]]]`                                                                             | No nesting: the inner link `b` wins; the outer brackets are text.                                                                                                                                  | `brackets.md`                        |
| Escapes `\[[x]]`, `\#tag`                                                                        | Not links or tags. A backslash escapes any ASCII punctuation.                                                                                                                                      | `brackets.md`                        |
| `[[]]`, `[[./x]]`, `[[../y]]`                                                                    | Not links: `empty-link` / `relative-link` warnings.                                                                                                                                                | `brackets.md`                        |
| `#` in URLs, `[text](#anchor)`, inline code, `a#b`, `# Heading`                                  | Not tags. Markdown link destinations and inline code spans are skipped.                                                                                                                            | `hash-cases.md`                      |
| `#tag.` `(#tag)` `#tag-`                                                                         | Tag `tag` (trailing punctuation, `-` and `/` are not part of the tag).                                                                                                                             | `hash-cases.md`                      |
| Raw HTML                                                                                         | Plain text; links and tags inside it still work.                                                                                                                                                   | `html.md`                            |
| `^id` not at the end, wrong length, uppercase                                                    | Not an ID.                                                                                                                                                                                         | `block-ids.md`                       |
| Duplicate ID in one file                                                                         | Both parse; a `duplicate-block-id` warning on the second. Cross-file duplicates are handled by the index (§3.4).                                                                                   | `block-ids.md`                       |
| `key:: value` after content, in a fence, or after a blank line                                   | Not a property (content).                                                                                                                                                                          | tests in `parse_features.rs`         |
| `std::fmt` (`::` without a following space)                                                      | Not a property.                                                                                                                                                                                    | `props.rs` tests                     |
| Invalid `scheduled`/`deadline` date, unknown `repeat`, bad `priority`                            | Warning diagnostic; the task still parses with that field empty.                                                                                                                                   | `tasks/pages/dates.md`, `repeats.md` |

Parsing details worth knowing:

- Inline code spans are matched per line (a span never crosses a line break).
- A block's own lines end at its last non-blank line; blank lines between blocks belong to no block.
- Property keys are `[a-z0-9_-]+`; `Status:: x` is not a property.
- A block ID may be the whole first line (`- ^abc123`).

## 11. Encryption (EN-2, EN-3, EN-5)

Specified in [encryption.md](encryption.md); this section is the byte layout. All numbers are little endian. All
keys are 32 bytes; derivations use BLAKE3 `derive_key` with a context string.

**Vault descriptor** (JSON, public): `version` (1), `key_id` (hex, 16 bytes: `derive_key("notatnik 2026 key id",
data_key)[..16]`), `public_key` (hex, the X25519 public key), `kdf` (`m_kib`, `t`, `p`: Argon2id, version 0x13,
32 byte output; limits 8 KiB to 1 GiB, 1 to 64 passes, 1 to 16 lanes), `slots`: `kind` (`password` or
`recovery`), `salt` (hex 16 bytes, password only), `nonce` (hex 24 bytes), `wrapped` (hex). A slot wraps the key
bundle `01 | data_key | x25519_secret` (65 bytes) with XChaCha20-Poly1305; the associated data is
`"notatnik slot v1" | key_id | public_key | kind` (so a slot cannot be moved into another descriptor). The password
slot's key is Argon2id over the NFC-normalised UTF-8 password and the salt. The recovery slot's key is
`derive_key("notatnik recovery kek v1", recovery_key)`.

**Recovery key text:** the 32 bytes plus a 3 byte check (`derive_key("notatnik recovery check v1", key)[..3]`) as
56 characters of base32 (`A-Z2-7`, no padding) in 14 groups of four separated by `-`; reading ignores dashes,
spaces and case.

**Encrypted file** (`NTNKFILE`): `"NTNKFILE"` | version `01` | key_id (16 raw bytes) | stream nonce (19) | chunks.
The key is `derive_key("notatnik file v1", data_key)`. Chunks: XChaCha20-Poly1305 STREAM-BE32, 64 KiB of plaintext
each plus a 16 byte tag, the last one marked as last (an empty file is one empty last chunk). The associated data
of every chunk is the header followed by the caller's context (the path). The file name for an encrypted copy is
`hex(keyed_blake3(derive_key("notatnik name v1", data_key), path)[..16])`.

**Sealed file** (`NTNKSEAL`, used for backups): `"NTNKSEAL"` | version `01` | descriptor length (u32) | descriptor
(the vault JSON) | comment length (u16) | comment | ephemeral public key (32) | wrap nonce (24) | wrapped file key
(48) | stream nonce (19) | chunks. The wrap key is `blake3 derive_key("notatnik seal v1")` over the X25519 shared
secret, the ephemeral and the recipient public key; the file key is random and wrapped with XChaCha20-Poly1305
with everything up to the ephemeral key as associated data. The chunks (as above) are bound to the BLAKE3 hash of
the whole header. The comment holds `notatnik-backup v1 <fingerprint>` for a snapshot (BK-1). A sealed snapshot is
`notatnik-<graph id>-<YYYYMMDD>-<HHMMSS>[-before].zip.enc`; decrypting it gives the zip of BK-1.

**Encrypted copy on a WebDAV server** (EN-11 to EN-13): a path listed under `[encryption] paths` in
`.notatnik/config.toml` is stored as `<path>.enc`, an `NTNKFILE` whose context is `<path>` (without `.enc`). The
key is the data key of the graph's vault, `.notatnik/vault.json` (the vault JSON of EN-2, plain, synced). Folder
entries end in `/`; matching is on the path relative to the graph folder, and a conflict copy is matched by the
path of the file it came from. Local files are never encrypted. With hidden names, a marked file in a snapshot is `.notatnik-enc/<aa>/<random>.enc` (an `NTNKSEAL` file with the comment `notatnik-entry v2 hidden` and the same `path length | path | content` inside); `<random>` is 32 random hex characters, unrelated to the path.

**Scope** (EN-27): `[encryption] scope = "marked"` (default) uses `paths` as above; `scope = "all"` stores every file
encrypted except those named by `except` (entries like `paths`), and `.notatnik/` and `.notatnik-enc/` never. A
conflict copy follows its original in both. The unused list is kept in the file.

**Hidden object** (EN-25): with `[encryption] names = "hidden"` a marked file is stored on the server at
`.notatnik-enc/<aa>/<name>.enc`, `<name>` being `Keys::file_name(path)` (32 hex characters) and `<aa>` its first two.
The object is an `NTNKFILE` whose plaintext is `path length (u16) | path (UTF-8) | content` and whose associated
data is `"notatnik hidden v1"`. The reader checks that the name is the one `file_name(path)` gives. A name that does
not have this shape inside `.notatnik-enc/` is not ours and is ignored. Locally the folder `.notatnik-enc/` is never
synced.

**Marked files in a snapshot** (EN-23): in a zip that is not sealed as a whole, a path listed under `[encryption]
paths` is the entry `<path>.enc` (stored, not deflated), an `NTNKSEAL` file for the vault `.notatnik/vault.json`
whose comment is `notatnik-entry v1 <path>`. The entry `<path>` itself is absent. An entry is taken for sealed
only when it carries that comment, so a user's own file called `x.enc` is left alone.

Fixtures made by an earlier build live in `fixtures/crypt/` and must keep opening.
