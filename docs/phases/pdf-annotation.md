# Annotating PDFs (A1 to A7)

**Goal:** read a PDF in the app, highlight text or an area, and write notes on the highlights, as in Logseq, but
with the highlights kept as ordinary blocks of the graph: they sync, are found by search, can be tagged, queried and
referenced from any page with `((id))`, and a reference jumps back to the spot in the PDF. The PDF itself is never
changed.

**Requirements covered:** PA-1 to PA-10 (see `requirements.md`).
**Depends on:** assets (ST-8), block IDs and references (3.4), page properties (3.1). Ink on the PDF (the drawing editor
as a layer over a page) is option B of the proposal and is **not** in this phase; the format leaves room for it
(see "Later").
**Status:** A1 done (Rust, tested). A2 to A7 in progress.

## Design

- **Viewer: [pdf.js](https://github.com/mozilla/pdf.js)** (`pdfjs-dist`, Apache-2.0). It renders a page to a canvas and
  gives a transparent text layer on top, which is what makes selecting text possible. It runs in WebKit (the engine of
  the Linux app and of the iPad), is loaded only when a PDF is opened (dynamic `import()`, its worker as a separate
  Vite asset), and pages are rendered lazily, only the ones near the viewport, so a PDF of hundreds of pages opens at
  once.
- **The PDF stays untouched** (ST-3, and no conflict copies of a big binary). It lives in the graph as an ordinary
  asset, `assets/…/report.pdf`, put there by `/file` → This computer → Copy (LN-12), by dropping it on a page, or by
  hand, and is linked as `[Report](../assets/report.pdf)`. A click on a link to a `.pdf` **inside the graph** opens the
  viewer; a PDF elsewhere (a file address, a web address) still opens with the system's program.
- **The annotations are a page, the sidecar**, `hls/<name>` (file `pages/hls/<name>.md`, the same idea and prefix as
  Logseq's `hls__<name>.md`; a namespace instead of a double underscore). `<name>` is the PDF's file name without
  `.pdf`; if that page already belongs to another PDF, `-<4 hex>` of the PDF's hash is added. The page's frontmatter
  says which PDF it belongs to:

  ```markdown
  ---
  file: assets/report.pdf
  sha: 9f2c41d07ab3e815
  kind: annotations
  archived: true
  ---

  - The sentence that was highlighted, as selected. ^a1b2c3
    hl-page:: 12
    hl-color:: yellow
    hl-from:: 1042
    hl-to:: 1187
    hl-context:: …text before… | …text after…
    - My comment on it, an ordinary child block, with [[links]] and #tags.
  - ![area](../../assets/hls/report-p12-3fa90c21.png) ^d4e5f6
    hl-page:: 12
    hl-color:: red
    hl-type:: area
    hl-rect:: 0.2000,0.4000,0.6000,0.2500
  ```

  - `file` is the PDF's path in the graph (`assets/report.pdf`, not relative to the sidecar, so a renamed sidecar
    still finds it); `sha` is the first 16 hex of the
    blake3 of the PDF's bytes. Both are plain page properties (3.1), so they are searchable and queryable
    (`file:: …`). The sidecar is found by `file`; if that finds nothing (the PDF was moved by hand, so its old file is gone)
    by `sha`, and the `file` property is corrected.
  - A highlight is **one block**. Its first line is the quoted text (an area highlight: a picture of the area), its
    comment is its **children**, so notes, tasks, tags and links work as everywhere else. Deleting the block deletes
    the highlight.
  - `hl-page` is the page of the PDF, counting from 1. **A text highlight stores only where it starts and ends**:
    `hl-from` and `hl-to` are character offsets (from 0, `to` exclusive) into the text of that page, which is the
    `str` of pdf.js's text items joined in order, with a newline after an item that ends a line. However long the
    selection is, it is two numbers. The boxes are not stored: the viewer finds the text-layer spans that hold the
    two offsets, takes the range between them and asks the browser for its line boxes, so they fit the zoom, the
    screen and the rotation by construction. If the text at `from..to` (spaces and line breaks ignored) is not the
    quote, the text of the page has changed or the extraction differs (another pdf.js version): the viewer then
    looks for the quote on that page, with `hl-context` to choose between several hits, and corrects the numbers
    (a guarded write); if it is not found the highlight is listed in the panel as _not found on page N_ and is not
    drawn. `hl-context` keeps up to 40 characters before and after the quote for that. An **area** highlight has no
    text, so it stores `hl-rect`, one box `x,y,w,h` as fractions of the page's width and height (0 to 1, from the
    top left, rotation applied). `hl-color` is one of `yellow`, `green`, `blue`, `red`, `purple`.
  - A PDF without a text layer (a scan) has no text to select; the area highlight is what serves there.
  - The block **ID** (`^a1b2c3`) is added when the highlight is made, not when it is first referenced, because a
    highlight is meant to be referenced.
  - **The sidecar is hidden** (`archived: true`, the _Hide_ feature, NS-20): it is not in the sidebar, Recent, the
    property groups or the agenda, and shows in All pages only on request; it stays searchable and linkable, its
    highlights are found by search and `((id))` references work. The viewer's own panel and a link on the PDF's
    page reach it. The _Open the notes page_ button (A4) opens it as a page.
  - The sidecar is a **page like any other** (ST-1): the person can edit it in a text editor, and the index needs no
    new table. The only code that knows its shape is the viewer.

- **Area highlights** (a rectangle drawn on the page) are rendered from the PDF at 2x, cropped, stored as
  `assets/hls/<name>-p<page>-<8 hex>.png` and shown in the block. Deleting the block leaves the picture to the
  _Unused images_ list (ST-8).
- **Not Logseq's format.** Logseq keeps ids as UUIDs in an `id::` property and the coordinates in a separate `.edn`
  file in `assets/`. Reading those (an importer) is not part of this phase.
- **One new backend command**, `annotation_page(pdf_path) -> { page, created }`: finds the sidecar of a PDF in the
  graph or creates it (name, frontmatter, `sha`). Everything else the viewer does is the ordinary page API: insert a
  block with properties under the sidecar, change a property, delete a block, and a page read for the list of
  highlights.
- **Reading the PDF's bytes.** The viewer gets the file through the same route as images (`asset URL` in the browser
  and the dev server, the asset protocol in the app); no new command.
- **Where the viewer lives:** like the drawing editor, a promise-based `openPdf(path, { highlight? })` mounts a
  full-screen view once in `App.svelte`. A phone-width or narrow window shows the list of highlights as a drawer,
  a wide one as a column on the right.

## Tasks

### A1 The sidecar (`annotation_page`) (done)

- `Graph::annotation_page(pdf_path)` in `crates/graph` and the app command; the PDF's hash is computed once and
  cached per path and modification time.
- The path must be a `.pdf` inside the graph's `assets/` (anything else is refused); the frontmatter and the page
  name are made as described above; a second call returns the same page.
- The app has no command that moves or renames an asset, so a moved PDF is only found again by `sha`.
- **Verify:**
  - [x] a PDF in `assets/` gets `pages/hls/<name>.md` with `file`, `sha` and `kind: annotations`; a second call
        returns it and changes nothing
  - [x] two PDFs with the same name in different folders get two sidecars (`-<4 hex>` on the second)
  - [x] a path that is not a `.pdf`, one outside the graph and one with `..` are refused
  - [x] the PDF moved by hand: the sidecar is found by `sha` and `file` is corrected (a guarded write)
  - [x] the sidecar is indexed like a page; a search for a word in a highlight finds the block (a test with a
        fixture sidecar, no PDF needed)
  - [x] the sidecar of a fixture parses and a no-op edit is byte-identical (golden file)

### A2 The viewer

- `PdfViewer.svelte` with `ui/src/lib/pdf/` (`load.ts`: the dynamic import and the worker; `pages.ts`: which pages
  to render for the scroll position; `anchor.ts`: a selection to `from`/`to` offsets and back to a range, the page text, and re-finding a quote).
- Pages in a vertical scroll, rendered lazily and dropped again when far away; fit to width, zoom in steps and by
  pinch, page number box and previous/next page, find in the document (pdf.js's find controller), the text layer
  for selection. A PDF with a password asks for it; a broken file shows a message, not a blank view.
- Opening: a click on a link to a `.pdf` in the graph (`[x](../assets/a.pdf)`, with an optional `#page=12`), and a
  _Open as PDF_ entry in the asset's menu. Escape closes.
- **Verify:**
  - [ ] a fixture PDF opens; page 1 is visible, the text layer holds its words
  - [ ] a 300-page fixture opens in under a second and keeps no more than a few pages rendered (checked by counting
        canvases after scrolling to the middle and to the end)
  - [ ] `#page=12` opens on page 12; the page box and the previous/next buttons agree with the scroll position
  - [ ] zoom keeps the page in view; find jumps to a hit and marks it
  - [ ] a PDF with a password asks for it; a file that is not a PDF shows a message
  - [ ] the loaded JS for pdf.js is not in the first page's bundle (the app starts as fast as before)
  - [ ] the same in Chromium and in WebKit (`webkit.list`)

### A3 Highlighting text

- Select text in the text layer, a small bar appears next to the selection with the five colours (and a Copy), one
  click makes the highlight: `annotation_page`, then a block under the sidecar with the quote, `hl-*` properties and
  an ID, in one undo step.
- The highlights of the sidecar are drawn over the pages (translucent boxes of the range from `hl-from` to `hl-to`, taken from the text layer; an area from `hl-rect`); a click on one opens
  its popup: change colour, delete, **Add comment** (focuses a child block in the side panel, A4), copy
  `((id))`.
- Overlaps, a selection across pages (one block per page, so each has one `hl-page`), a selection in a rotated
  page (the box is computed in page space).
- **Verify:**
  - [ ] selecting a sentence and clicking _yellow_ adds one block to the sidecar with the quote, `hl-page`,
        `hl-color: yellow`, `hl-from`, `hl-to` and an ID; the highlight is drawn over the same words
  - [ ] a selection across two pages makes two blocks, each on its own page
  - [ ] a selection of a whole page of text is stored as two numbers (the block has no long property); the drawn
        boxes cover the same words at 50 %, 100 % and 300 % zoom and on a page rotated by 90° (a fixture; compared
        against the text layer's boxes)
  - [ ] a sidecar whose `hl-from`/`hl-to` are wrong (edited by hand) but whose quote is on the page: the viewer
        finds the quote, corrects the numbers and draws it; a quote that is not on the page is listed as _not found_
        and nothing is drawn
  - [ ] the offsets are the same for the same PDF in Chromium and in WebKit (the page text is built the same way)
  - [ ] changing the colour rewrites only that property (ST-3); deleting removes the block, its comment and the box
  - [ ] one undo removes a highlight made just now; a highlight made on another device (the sidecar changed on
        disk) appears without reopening the PDF
  - [ ] closing and opening the PDF again draws the same highlights

### A4 The side panel

- A list of the highlights in reading order (page, then position), each with its quote, colour bar and comment
  blocks, which are the ordinary outliner blocks of the sidecar (the panel embeds the page's blocks, so editing
  has all the usual features). Filter by colour and by text; a click on the quote scrolls the PDF to the highlight and
  flashes it; selecting a highlight in the PDF scrolls the list.
- A count in the header (_14 highlights_), and _Open the notes page_ to go to the sidecar as a page.
- **Verify:**
  - [ ] the list holds every highlight of the sidecar in page order; one added later (outside the viewer) joins it
  - [ ] typing a comment under a highlight in the panel writes a child block in the sidecar file; `[[links]]` and
        `#tags` in it show up in the backlinks of their pages
  - [ ] a click on a quote scrolls to its page and marks it; a click on a highlight in the PDF scrolls the list
  - [ ] the colour filter and the text filter narrow the list and the drawn highlights
  - [ ] a comment turned into a task (`TODO`) appears in the agenda, with the quote as context

### A5 Area highlights

- A tool next to the colours: drag a rectangle on the page; on release the area is rendered at 2x into
  `assets/hls/…png` (`save_asset`, ST-8) and the block shows the picture. On a touch screen the tool is a mode
  (otherwise a drag scrolls).
- **Verify:**
  - [ ] a drag makes a block with `hl-type: area`, one `hl-rect` and a picture that shows the area (size
        checked, not the pixels)
  - [ ] the rectangle is drawn on the page and can be deleted; the picture stays in _Unused images_ after the block
        is deleted
  - [ ] a rectangle dragged outside the page is cut to the page

### A6 References from the notes

- `((id))` of a highlight, pasted into any block, shows the quote (as now) **and a small chip** _report.pdf p. 12_;
  a click on the chip opens the viewer on that highlight. _Copy highlight reference_ in the popup and the list.
  Also _Copy as quote_, which gives `> quote ((id))` for pasting into a note.
- A link `[Report p. 12](../assets/report.pdf#page=12)` opens on that page (A2).
- **Verify:**
  - [ ] a reference pasted into a journal shows the quote and the chip; the chip opens the PDF at the highlight with
        the highlight flashing
  - [ ] the sidecar block lists the page that references it (backlinks)
  - [ ] deleting the highlight leaves the reference as a dead one, like any removed block (no crash, no empty chip)

### A7 Touch, iPad, documents and checks

- Touch: long press selects a word as on the system, the bar appears above the selection and is 44 px high on a
  coarse pointer; two fingers zoom the page; the Pencil highlights nothing by itself (no ink in this phase); the
  panel is a drawer on a narrow window.
- Documents: `requirements.md` (PA rows, and the non-goal removed), `file-format.md` (the sidecar), `keymap.md` (the
  viewer's keys and mouse), `testing.md`, `TODO.md`, and _Open checks_ in this file.
- Tests: Rust (A1), Vitest (`anchor.ts`: selection to offsets and back, line breaks, re-finding a quote with context, cross-page selections),
  Playwright with fixture PDFs (a one-page and a 300-page, made by a script so no binary is hard to review),
  Chromium and WebKit.
- **Verify:**
  - [ ] `scripts/check.sh` and `scripts/check-webkit.sh` pass with the new specs
  - [ ] on a real iPad: selecting text by long press and handles, the colour bar, a 100-page PDF scrolls smoothly,
        the drawer, pinch zoom (not testable here; stays open until done)
  - [ ] the desktop app: a PDF opened from a link renders and highlights (WebKitGTK)
  - [ ] a graph with highlights synced to a second device shows the same highlights (the sidecar is only text)

## Later (not in this phase, but not made impossible)

- **Ink on the PDF** (option B): the drawing editor as a transparent layer over a page, one SVG per page in
  `assets/hls/`, with a `hl-type:: ink` entry in the sidecar so ink and highlights share the list.
- **Finding a highlight again in a changed PDF on another page** (this phase re-finds a quote only on its own page): a
  text search over the whole file with `hl-context` and the quote.
- **Highlights as tasks**, a reading view across several PDFs, **export**: the highlights as a list of quotes in a note
  and an annotated copy of the PDF (standard `/Highlight` objects written into a _copy_, never into the original).
- **Import of a Logseq `hls__` page and its `.edn`.**
- **EPUB and images** with the same sidecar (`kind: annotations`, a different locator than `hl-page`).

## Decided in review

- Sidecars are hidden by default (`archived: true` in their frontmatter).
- A text highlight stores only its start and end (`hl-from`, `hl-to`), not boxes.

## Open questions

- **The page text must be built identically everywhere.** The offsets depend on how `str` of the items is joined
  (the newline after an end-of-line item, items with no text). A1/A3 fix one function, `pageText()`, used for
  writing and reading, with a test on a fixture; a change to it needs a migration (the quote re-finding above covers
  a mistake, at the cost of a guarded write per highlight).
- **Selections across pages** are still one block per page (`hl-from`/`hl-to` are offsets in one page's text).
- The values `hl-from`/`hl-to` are plain numbers; `hl-context` has `|` and `…` in it and has to survive the property
  parser as text (the golden file in A1 covers it).
