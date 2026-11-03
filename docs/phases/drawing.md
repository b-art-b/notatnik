# Handwriting and drawings (D1 to D5)

**Goal:** write and draw by hand in a note (mouse, finger, stylus; later the Apple Pencil on an iPad) and keep the
handwriting as it was written: no shape correction, no straightening. A drawing is an image in the note on an
infinite canvas, edited in a full-screen overlay, as in Joplin.

**Requirements covered:** DR-1 to DR-11 (see `requirements.md`).
**Depends on:** assets (ST-8). `canvas.plan` has the reasoning behind these choices.
**Status:** D1 to D4 built and tested (Chromium and WebKit). D5 (iPad) is not started.

## Design

- **Library:** [js-draw](https://github.com/personalizedrefrigerator/js-draw) (MIT, no framework, the one Joplin's
  drawing plugin uses). It gives an infinite canvas, pan and zoom, pen, eraser, selection, undo and redo, and reads
  and writes SVG. It mounts into a `<div>`, so `DrawingEditor.svelte` is a thin wrapper.
- **Handwriting stays as written.** The default pens of js-draw can change a stroke: _shape autocorrect_ turns a
  held-still stroke into a line, circle or rectangle, an _input stabilizer_ pulls the pen toward a smoother path, and
  the curve fit may move a stroke up to 3 screen pixels. The editor turns the first two off on every pen. The curve
  fit of the stock freehand builders (1 to 3 screen pixels) went too: it rounds off the small details of a
  handwriting, which looked like Joplin's smoothing. `ui/src/lib/rawInk.ts` is an own stroke builder that keeps every
  point the pen reports and draws a quadratic curve per point through the middles of the segments (nothing dropped,
  fitted or straightened; positions kept to a tenth of a screen pixel). The two stock freehand pen types are
  hidden from the pen list; the shape pens (line, arrow, rectangle, circle) stay for when a shape is wanted. Pressure sensitivity is on, so a stylus gives a line of varying width. The ink is black until
  another colour is chosen; the choice is kept on this device.
- **One SVG file per drawing**, `assets/drawings/draw-<8 hex>.svg`, linked from the note as `![](relative/path)`.
  Editing it again overwrites the same file (written atomically), so the link never changes and no orphan is left.
  The file is an ordinary SVG: any viewer shows it, sync carries it like any asset, a conflict is a whole-file copy
  through the existing conflict mechanism. The white background is part of the file, so it reads on a dark theme.
- **Opening again for editing** loads the SVG back into the editor as strokes (js-draw reads what it wrote).
- **Only drawings are touched by the two commands** (`save_drawing`, `read_drawing`): the path has to be
  `assets/drawings/draw-<8 hex>.svg`; anything else is refused. Other assets keep their own rules (ST-8).
- **Input:** Pointer Events (mouse, touch, pen in one code path). Two fingers pan and zoom.
  The first time a stylus touches the canvas one finger starts to pan as well (remembered on the device), so a resting
  palm does not draw; before that a finger draws. A mouse draws with the primary button.
- **Where the overlay lives:** like the template questions, a promise-based `openDrawing(path | null)` opens a
  dialog mounted once in `App.svelte`; the editor and a click on a drawing in a block both use it.

## Tasks

### D1 Storage (`save_drawing`, `read_drawing`)

- `Graph::save_drawing(id, svg)` and `Graph::read_drawing(path)` in `crates/graph/src/drawings.rs`; the app commands
  of the same names return the path and the Markdown link relative to the page.
- **Verify:**
  - [x] a new drawing is stored under `assets/drawings/draw-<8 hex>.svg` and the reply has `![](…)` relative to the page
  - [x] saving again with the path overwrites the file; the path and the link stay the same
  - [x] a path outside `assets/drawings/`, one that is not `draw-<8 hex>.svg`, text that is not an SVG, and more than 25 MB
    are refused
  - [x] `read_drawing` gives back what was saved
  - [x] a drawing that a page links is not listed under Unused images

### D2 The overlay (`DrawingEditor.svelte`)

- js-draw in a full-screen dialog: pen, eraser, selection, colour, width, undo, redo, zoom, pan; **Done** saves and
  closes, **Cancel** (and Escape) closes and asks first when something changed.
- Pen settings: the `Handwriting` pen (`rawInk.ts`, every point kept), autocorrect off, stabilization off, pressure on, black ink; colour and width kept on this device.
- **Verify:**
  - [x] drawing with the mouse and pressing Done saves an SVG with a path
  - [x] a wobbly stroke held still at the end is saved with one piece per input point, not as a straight line (no autocorrect) and not as a few fitted curves (no smoothing)
  - [x] Cancel after drawing asks first; Cancel on an empty drawing closes at once and stores nothing
  - [x] the dialog is a labelled dialog and takes the focus; Escape does not reach the editor behind it

### D3 In the page

- `/draw` (Insert group) opens the overlay and inserts the link at the cursor; a drawing in a block is an image whose
  click opens the overlay on that file; the picture is refreshed after a save.
- **Verify:**
  - [x] `/draw`, a stroke, Done: the block text is `![](…/assets/drawings/draw-xxxxxxxx.svg)` and the file exists
  - [x] click the picture, add a stroke, Done: the same file, now different; the block text is unchanged
  - [x] the picture on the page is the new one after Done (not the cached old one)

### D3b Name, form and delete

- The editor's header has a **Name** and **Picture / Link**; a block that holds a drawing rewrites its own reference
  (`Block.svelte` provides a `DrawingHost`, `Inlines.svelte` finds it), so a new name or form changes the text and
  leaves the file alone. **Delete…** asks, removes the reference, and trashes the file when no other page links it.
- **Verify:**
  - [x] a new drawing with a name and "Link" is written as `[name](…)`, shown as a ✏️ link, and opens with its name and form
  - [x] renaming and switching to "Picture" rewrites the text (the target as it was written, `../` included) and leaves the file as it was
  - [x] without a name a picture is `![](…)` and a link is `[Drawing](…)`
  - [x] Delete asks (Keep it changes nothing); then the reference is gone and the file is gone
  - [x] a drawing another page links as well keeps its file, and a message says so

### D4 Documentation and tests in WebKit

- `docs/keymap.md`, `docs/file-format.md`, `docs/requirements.md`; the browser tests run in WebKit as well.
- **Verify:** `scripts/check.sh` and `scripts/check-webkit.sh tests/drawing.spec.ts` pass.

### D6 Make space (done)

Like OneNote's "Insert Space": a tool with a line that is dragged, a horizontal one up or down or a vertical one sideways
(the direction is taken from the first 6 pixels of the drag); everything that lies below (or right of) the
line moves with it, so there is room to draw (or to take a gap away again) without selecting by hand. (DR-12)

- A toolbar button "Make space". Pressing the pen, a finger or the mouse on the canvas sets the line at that height
  (a thin dashed line across the whole width, a handle in the middle); dragging moves everything whose top is below the
  line, by the distance dragged; releasing ends the drag. Dragging up closes a gap but never moves a stroke above the
  line (nothing is moved past the line, so strokes cannot be pushed over each other).
- One undo step per drag. Works in the infinite canvas, at any zoom. Text boxes and strokes both move; a stroke that
  crosses the line stays where it is (it is above the line by its top edge).
- Built as a js-draw tool: `BaseTool` for the pointer events, `AbstractComponent.transformBy(translation)` per element
  and `uniteCommands` for the single undo step. No change to the file format: the SVG is saved as before.
- **Verify:**
  - [x] unit test (`makeSpaceMath.test.ts`): the elements chosen for a line at y are exactly those whose top edge is at or below y
  - [x] browser test: draw three strokes, one above and one below a line, drag the line down 100 px; the lower stroke
        moved by 100 px (by its bounding box in the saved SVG), the upper one did not
  - [x] one undo restores both; redo moves again
  - [x] it works with a pen pointer as well as a mouse (Chromium and WebKit); dragging up stops at the line; picking a pen switches the tool off
  - [ ] it works after zooming and panning (the maths is in canvas units; not tested)
  - [ ] on a real tablet: a stylus drag moves the content smoothly (not run)

### D7 More in the editor (DR-13 to DR-24, built and tested in Chromium and WebKit)

- **Layout:** the header is one row (title, name, Picture/Link, pen presets, Cancel, Done); the help line moved under the
  canvas into a bar with the zoom, Pen only and a More menu (background, snap shapes, toolbar position, focus, copy and
  save). The js-draw toolbar is 40 px high (48 px with a coarse pointer, presets and bar buttons 44 px).
- **Drafts (DR-13):** `drawing/draft.ts`, IndexedDB `notatnik-drawing`/`drafts`, key `file:<path>` or `new:<page>`; written
  3 s after the first change, at once when the page is hidden; removed by Done, Discard and Delete.
- **Search (DR-14):** `crates/index/src/svgtext.rs` reads `<text>` of the drawings a page links while it is indexed
  (`Parsed.drawings`); `save_drawing` of an existing drawing indexes the pages that mention it again
  (`Index::files_mentioning`). Schema version 8.
- **Presets, Pen only, snap, zoom, background, focus (DR-15 to DR-21):** `drawing/presets.ts`, `prefs.ts`, `view.ts`;
  the lasso is js-draw's own mode of the selection tool (it was in the tool's menu), made the default once per device
  (`notatnik.drawing.lasso`). Snapping needs `RawInkBuilder.autocorrectShape`, which feeds js-draw's shape fitter with
  the same points; without the switch nothing calls it.
- **Export (DR-22):** `drawing/exportImage.ts`; `save_export` in `src-tauri` (desktop only: the save dialog and the write).
- **Copies of other devices (DR-23):** `drawing_copies` and `discard_drawing_copy` in the API; `read_drawing` reads a copy
  too. Strokes are compared by what they are made of (without the id), so a stroke both versions have is not doubled; what
  one side erased comes back.
- **Not done, on purpose:** a "fit the page to the drawing" action (the sheet already follows the drawing, also after
  Make space and erasing), dots or ruled lines as a background (js-draw has grid and plain only), locking a pasted
  picture against selection, a width for the picture in the note.
- **Verify:**
  - [x] unit tests (`presets.test.ts`, `draft.test.ts`, `view.test.ts`, `svgtext` in `notatnik-index`): presets, the draft
        saver (one write per burst, flush, clear), zoom steps, export names and sizes, the words of an SVG
  - [x] integration tests: a search finds the words of a drawing and not the old ones after it was saved again
        (`notatnik-index`, `notatnik-app`); a conflict copy is listed, read and trashed, and nothing else can be
  - [x] browser tests (`drawing-extras.spec.ts`, Chromium and WebKit): presets and storing a pen, Pen only (also
        against a stylus), snapping, zoom, background (three sizes, blank, reopened), focus and toolbar position, lasso,
        draft restore and discard, the three ways to deal with a copy, SVG and PNG download, a pasted picture
  - [ ] on a real iPad: the presets with a Pencil, a long press on a preset, Pen only with a finger and the Pencil, the
        toolbar below the canvas, a draft after the app was killed in the background (not run)
  - [ ] in the desktop app: Save as PNG and SVG open the system's dialog, Copy as a picture reaches another app
        (WebKitGTK may refuse the clipboard; then a message says so) (not run)

### D5 iPad (not started)

- The same UI as an installable web app (PWA) reached through the sync, tested with an Apple Pencil: the pen draws,
  a finger pans and zooms. Then the Tauri iOS app.
- **Verify (when built):** a drawing made on the iPad opens on the desktop and the reverse; the Pencil does not
  scroll the page while drawing.
