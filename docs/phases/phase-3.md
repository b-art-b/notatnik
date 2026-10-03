# Phase 3: Tasks, queries and views

**Goal:** the app becomes a task manager and a lightweight database. Tasks with dates and an agenda,
live query blocks, and list / table / kanban / timeline views, all backed by plain Markdown.

**Requirements covered:** TK-1–6, QV-1–8, QV-10, PR-3.
**Depends on:** phase 2 (editor, IPC); phase 1 query engine (P1.16–P1.29).

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh` (and `--full`) pass.
2. Live query results update within 200 ms of an edit (measured).
3. The manual acceptance checklist passes; you plan a full week using the agenda and at least one kanban board.

## Milestones

| | Milestone | Tasks |
|---|---|---|
| 3A | Tasks | P3.1–P3.5 |
| 3B | Live query blocks | P3.6–P3.9 |
| 3C | Views | P3.10–P3.14 |
| 3D | Repeats, progress, property UX | P3.15–P3.17 |

## Key design decisions for this phase

| Topic | Decision |
|---|---|
| Live updates | Each visible query is registered with a *dependency signature* (tags, namespaces, property keys, task-ness, date fields it reads). On `graph-changed`, the backend computes which signatures may be affected by the changed blocks and only those queries re-run. Fallback: re-run all visible queries (correct but slower). |
| Views | A view is a Svelte component implementing `View { render(rows, columns, groups, options), onEdit?(op) }`. Built-in views use the same interface plugins will use (QV-9, phase 6). |
| Edits from views | Every edit in a view (kanban drag, table cell edit, checkbox) becomes a `BlockOp` (`SetProperty`, `SetTaskState`), going through the same guarded write path as the editor. |
| Repeat semantics | Defined in P3.15 with an explicit test table. Default = "next future occurrence" (like org-mode `++`). |
| Dates | Dates without a time are "floating" (no timezone). Datetimes are local time. CalDAV conversion is handled in phase 4. |

---

## 3A: Tasks

### P3.1 Task state cycling (TK-1)
- [ ] Ctrl+Enter cycles `(none) → TODO → DOING → DONE → (none)`; clicking the marker in rendered mode cycles too; `WAITING`/`CANCELED` set via the slash menu or right-click
- [ ] Implemented as the `SetTaskState` op (P1.14), not as text manipulation in the UI

**Verify**
- [ ] Playwright: each transition shows the correct marker and styling (DONE struck through, CANCELED dimmed)
- [ ] e2e: after a cycle, exactly the first line of that block changed on disk

### P3.2 Done timestamp (TK-5)
- [ ] Moving a non-repeating task to DONE adds `done:: <datetime>` (hidden property); moving away from DONE removes it. Repeating tasks use `completed::` instead (P3.15)

**Verify**
- [ ] Golden edit tests: TODO→DONE adds the property line; DONE→TODO removes it; other properties untouched and in the same order
- [ ] The `done` property is not shown in the rendered view (PR-4) but is visible in raw edit mode

### P3.3 Task properties UI (TK-2)
- [ ] Inline chips for scheduled / deadline / priority / repeat in the rendered view; clicking a chip opens a picker (date picker with natural language input from P2.24, priority dropdown)
- [ ] Slash commands: `/scheduled`, `/deadline`, `/priority`

**Verify**
- [ ] Playwright: setting a deadline via the chip → `deadline:: 2026-10-10` in the file; clearing it removes the line
- [ ] An overdue deadline chip is styled as overdue (fixed "today" in the test)

### P3.4 Agenda view (TK-4)
- [ ] Sections: Overdue, Today, Next 7 days (grouped by day), No date; each entry is an editable block with its page breadcrumb
- [ ] Filters: namespace, tag, priority; saved per device
- [ ] Implemented with the query engine (an agenda is a set of predefined queries)

**Verify**
- [ ] On fixture `tasks` with a fixed today: each section contains exactly the expected tasks (Playwright compares to an expected JSON)
- [ ] Completing a task in the agenda → it disappears from Today (or moves to the completed list) and the file changes
- [ ] Filter `ns: work` → only tasks from `work` and pages below it

### P3.5 Journal integration
- [ ] Today's journal shows a "Scheduled / due today" section above its blocks (read from the agenda queries; not written into the file)

**Verify**
- [ ] A task anywhere with `scheduled:: <today>` appears in today's journal section; the journal file does not contain it

## 3B: Live query blocks

### P3.6 Query block renderer (QV-1)
- [ ] A ```` ```query ```` fenced block renders as results instead of code; the raw text is shown in edit mode
- [ ] Header shows the result count, a "show query" toggle and the view switcher

**Verify**
- [ ] Playwright (mock): each query from `fixtures/graphs/queries/queries/*.query` embedded in a page renders the expected number of results

### P3.7 Live updates
- [ ] Register visible queries with their dependency signature; re-run affected queries on `graph-changed`; unregister when scrolled away or closed

**Verify**
- [ ] Test (Rust): a change to a block tagged `#urgent` invalidates `tag: urgent` but not `tag: home`
- [ ] Playwright: adding `#urgent` to a block on another page (via the mock API) → the query on the open page updates without reload
- [ ] Measured: edit → updated results < 200 ms on the synthetic graph with 5 visible queries

### P3.8 Inline errors (QV-10)
- [ ] Parse errors are shown under the query block with line/column and the suggestion from P1.16; the page keeps working

**Verify**
- [ ] A query with `tags: x` renders "unknown key `tags`; did you mean `tag`?" and the rest of the page is still editable

### P3.9 Query editing help
- [ ] In edit mode, autocomplete of clause keys, tag names, namespaces and property keys inside a query block (using the WASM query parser)

**Verify**
- [ ] Typing `ta` at the start of a line in a query block suggests `task:` and `tag:`

## 3C: Views

### P3.10 List view (QV-4)
- [ ] Results as blocks with a breadcrumb; grouped by page by default; group headers collapsible

**Verify**
- [ ] `group: page` and `group: task` render the expected group headers and order (Playwright vs the expected JSON)

### P3.11 Table view (QV-4)
- [ ] Columns = `columns:` clause, or inferred from properties present in the results; sortable by clicking headers; cell editing writes `SetProperty`

**Verify**
- [ ] Editing a cell → only that property line changes in the file (e2e)
- [ ] Sorting by a date column handles empty values (always last)
- [ ] 500 rows render and scroll smoothly (no frame drops in the inspector)

### P3.12 Kanban view (QV-5)
- [ ] Columns from `group:` (task state, or a property with known values, from config or observed values); drag a card → `SetTaskState` or `SetProperty`
- [ ] Order within a column follows `sort:`; manual ordering is out of scope (it would need a stored order property)

**Verify**
- [ ] Drag a card from TODO to DOING → the file shows `DOING` on that block, and nothing else changed
- [ ] Drag with `group: status` (a property) → the `status::` line changes
- [ ] Keyboard alternative: a focused card can be moved with a shortcut (accessibility)

### P3.13 Timeline / calendar view (QV-6)
- [ ] Month and week views placing results on `scheduled`/`deadline` (or a chosen date property); drag to another day → updates the property

**Verify**
- [ ] On fixture `tasks`, the October 2026 month view shows each task on the expected day
- [ ] Drag to another day → the property changes by exactly the dragged number of days

### P3.14 Editable results and saved queries (QV-7, QV-8)
- [ ] The list view embeds real editable blocks (same editor component as pages)
- [ ] "Save to sidebar" pins a query (stored in `.notatnik/config.toml` so it syncs)

**Verify**
- [ ] Editing a block inside a query result writes to the source page's file
- [ ] A saved query appears in the sidebar on a second device after Nextcloud syncs `config.toml`

## 3D: Repeats, progress, property UX

### P3.15 Repeating tasks (TK-3)
- [ ] On DONE of a task with `repeat::` (decided 2026-10-03): append the completion date to the hidden `completed::` property, then reset to TODO with the dates moved forward. If `[tasks] completed_log_limit` is N > 0, keep only the last N dates
- [ ] Undo restores the previous state exactly (DONE is reverted, the date is removed from `completed::`, the dates move back)
- [ ] Semantics (default "next future occurrence", configurable per task with `repeat:: weekly from-completion`):

| repeat | scheduled before | completed on | scheduled after |
|---|---|---|---|
| `daily` | 2026-10-01 | 2026-10-03 | 2026-10-04 |
| `weekly` | 2026-09-28 (Mon) | 2026-10-03 | 2026-10-05 (Mon) |
| `monthly` | 2026-01-31 | 2026-02-02 | 2026-02-28 (clamped) |
| `weekdays` | 2026-10-02 (Fri) | 2026-10-02 | 2026-10-05 (Mon) |
| `every 2w` | 2026-09-21 | 2026-10-03 | 2026-10-05 |
| `weekly from-completion` | 2026-09-28 | 2026-10-03 | 2026-10-10 |

**Verify**
- [ ] The table above (extended to ≥ 15 rows, including deadlines with a time, and tasks with both scheduled and deadline keeping their offset) passes as a unit test
- [ ] e2e: completing a weekly task → the file shows TODO, the new date, and today's date appended to `completed::`; no other lines changed
- [ ] `completed::` is hidden in the rendered view and visible in raw edit mode
- [ ] With `completed_log_limit = 3` and 4 completions → only the last 3 dates remain
- [ ] A query can use it: `prop.completed: contains 2026-10-03` finds the task

### P3.16 Subtask progress (TK-6)
- [ ] A parent block with task children shows `2/5` and a small progress bar (computed, not stored)

**Verify**
- [ ] Completing a child updates the parent's progress immediately; nested grandchildren count (decide: direct children only or all descendants; document it)

### P3.17 Property autocomplete (PR-3)
- [ ] Typing `key::` suggests existing keys; after `:: ` suggests values seen for that key (from the index)

**Verify**
- [ ] On fixture `queries`, typing `sta` at the start of a property line suggests `status::`; after `status:: ` it suggests `active`, `done`, `paused`

---

## Manual acceptance checklist

- [ ] Plan next week: create 15 tasks across namespaces with dates and priorities; the agenda shows them correctly
- [ ] Build a kanban board for one project with a query; move cards around over a few days
- [ ] A weekly repeating task survives 2 completions with the correct dates
- [ ] Create a table of 20 pages with properties (e.g. `type:: book`, `rating::`); sort it and edit cells
- [ ] All edits look sensible when the files are opened in a text editor

Record: date / commit / notes:
