# Annotating EPUBs (E1 to E7)

**Goal:** read an EPUB in the app, highlight text and write notes on the highlights, exactly as for a PDF
([pdf-annotation.md](pdf-annotation.md)): the highlights are ordinary blocks of a hidden sidecar page, so they sync, are
found by search, can be tagged, queried and referenced from any page with `((id))`, and a reference jumps back to the
spot in the book. The EPUB file is never changed.

**Requirements covered:** EA-1 to EA-10 (see `requirements.md`).
**Depends on:** the PDF annotation phase (the sidecar, `annotation_page`, the panel, the popup, the `((id))` chip), assets
(ST-8).
**Status:** E1 to E7 built and tested in Chromium and WebKit. Open: the manual checks on a real iPad, in the desktop app (WebKitGTK), across two synced devices and with real books (E7).

## Design

- **Reader: [foliate-js](https://github.com/johnfactotum/foliate-js)** (MIT; the engine of the Foliate reader, pure
  JavaScript, native ES modules, no build step, no hard dependencies). It reads EPUB 2 and 3 (and, as a side effect,
  MOBI/AZW3, FB2 and CBZ), paginates reflowable books with CSS columns (paginated or scrolled), reads and writes **EPUB
  CFIs**, draws highlights with an SVG overlay, searches the book and gives the table of contents and the reading
  progress. Its own README says it is **not stable** (no release, the API may change at any time) and recommends
  including it as a submodule. So:
  - **It is vendored**, unmodified, at a pinned commit in `ui/vendor/foliate-js/` with its `LICENSE`, and **all of
    our code talks to it through one file**, `ui/src/lib/epub/foliate.ts`. Updating it is one commit that re-runs the
    EPUB tests; nothing else imports from `vendor/`.
  - **It is loaded only when a book is opened** (dynamic `import()`), like pdf.js: the first page load does not change.
  - The EPUB is read with `fetch` into a `Blob` and handed to `view.open(blob)`; foliate reads the zip with its own
    zip.js (random access, so a big book is not unpacked as a whole).
- **The EPUB stays untouched** (ST-3). It lives in the graph as an asset (`/file` → This computer → Copy, a drop, or
  by hand) and is linked `[Book](../assets/book.epub)`. A click on a link to an `.epub` **inside the graph** opens the
  reader; one elsewhere still opens outside.
- **Same sidecar, a different locator.** `annotation_page` accepts `.epub` as well as `.pdf` and makes the same page
  `hls/<name>` (frontmatter `file`, `sha`, `kind: annotations`, `archived: true`). A highlight is one block:

  ```markdown
  - The sentence that was highlighted, as selected. ^a1b2c3
    hl-cfi:: epubcfi(/6/14!/4/2/1,:20,:97)
    hl-chapter:: 3 · The Storm
    hl-color:: yellow
    hl-context:: …text before… | …text after…
    - My comment on it, an ordinary child block.
  ```

  - `hl-cfi` is a **range CFI**, which is a start and an end and nothing else, so a highlight is the same size
    however long the passage is (the rule decided for PDFs). It is the standard of EPUB, so Calibre, Kobo and Apple
    Books can read the same locations. It contains the spine step (`/6/14`), so the chapter is part of it.
  - `hl-chapter` is only for people and for the panel: the number in the reading order and the title from the table of
    contents. It is rewritten when it is wrong and nothing depends on it.
  - `hl-color` and `hl-context` are as for a PDF (`hl-context` holds up to 40 characters on each side of the quote).
    There is no `hl-page`, `hl-from` or `hl-to`, and no area highlight: a book has no fixed page to draw a rectangle on.
  - A PDF highlight and an EPUB highlight are told apart by the properties they have (`hl-page` or `hl-cfi`); one sidecar
    belongs to one file, so they are never mixed.

- **Why a CFI and not offsets as for a PDF.** A PDF page is a fixed unit with a text we extract ourselves. In a
  reflowable book the library already knows where a passage is, and its CFI survives a change of font, size and
  window, which character offsets in our own text extraction would also do but in a private format. The text offsets
  are still used, but only to **find a highlight again**: if the CFI does not resolve, or the text it covers is no
  longer the quote (the book was replaced by a corrected edition), the viewer searches the quote in the text of the
  chapter (`anchor.ts`, the same `resolveAnchor` as for PDFs, with `hl-context` to choose between equal quotes), makes a new
  CFI from where it found it and corrects `hl-cfi` with a guarded write. If it is not found, the highlight is listed
  as _not found in chapter N_ and is not drawn.
- **A selection never spans chapters**: foliate-js shows one chapter at a time, each in its own frame, so a selection
  belongs to one chapter and is always one block with one CFI (a PDF selection over two pages is two blocks).
- **Safety first.** An EPUB is a web page that someone else wrote. foliate-js shows each chapter in an `iframe` from a
  `blob:` URL, which has the origin of the app, and cannot sandbox them (its README explains the WebKit bug). So the
  app's **Content-Security-Policy is what keeps a book's scripts from running**: `script-src 'self'` stays as it is, `blob:`
  is **not** added to it, and only `frame-src`, `img-src`, `font-src` and `style-src` get `blob:`. Remote images, fonts
  and styles stay blocked by the same policy (a book cannot phone home), and there is a test with a book that tries.
  Links in a book: an internal one moves in the book, an external one goes through `backend.openExternal` (only
  `http`, `https`, `mailto`), anything else is dropped.
- **Reading settings are per device, not in the graph**: theme (light, sepia, dark; the book's own colours are
  overridden), font size, flow (paginated or scrolled) and the **place where the reader stopped**, kept in
  `localStorage` under the book's `sha`. They are not synced (ST-2: it is state, not content).
- **The shared parts.** The panel, the popup, the selection bar, the filters, the comments (child blocks), the agenda
  rule and the `((id))` chip are the PDF viewer's. E2 moves them out of `PdfViewer.svelte` so both viewers use one
  copy; the PDF specs are the proof that nothing changed.
- **Not in this phase:** fixed-layout EPUBs (they are shown with a message), DRM-protected books, MOBI/AZW3/FB2/CBZ
  (foliate opens them, but they are not offered or tested), text-to-speech, dictionaries, images as highlights.

## Tasks

### E1 The sidecar of an EPUB (`annotation_page`) (done)

- `is_annotatable_path` in `crates/graph/src/pdfs.rs` (a file below `assets/` ending in `.pdf` or `.epub`, same rules
  about `..`, backslashes, empty segments); `annotation_page` uses it; the error text says "a PDF or an EPUB". The sha
  is the same function (streamed blake3, first 16 hex).
- The sidecar name is `hls/<stem>`; two files with one stem (`book.pdf` and `book.epub`) get `-<4 hex>` on the second,
  as two PDFs do.
- A highlight block with `hl-cfi` parses like any block; golden cases for the shape.
- **Verify:**
  - [x] an EPUB in `assets/` gets `pages/hls/<name>.md` with `file`, `sha`, `kind: annotations`, `archived: true`; a second
        call returns it and changes nothing; `create: false` makes nothing
  - [x] `book.pdf` and `book.epub` side by side get two sidecars
  - [x] a path that is not a `.pdf` or `.epub`, one outside `assets/`, one with `..` is refused
  - [x] a moved EPUB is found by `sha` and `file` is corrected
  - [x] a sidecar with a CFI highlight parses, `hl-cfi` keeps its parentheses, commas and colons, a no-op edit is
        byte-identical, and setting `hl-color` rewrites only that line (golden cases in `fixtures/edits/` and
        `fixtures/annotations/`)
  - [x] a search for a word in the quote of a CFI highlight finds the block

### E2 The shared parts move out of the PDF viewer (done)

- `ui/src/lib/annotate/`: the sidecar handling (`ensureSidecar`, `pathOfUid`, edit/colour/delete/comment ops,
  `copyText`), the model (`Highlight` has `kind: 'text' | 'area' | 'epub'`; an `epub` one has `cfi` and `chapter` and no page or
  offsets; `highlightsOf` takes the kind from the `file` in the sidecar's frontmatter and ignores blocks with the other
  kind of locator), and the components `AnnotationPanel.svelte`, `HighlightPopup.svelte`,
  `SelectionBar.svelte`. `highlightsOf` reads both kinds. `PdfViewer.svelte` keeps only what is PDF.
- No behaviour change: `pdf.spec.ts` and `pdf-touch.spec.ts` are not edited and pass in Chromium and WebKit.
- **Verify:**
  - [x] the PDF specs pass unchanged (Chromium and WebKit); the vitest tests of `highlights.ts` still pass, with new
        cases for `hl-cfi` blocks, a mixed list, and a block with both kinds of property (ignored, the file decides)
  - [x] `PdfViewer.svelte` is smaller by the moved code and imports it (no copy is left)

### E3 The reader (done)

- `ui/vendor/foliate-js/` at a pinned commit (with `LICENSE`, and a `README` line saying which commit and why it is
  not edited); `ui/src/lib/epub/foliate.ts` (the loader, `open(blob)`, the events we use, `getCFI`, the overlayer
  hooks, `search`, the table of contents); `EpubViewer.svelte`; `openBook({ path, cfi?, highlight? })` in
  `commands/epub.ts`, next to `openPdf`; a click on a link to an `.epub` in the graph opens it (`Inlines.svelte`).
- The reader: previous/next page (keys, a click on the edge, a swipe), a progress bar with the percent, a table of
  contents drawer, **Find** (foliate's search, hits marked, previous/next), theme, font size, paginated or scrolled,
  internal links (footnotes included) and external links as described above, the place kept per device. Escape closes.
  An unreadable file, a DRM-protected book and a fixed-layout book show a message, not a blank view.
- The CSP changes (`ui/index.html` and `src-tauri/tauri.conf.json`): `frame-src blob:`, `img-src … blob:`, `font-src
'self' blob: data:`, `style-src … blob:`. `script-src` is not touched.
- **Verify:**
  - [x] a fixture EPUB (three chapters, a table of contents, an image, a footnote) opens; the first chapter's text is
        on the page; next/previous change the page; the percent grows; the table of contents opens a chapter
  - [x] a book with 60 chapters opens at once and only a few chapters are loaded (the number of section iframes is
        checked after jumping to the middle and to the end)
  - [x] a book with a `<script>` in a chapter, a script in an `onclick`, a `javascript:` link, a remote `<img>` and
        a remote stylesheet: the script does not run (a flag it would set on `window.parent` stays unset), the link
        does nothing, the remote requests are blocked (no request leaves for them)
  - [x] an external link in a book calls `openExternal` with an `https` address and nothing for other schemes
  - [x] the theme, the font size and the flow change the display and are still set after closing and opening the book;
        the place where the reader stopped is restored (and is not restored for another book)
  - [x] Find marks hits across chapters and steps through them
  - [x] a fixed-layout fixture and a file that is not an EPUB show a message
  - [x] the foliate code is not in the first page's bundle or requests (as for pdf.js)
  - [x] the same in Chromium and in WebKit

### E4 Highlighting text (done)

- Select text in a chapter (the selection is inside a section `iframe`, so `selectionchange` is listened to on each
  section document when foliate says it loaded): the colour bar appears next to the selection (its position is the
  range's rectangle plus the iframe's offset). A colour makes `annotation_page`, then one block under the sidecar
  with the quote, `hl-cfi`, `hl-chapter`, `hl-color`, `hl-context` and an ID, in one undo step.
- The highlights of the sidecar are drawn with foliate's overlayer (a translucent box per line, in the colour); a click
  (foliate's `show-annotation`) opens the shared popup: colour, delete, comment, copy reference, copy as quote.
- Finding again: when a chapter loads, each of its highlights is checked (the CFI resolves, and the text of the range is
  the quote with white space ignored); if not, the quote is searched in the chapter's text, the CFI is corrected with a
  guarded write, or the highlight is marked _not found_.
- **Verify:**
  - [x] selecting a sentence and choosing _yellow_ adds one block with the quote, a CFI that foliate resolves back to
        the same words, `hl-chapter`, `hl-color: yellow`, `hl-context` and an ID; the highlight is drawn over the words
  - [x] a selection of a whole page of text is one short `hl-cfi` (no long property)
  - [x] the highlight stays on the same words after a change of font size, of theme, of flow and of window size
  - [x] changing the colour rewrites only that property (ST-3); deleting removes the block, its comment and the drawing
  - [x] one undo removes a highlight made just now; a highlight made on another device (the sidecar changed on disk)
        appears without reopening the book
  - [x] a sidecar whose CFI is wrong (edited by hand) but whose quote is in the chapter: the highlight is found,
        drawn, and the CFI in the file is corrected; a quote that is not in the book is listed _not found_ and not drawn
  - [x] the same highlight, made in Chromium, is at the same words in WebKit (the CFI means the same everywhere)
  - [x] Chromium and WebKit

### E5 The panel and the comments (done)

- The shared panel with the EPUB's locator: the label is the chapter (`3 · The Storm`) instead of `p. 12`; the list is in
  reading order (spine index, then `CFI.compare`); a click on a quote goes to the highlight (`goTo(cfi)`) and flashes
  it; selecting a highlight in the book selects its entry; the colour and text filters; _＋ Comment_ and the comment
  editing are the shared ones; _Not found in chapter N_.
- **Verify:**
  - [x] the list holds every highlight of the sidecar in reading order; one added later (outside the viewer) joins it
  - [x] typing a comment writes a child block in the sidecar file; `[[links]]` and `#tags` show up in the backlinks;
        a `TODO` comment is on the agenda with the quote above it
  - [x] a click on a quote opens the book at that highlight and marks it; a click on a highlight selects its entry
  - [x] the colour filter and the text filter narrow the list and the drawn highlights

### E6 References from the notes (done)

- `((id))` of an EPUB highlight shows the quote and a chip _book.epub · 3 · The Storm_ (`BlockRefView.svelte` looks for
  `hl-cfi` as well as `hl-page`, and for the file's extension); the chip calls `openBook` at the highlight. A link
  `[Book](../assets/book.epub#epubcfi(/6/14!/4/2))` opens the book at that place (the note renderer keeps an
  `#epubcfi(…)` fragment of an `.epub` link, as it keeps `#page=N` of a PDF; the link is written in `<…>` because of
  the parentheses, which `fileLink` already does).
- **Verify:**
  - [x] a reference pasted into a journal shows the quote and the chip; the chip opens the book at the highlight, which
        flashes
  - [x] a link with `#epubcfi(…)` opens at that place; one with a bad CFI opens at the start of the book
  - [x] deleting the highlight leaves a dead reference without a chip (no crash)
  - [x] a unit test of the renderer: the fragment is kept for `.epub` and dropped for other files

### E7 Touch, iPad, documents and checks (built; manual checks open)

- Touch: the colour bar and the buttons are 44 px on a coarse pointer, a swipe turns the page, a tap on the edge turns
  it, the text selection of the system works inside the section frames, the panel is a drawer on a narrow window.
  Whether iOS lets the selection handles work inside a `blob:` iframe is the main unknown (see Open checks).
- Documents: `requirements.md` (EA rows), `file-format.md` (the EPUB highlight), `keymap.md` (the reader's keys and
  mouse), `testing.md`, `TODO.md`, `CLAUDE.md` (the doc list), and the vendoring note (which commit, how to update).
- Tests: Rust (E1); Vitest (`highlights.ts` with CFI blocks; the pure part of re-finding over a chapter's text);
  Playwright with EPUBs made by `ui/tests/epub-fixture.ts` (a small zip writer, `mimetype` first and stored, no compression
  needed: a plain book, a book with an image and a footnote, a 60-chapter book, a hostile book, a fixed-layout book),
  Chromium and WebKit.
- **Verify:**
  - [x] `scripts/check.sh` and `scripts/check-webkit.sh` pass with the new specs (Rust, 293 unit tests, 397 browser tests in Chrome, 127 in WebKit)
  - [ ] on a real iPad: selecting text by long press and handles inside the book, the colour bar, turning pages by
        swipe, a 100-chapter book, the drawer (stays open until done)
  - [ ] the desktop app: a book opened from a link renders and highlights (WebKitGTK), the CSP lets the chapters
        load and still blocks a script
  - [ ] a graph with EPUB highlights synced to a second device shows the same highlights
  - [ ] three real books (a novel, one with many images, one with footnotes and a table of contents of several levels)
        open, paginate, and take a highlight in each chapter type

### How it was built, where it differs from the plan

- foliate-js is vendored at commit `78914aef…` (2026-05-01) in `ui/vendor/foliate-js/` (see its `README.md`: which
  files, how to update). The PDF adapter and the 10 MB of PDF.js it needs are left out; `pdf.js` there is a stub. It is
  bundled by Vite through `import.meta.glob` in `ui/src/lib/epub/foliate.ts`, so TypeScript does not follow the
  imports into code written for other compiler settings. ESLint and Prettier skip `vendor/`.
- The CSP gets `frame-src blob:`, `font-src 'self' data: blob:`, and `blob:` in `img-src` and `style-src`; `script-src`
  is untouched. `connect-src` gets `blob:` as well, because foliate-js reads a book's style sheets (and what they `@import`) with a `fetch` of a `blob:` address; without it a book with CSS loses its styles (found while checking a web clip, W1; the style-sheet spec covers it). The first reader spec fails if the policy refuses anything a plain
  book needs; the hostile-book spec fails if a script runs or a remote request leaves.
- The shared parts are `ui/src/lib/annotate/` (`Sidecar`, colours) and `SelectionBar`, `HighlightPopup`,
  `AnnotationPanel`. Their test ids keep the `pdf-` prefix of the first viewer, so the PDF specs were not touched.
- A highlight block: `^id`, `hl-cfi`, `hl-chapter` (`<section number> · <table-of-contents entry>`), `hl-color`,
  `hl-context`. After a chapter loads, each highlight is checked against it: the CFI must resolve in that chapter and
  the words under it must be the quote (white space ignored); if not, the quote is searched in the chapter's text and
  the CFI is corrected with a guarded write; if that fails the highlight is listed as not found and not drawn.
- The reader draws highlights through foliate's overlay, which is in a closed shadow root, so the reader also keeps a
  visually hidden list of the highlights drawn in the chapter on the screen (`epub-marks`: the colour, the words under
  the drawing, whether it is flashing). The tests read it.
- Where the reader stopped is kept per device in `localStorage` under the book's `sha`; theme, size and layout in one
  more key. A CFI in a link or a saved place that leads nowhere in the book is dropped and the book opens at its start.
- A tap on the left or right fifth of the page and a swipe turn the page (paginated layout); a tap on a highlight
  opens its menu and does not turn the page.
- The note renderer keeps an `#epubcfi(…)` fragment of an `.epub` link (only the characters a CFI uses); the link is
  written in `<…>` because of the parentheses.
- The app's own error log ignores two harmless browser/foliate messages (`ResizeObserver loop…`, and foliate's resize of
  a chapter frame that is being removed).

### Open checks (not done here)

- On a real iPad: selecting text inside the book by long press and with the handles (the system's own bar appears too),
  swipes, a 100-chapter book, the drawers.
- In the desktop app (WebKitGTK): a book opened from a link renders and highlights, the CSP lets the chapter frames,
  pictures and fonts load and still blocks a script.
- Two devices: highlights made on one appear on the other after a sync.
- Real books: a novel, one with many images, one with footnotes and a table of contents of several levels, an
  EPUB 2. Only generated books were run.

## Later (not in this phase, but not made impossible)

- **Other formats foliate opens** (MOBI/AZW3, FB2, CBZ): the sidecar, the locator for a text book and the CSP are
  the same; they need only an extension in `is_annotatable_path`, an offer in the file picker, and tests with real files.
- **Images as highlights** and a highlight on a picture of a book.
- **A shared place in the book across devices** (the last position in the sidecar or the device-synced state).
- **Export:** the highlights as a list of quotes in a note; an annotated EPUB written into a _copy_ (never the original).
- **Text-to-speech and dictionary lookups** (foliate has modules for both).

## Open questions

- **Which foliate-js commit.** Chosen in E3 by trying the newest in Chromium and WebKit; a regression there means an
  older pin. The decision and the date go into the vendoring note.
- **CFI across foliate versions.** A CFI is a standard, but foliate-js may change how it handles edge cases (ignored
  nodes, text offsets after inline markup). The re-finding by quote is the safety net; E4 has a test that a wrong CFI
  is repaired, and the vendoring note says to re-run it on every update.
- **Selection inside `blob:` iframes on iOS** (handles, the system callout over our bar). If it does not work well,
  the fallback is a long press that selects a word and our own handles, which is more work and is not planned here.
- **Big books.** The paginator uses CSS columns and is slow on very long chapters. If a real book is too slow, the
  options are scrolled flow for it or a smaller chapter size from foliate's options; checked with the three real books
  in E7.
- **Hidden sidecar for books.** Same as for PDFs: `hls/<name>` is hidden; the book's own panel reaches it.
