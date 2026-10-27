# Hide pages (H1 to H4)

**Goal:** one concept, _Hide_, for taking pages out of the way: a page, a folder of pages (also one that has no page file of
its own), or many at once. Hidden pages are not in the sidebar, Recent, the property groups and the agenda; All pages
shows them on request, can show only them, and takes them back. This replaces _Archive_ (same behavior, new name,
plus the folder and bulk cases).

**Requirements covered:** NS-20 (renamed), NS-25, NS-27.
**Status:** built and tested.

## Design

- **The mark.** A page with a file is hidden by `archived: true` in its front matter (the key keeps its name so existing graphs
  and queries such as `-prop.archived: true` keep working; the app calls it _hidden_). A name that has no file (a folder like `work/` that exists
  because `work/x` does) is hidden by an entry in `hidden = [...]` of `.notatnik/config.toml` (a plain value before the tables,
  like `favorites`; it syncs). Either mark hides the page and everything below it.
- **One command** `hide_pages { pages, hidden }` marks or unmarks many names: a name with a file gets or loses the front matter
  line (a minimal edit), a name without one goes into or out of the list. Names below another name of the same request are dropped
  (the one above covers them). Unhiding a page that is hidden only by a folder above it cannot be done for that page alone: the
  answer names the folder.
- **Follows.** A rename moves entries of `hidden` (and the entries below a renamed folder); deleting a page removes its entry.
- **The agenda and the lists** read the union of both marks (`archived_pages` returns both).
- **All pages.** _Hidden pages_ is a three-way filter: _not shown_ (default), _shown too_ (muted, with a _hidden_ badge), _only hidden_.
  In the list every row has a checkbox, the header one selects all shown rows, and a bar offers **Hide** / **Unhide** for the
  selection with Undo. The Namespaces view has the same filter (_only hidden_ keeps the names above hidden pages) and the
  per-row button.
- **The page.** _Hide_ / _Unhide_ under the title (also for a folder without a file), and a banner _Hidden: not in the sidebar and
  the agenda. Unhide_ (or _…because_ `<folder>` _is hidden_).

## Tasks

### H1 Backend (NS-27)

- [x] `hidden` in the config (known key, before the tables); `archived_pages` and the agenda read both marks
- [x] `hide_pages` (front matter or list, subtree dedupe, inherited marks reported); renames and deletes keep the list right

**Verify**

- [x] Config round trip (list before the tables, not written when empty); app test: a page, a folder without a file, many at once, the subtree rule, unhide, an inherited mark, rename, delete, the agenda

### H2 UI: page and lists (NS-20)

- [x] _Hide_ / _Unhide_ wording everywhere; button, banner, row buttons and toasts use `hide_pages`; the folder case works from the page

**Verify**

- [x] Browser: the existing archive tests renamed to hide pass; a folder without a file hides and unhides from its page

### H3 All pages (NS-25, NS-27)

- [x] The _Hidden pages_ filter (three states) in List and Namespaces; muted rows with a badge; row checkboxes and the bar

**Verify**

- [x] Browser: the three states, only hidden in the tree keeps the names above, select rows and hide, unhide, Undo, select all

### H4 Docs

- [x] requirements, keymap, file-format (`hidden`), testing
