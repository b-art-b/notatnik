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
│   └── 2026/10/diagram-a1b2.png
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
~/.config/notatnik/settings.toml # device settings, list of graphs
```

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
- A namespace page (e.g. `work/projects`) may exist as `pages/work/projects.md` *alongside*
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
- Reserved keys: `title`, `aliases`, `tags`, `template`, `icon`.
- All other keys are user properties (queryable).

### 3.2 Blocks

- The page body is a Markdown bullet list. Each `- ` list item is a block, and nesting is by
  indentation (**2 spaces** canonical; tabs and 4 spaces are accepted when reading).
- A block's content is the first line plus any continuation lines indented to the
  block's content column (multi-line paragraphs, code fences, tables).
- Text before the first bullet (after frontmatter) is a "preamble" block (kept, rendered
  as a normal block). This makes plain Markdown files written elsewhere still usable.
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

## 4. Inline syntax

| Syntax | Meaning |
|---|---|
| `[[page]]` | Page link. **All links are global**: the full page name from the graph root, e.g. `[[work/projects/alpha]]`. There are no relative links. |
| `[[page\|label]]` | Page link with custom label |
| `[[2026-10-03]]` | Journal link |
| `#tag` `#work/alpha` `#[[multi word]]` | Tag = page link |
| `((k3x9qa))` | Block reference (rendered inline) |
| `!((k3x9qa))` | Block embed (rendered with children, editable) |
| `![[page]]` | Page embed |
| `==text==` | Highlight |
| Standard GFM | Bold, italic, `code`, ~~strike~~, links, images, tables, task-less lists inside code |
| Raw HTML (`<b>`, `<script>`, …) | **Not rendered.** Shown escaped, as literal text |

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
  `TODO`, `DOING`, `WAITING`, `DONE`, `CANCELED`.
- Task properties:

| Property | Format | Notes |
|---|---|---|
| `scheduled` | `YYYY-MM-DD` or `YYYY-MM-DD HH:MM` | Start / planned date |
| `deadline` | same | Due date |
| `priority` | `high` \| `medium` \| `low` | |
| `repeat` | `daily`, `weekly`, `monthly`, `yearly`, `every 2w`, `weekdays`, or a raw `RRULE:...` | Maps to iCalendar RRULE |
| `done` | `YYYY-MM-DDTHH:MM` | Set automatically on DONE (non-repeating tasks) |
| `completed` | `YYYY-MM-DD, YYYY-MM-DD, …` | Repeating tasks only: completion log, appended automatically each time the task is completed (hidden) |
| `cal` | `no` | Opt a dated task out of calendar sync |

### Repeating tasks

When a task with `repeat::` is marked DONE, it is **not** left as DONE. Instead:

1. The completion date is appended to the hidden `completed::` property.
2. The task is reset to `TODO`, and `scheduled`/`deadline` move to the next occurrence.

```markdown
Before (marked DONE on 2026-10-03):       After:
- TODO Water plants                        - TODO Water plants
  scheduled:: 2026-10-03                     scheduled:: 2026-10-10
  repeat:: weekly                            repeat:: weekly
  completed:: 2026-09-26                     completed:: 2026-09-26, 2026-10-03
```

## 6. Queries

Fenced code block with language `query`. One `key: value` clause per line, clauses ANDed.
Comma-separated values are ORed. `-key:` negates.

`ns: work` matches the page `work` **and every page below it** at any depth (`work/projects`,
`work/projects/alpha`), but never `workshop`. Use `page: work` to match only the single page.

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

Planned clauses (v0): `task`, `tag`, `links`, `ns`, `page`, `prop.<key>`, `scheduled`,
`deadline`, `text`, `type` (block | page), `sort`, `limit`, `group`, `view`, `columns`.
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

Remaining before **v0 frozen**: the edge-case section (phase 1, P1.2) and your review of the whole spec.
