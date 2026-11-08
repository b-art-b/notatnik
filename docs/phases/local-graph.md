# Local graph and pages with no links (LG1 to LG3)

**Goal:** see the **neighborhood of the page you are on**, and find the pages that are **not connected to anything**.
No whole-graph picture: it is unreadable beyond a few hundred pages and answers no question.

- **Local graph:** a tab _Graph_ in the right side panel (its own header has the ×): the current page in the middle, the pages it links to and the
  pages that link to it around it (one step, or two), each a link that opens the page.
- **Pages with no links:** a filter of _All pages_ (and a palette command): the pages that no page links to and that
  link to no page, so islands can be linked or deleted.

**Requirements covered:** LN-9 and LN-13 (see `requirements.md`).
**Depends on:** the index (`refs`, `aliases`, `pages`), the right side panel (`SidePanel.svelte`, tabs), the All pages
view (`AllPagesView.svelte`), the command table.
**Status:** LG1 to LG3 done and tested in Chromium; WebKit and the desktop app are open.

## Decisions

- **What is a link:** a page link, a tag or a page embed (the kinds `backlinks` already counts), also through an alias.
  A page linking to itself is no link. Block references `((id))` are not page links and are not drawn.
- **The local graph** shows the page, its neighbours at distance 1 (links out and links in) and, on request, at distance
  2, and **the links between all of those pages** (so a cluster looks like one). At most **60 pages**: when there are
  more, the nearest are kept (distance 1 before 2, then by name) and the panel says so.
- **Drawn as SVG** with a fixed layout (no physics): the page in the middle, distance 1 on a ring, distance 2 on an outer
  ring beside the page that brought it in. The same page always looks the same. Arrows show the direction. Every node is
  a link (focusable, Enter opens it), with the full name as its title; names are shortened in the picture. Below the
  picture the same pages are listed as text (_Links to_, _Linked from_), which is also what a screen reader uses.
- **Not a tab you always have:** it appears when you ask for it (_Local graph_ in the palette), like the Pages tab; its ×
  closes it. It follows the page you are on. A namespace field limits the pages shown to one namespace (the page itself
  stays).
- **Pages with no links** is a view of _All pages_, so it has what that view has (filter by name, sort, hidden and archived
  pages left out unless asked for, the padlocks). Journal days are left out. A page that exists only because others link
  to it has a link, so it is never listed.
- **Nothing is written** to the graph folder or to settings by either feature.

## Tasks

### LG1 The backend

- `Index::link_graph(name, depth, max)` and an `outgoing` count on `PageRow` (alias-aware); API `local_graph {page,
depth}` and `outgoing` on `PageListRow`; bindings.
- **Verify:**
  - [x] a page with two links out and one in: depth 1 has the three pages and the edges; the page itself has depth 0
  - [x] depth 2 adds the pages one step further and the links between all of them; a link between two neighbours is an edge
  - [x] a link through an alias, a tag and a page embed count; a self link and a block reference do not
  - [x] more than 60 pages: the cap keeps depth 1 before depth 2 and says `truncated`
  - [x] a page that does not exist (a name only) answers with just itself
  - [x] `all_pages` has `outgoing`; a page linked only through an alias has a backlink

### LG2 The panel and the list

- `Graph` tab in `SidePanel`, the layout function, the SVG, the text list, the depth and namespace controls; the _No
  links_ filter in _All pages_ and the route `#/pages?links=none`; the palette commands _Local graph_ and _Pages with no
  links_.
- **Verify:**
  - [x] _Local graph_ opens the panel on the Graph tab for the current page; the middle node is the page; a neighbour opens its
        page (mouse and Enter on the focused node)
  - [x] the graph follows the page: navigating to a neighbour redraws it around that page
  - [x] 1 step / 2 steps; the namespace field leaves out the pages outside it but not the page itself
  - [x] the text list has the same pages as the picture; a long name is shortened in the picture and whole in the title
  - [x] a page with no links shows only itself and says it has no links
  - [x] × closes the graph (a button in the graph's own header, since a single tab has no tab bar); the other tabs keep working
  - [x] the layout is the same twice for the same graph (unit test), and no two nodes of one ring overlap
  - [x] _Pages with no links_ lists the unlinked pages only, not journals, not archived pages, not pages that are linked to;
        linking one makes it leave the list

### LG3 Tests and documents

- Rust tests (`crates/index`, `crates/app`), Vitest for the layout, Playwright (Chromium and WebKit); `keymap.md`,
  `requirements.md`, `testing.md`, `TODO.md`, `CLAUDE.md` (the doc list).
- **Verify:**
  - [x] `scripts/check.sh` passes
  - [ ] `scripts/check-webkit.sh` passes
  - [ ] by hand, in the desktop app: the panel at 360, 600 and 900 px, and with a graph of a few hundred pages
