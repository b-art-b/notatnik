# Documents beside the notes (DK1 to DK4)

**Goal:** read and annotate a PDF or an EPUB (also a clipped page, a Nextcloud document, a file of this computer) **in
the side panel, with the notes page next to it**, instead of in a full-screen window that covers the notes. You
highlight and comment on the right, and write, paste a quote (`> quote ((id))`) or follow a reference on the left, without
closing anything.

Docking is a **choice, not the default**: a document opens full screen as it does today. A button in the viewer moves it
into the side panel and back, and a device setting (off) can make documents open docked.

**Requirements covered:** DD-1 to DD-8 (see `requirements.md`); PA-1 and EA-1 (the viewers) get a second place to live.
**Depends on:** the PDF and EPUB viewers and the shared sidecar, panel and popup (PA-1..PA-10, EA-1..EA-10), the right
side panel (`SidePanel.svelte`, `rightOpen`, `rightWidth`, the resizer), the document targets of the Nextcloud and local
phases ([nextcloud-annotation.md](nextcloud-annotation.md), [local-annotation.md](local-annotation.md)).
**Status:** DK1 to DK4 done and tested (Chromium and WebKit). Open: the checks in the real desktop app and on the iPad.

## Decisions

- **The existing right panel is the only panel.** No left panel, no mirrored layout, no third column: the notes stay where
  they are and the document goes into the side panel that is already there, which already has a resizer, a focus mode
  (double click on the resizer folds the notes away) and a show/hide key (Ctrl+Shift+\).
- **Full screen stays the default.** Every document opens full screen. _Dock_ in its header moves it into the panel for
  as long as it is open. _Settings → Linked files → Open documents beside the notes_ (a switch, **off**, this device)
  makes them open docked. Nothing remembers the last choice by itself, so docking never becomes the default by use.
- **One document at a time,** as today. Opening another (a link, a chip, _Open in the document_) replaces the one in the
  panel; it does not add a tab for each document.
- **Not on a narrow window.** The dock button is offered when the window is at least 900 px wide (the notes keep their
  320 px and the panel its 280 px, with room to spare); on a phone or a narrow window the document is full screen as
  now. A tablet in landscape can dock; in portrait it cannot.
- **Nothing is reloaded by docking.** Moving a document between full screen and the panel keeps its place, its zoom, its
  find, its selection and its open comment (see _Design_).

## Design

- **The viewer stays where it is mounted; only its place on the screen changes.** `PdfViewer` and `EpubViewer` are
  mounted once in `App.svelte`, as now. A prop `docked` (from `app.pdf.docked` / `app.book.docked`) changes the root
  element from a fixed, dimmed overlay to a plain box. In the panel, the box is placed **over the panel's body by its
  geometry** (`position: fixed` with the rectangle of a slot in the panel, kept in step by a `ResizeObserver`), not
  moved into it. Moving the DOM node or mounting a second copy would reload the EPUB's chapter frames and pdf.js and
  lose the place; this does not.
- **A tab in the side panel.** `SidePanel` gets a `document` tab (label: the file's name, with ✕) that holds the empty
  slot. It is the current tab while a document is docked; the other tabs (References, Tasks, Pages) stay one click away,
  and the document is only hidden (not closed) while one of them is shown. Docking opens the panel if it was closed.
  Closing the document (✕, or Close in its header) removes the tab and returns to the tab that was current.
- **Layout by the box, not by the window.** The viewers' narrow-window rules (`@media (max-width: 900px)`: the highlights
  panel as a drawer, fewer buttons) become container queries on the viewer's own box (`container-type: inline-size`),
  so a viewer in a 420 px panel lays out as a viewer in a phone-sized window does now, and a full-screen one is as
  before. The header gets an overflow menu for what does not fit (_Find_, _Aa_, _System viewer_, _Dock/Full screen_); the
  PDF is laid out for the width of its scroller and the EPUB reader for the width of its box (both already follow their
  box with a `ResizeObserver`).
- **Not modal when docked.** A docked viewer is a `region` (`aria-label` with the file's name), not an `aria-modal`
  dialog, takes no focus when it opens (the note you were writing keeps the cursor), has no backdrop and no focus trap.
  Full screen is exactly as today.
- **Keys.** Everything the page and the app do keeps working: Alt+1..5, Ctrl+K, Ctrl+Shift+\, and every editing key in
  the notes. The viewer's own keys (←/→ and PageUp/Down in the book, Ctrl+F, Ctrl+± in the PDF, Escape to leave a popup, the
  colour bar, the find bar and finally the document) act only while the focus is inside the viewer. In the panel Escape
  closes the popup, the bar and the find bar as now, and closes the document only when the viewer has the focus and
  nothing is open.
- **Writing next to the document.** Clicking in a note while a selection is open in the document ends the selection (the
  colour bar goes), as clicking anywhere does; a note that was being edited is saved and left, as for any click outside
  a block. A comment typed in the viewer's own panel and a note typed in the page are one editing state, so only one of
  them is being edited at a time (the other is saved when you click into it).
- **Links, chips and buttons.** A click on a link to a PDF or an EPUB, a `((id))` chip, _Open in the document_ or _Open the
  document_ opens the document in the mode the setting says (full screen unless the setting is on); when a document is
  docked already, the new one replaces it **in the panel**, whatever the setting says, so a reader who docked once does
  not have the screen covered by the next click. **Ctrl+click** and the ↗ (system's program) are as before.
- **The state.** `app.pdf` / `app.book` get `docked: boolean`; a store function `dockDocument(on)` flips it, opens the panel
  and selects the tab. The remembered setting is `localStorage` (`notatnik.dockDocuments`), per device like the other
  reader settings.
- **Not docked:** the Nextcloud and local documents, clips and files of the graph all dock alike (they are all viewer
  targets); the bar _From your Nextcloud_ / _On this computer_ shows in the panel too, one line high.

## Tasks

### DK1 The viewers can live in a box

- `docked` prop on `PdfViewer` and `EpubViewer`: a plain box instead of the overlay, `region` instead of modal dialog, no
  focus on open, no backdrop; the narrow rules become container queries; the header overflow menu; the highlights panel
  as a drawer in a narrow box. `AnnotationPanel` follows the same container.
- **Verify:**
  - [x] a viewer in a 420 px box: the header fits (with the overflow menu), the highlights panel is a drawer, the
        selection bar and the popup stay inside the box, nothing scrolls sideways
  - [x] the same viewers full screen look and work as before (the existing specs pass unchanged)
  - [x] a PDF is laid out for the width of its box and follows when the box is resized; an EPUB re-flows
        (paginated and scrolled) and keeps its place when the box is resized
  - [x] a docked viewer takes no focus when it opens (tested for a document opened by the setting; that a block being edited
        keeps its cursor is by construction, not a separate test: a click on a link ends the editing first)
  - [x] role and name: a `region` named after the file when docked, a modal `dialog` when full screen

### DK2 The panel tab, the geometry slot, docking and undocking

- `SidePanel` gets the `document` tab and its slot; `App.svelte` places a docked viewer over the slot; `dockDocument`;
  the _Dock_ / _Full screen_ button; the setting; the 900 px rule; opening the panel.
- **Verify:**
  - [x] _Dock_ moves an open document into the side panel (the panel opens if it was closed) and the notes page is
        visible and usable beside it; _Full screen_ moves it back
  - [x] docking and undocking keep the document's place (PDF page and scroll, EPUB position), zoom, theme and size,
        find, and highlights; the EPUB's chapter frames are not reloaded (the same frame element survives)
  - [x] dragging the panel's resizer re-lays out the document live; the focus mode (double click) gives it the whole window
        and the notes come back with it
  - [x] the References, Tasks and Pages tabs still work; choosing one hides the document without closing it; choosing the
        document tab shows it again where it was
  - [x] closing the document (✕, Close, a new document replacing it) removes the tab and returns to the previous tab
  - [x] below 900 px there is no _Dock_ button and a document opens full screen even with the setting on; a window made
        narrower while docked brings the document back to full screen
  - [x] the setting is off by default; on, a link opens docked, and the next link replaces the docked document in the panel
        (also with the setting off)

### DK3 Keys, focus and writing next to the document

- Viewer keys act only while the focus is inside; Escape rules; the editing state shared by the notes and the viewer's
  comments; links, chips and buttons route by mode; no key of the notes is taken by a docked viewer.
- **Verify:**
  - [x] typing in a note with a document docked: every key reaches the note (arrows, Escape, Ctrl+Z, Alt+1..5, Ctrl+K);
        the book does not turn pages and the PDF does not zoom
  - [x] with the focus in the book, ←/→ turn pages and Ctrl+F finds; with the focus in the PDF, Ctrl++ and Ctrl+− zoom
  - [x] Escape: with the focus in the docked viewer it closes the popup, then the bar, then the find bar, then the
        document; with the focus in a note it never closes the document
  - [x] highlight a sentence, _Copy as quote_, click in the note, paste: the note holds `> quote ((id))` and its chip opens
        the document at the highlight, in the panel
  - [x] a comment typed in the viewer's panel and a note typed on the page are saved correctly, whichever came first
  - [x] a click on a link to a PDF or an EPUB, a chip, _Open in the document_ and _Open the document_ replace the docked
        document in the panel and leave the notes page as it is

### DK4 Tests, documents, checks

- Playwright (Chromium and WebKit) with a PDF and an EPUB, window sizes 1850 px and 1000 px, touch emulation; docs:
  `requirements.md` (DD rows), `keymap.md`, `testing.md`, `TODO.md`, `CLAUDE.md` (the doc list).
- **Verify:**
  - [x] the whole path, for a PDF and for an EPUB: open full screen, _Dock_, highlight and comment, copy as quote, paste in
        the note, click the chip, resize the panel, _Full screen_, close
  - [x] the setting and the 900 px rule, by keyboard (Tab to _Dock_, Enter) and by touch (44 px)
  - [x] `scripts/check.sh` and `scripts/check-webkit.sh` pass
  - [ ] by hand, in the real desktop app (WebKitGTK): docking an EPUB with embedded fonts and a PDF of 300 pages, selecting
        text in the panel while a note is open, the panel at 360, 600 and 900 px, and on the iPad in landscape

### How it was built, where it differs from the plan

- **The box is measured, not queried.** `container-type` on the viewer would have made it a containing block for the colour bar
  and the popup (they are `position: fixed` in window coordinates), so each viewer measures its box (`bind:clientWidth`) and
  sets a class `narrow` at 900 px or less; `AnnotationPanel` takes a `narrow` prop. A full-screen viewer in a 900 px window
  behaves as before.
- **The header wraps; there is no overflow menu.** The header already wraps, the chapter label and the site's host are hidden
  in a narrow box, and Dock/Full screen, System viewer and Close stay buttons. A menu can come if a real panel proves too
  crowded (the hand check at 360 px decides).
- **One tab is no tab bar.** The panel shows its tab bar only with two tabs or more, as it always did; with only the document
  there is no tab, and the viewer's own Close closes it. With more tabs the document's tab has a ×.
- **The list of highlights starts closed in a narrow box** (a drawer over the document); it reopens when the box is wide again
  if it was open. Found by the drawer test: it was open and covered the text.
- **The PDF keeps its place across a change of width.** The browser keeps the scroll offset in pixels, which is another page
  once the pages are narrower or wider; the viewer now remembers the page at the top and how far down it is, and puts that back
  after the layout changed (and ignores the scroll events the browser makes by itself in the next 250 ms).
- **The slot is followed every frame.** The viewer is placed over the panel's slot by its rectangle, read once a frame while the
  document tab is shown (a resize of the panel, the sidebar opening or a scroll moves the slot without an event), and only
  written when it changed. The WebKit specs pass with it; the desktop app's engine is the place to look for lag.
- **`dock.svelte.ts`** holds the width of the window, the setting, `dockDocument` and the tab bookkeeping; `openPdf` and `openBook`
  decide `docked` (a docked document is replaced in the panel; otherwise the setting, on a window of 900 px or more) and close
  the other kind of viewer, so there is one document at a time.
- **What the tests did not do on their own:** the comment typed in the viewer's panel and a note typed on the page in either
  order (they are one editing state, and the existing comment specs cover the panel); the chip, _Open in the document_ and
  _Open the document_ were not each tried as a replacement of a docked document (a link was; they all go through `openPdf`
  and `openBook`).

## Later (not in this phase, but not made impossible)

- **Two documents** (a tab for each) and a document beside another page of the notes in the right panel.
- **Dragging** a highlight or a quote from the viewer's panel into a note, as a reference.
- **The document on the left, the notes on the right** (a mirrored layout); asked for, and left out on purpose: it
  would be a second panel.
- **A reading position per device** that survives a restart, shared by the docked and the full-screen viewer (the EPUB
  already keeps one).

## Open questions

- **How wide is enough?** 900 px is a guess that keeps 320 px for the notes and 280 px for the panel with room; the
  hand tests at 1000 px on a laptop decide whether the limit or the panel's minimum width should change.
- **Selection across the two sides.** Clicking into a note ends a selection in the document; keeping the colour bar while
  the note is typed in is possible (the bar belongs to the viewer) but unusual, and is left as it is until someone asks.
- **WebKitGTK and `position: fixed` over a scrolling panel.** The geometry approach is simple; the desktop app's engine is
  the place to check that it does not lag behind a resize or a scroll of the page next to it.
