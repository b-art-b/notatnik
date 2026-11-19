# Diagrams with Excalidraw (X1 to X6)

**Goal:** draw diagrams (boxes, arrows, labelled shapes, flow charts, architecture sketches) in a note with
[Excalidraw](https://github.com/excalidraw/excalidraw), next to the handwriting editor (`/draw`, js-draw). A diagram is
an image in the note; a click opens it again as an editable Excalidraw scene.

**Requirements covered:** DG-1 to DG-10 (see `requirements.md`).
**Depends on:** drawings ([drawing.md](drawing.md): storage, overlay host, name/form/delete header, drafts, search of
`<text>`, conflict copies), assets (ST-8).
**Status:** X1 (storage), X2 (the Excalidraw island) and X3 (the overlay, with `/diagram` to reach it) and the copies part of X5 are built; the phone checks of X5 are waiting for a device, then X4 (checks of the page) and X6.

## Design

- **A second editor, not a replacement.** js-draw stays the editor of handwriting (DR-3: every pen point kept, no
  shape correction). `/diagram` opens Excalidraw. The two kinds of file are told apart by name, so a click knows which
  editor to open without reading the file.
- **One SVG file per diagram, the scene inside it.** Excalidraw's `exportToSvg` with `appState.exportEmbedScene: true`
  writes an ordinary SVG (any viewer shows it, the note shows it as an image) that carries the scene (elements, their
  order, the app state that matters) in the file. Opening a diagram reads the scene back from the SVG, so every shape is
  still a separate, editable object. The SVG is written with a white background so it reads on a dark theme, as
  drawings do (DR-5).
- **Where and how it is named:** `assets/drawings/diagram-<8 lower-case hex>.svg`, next to the drawings (the same
  trash, unused-images, sync and conflict rules apply), the 8 digits random as for drawings, overwritten in place when
  the diagram is edited again, so the link never changes. The reference in the note is the same as for a drawing:
  `![name](../assets/drawings/diagram-0f3a9c21.svg)` as a picture or `[name](…)` as a link.
- **Nothing leaves the computer.** Excalidraw loads its fonts and its shape libraries from the network unless told
  otherwise. The editor sets `window.EXCALIDRAW_ASSET_PATH` to fonts bundled with the app, hides "Browse libraries",
  collaboration, share link and the menu entries that talk to excalidraw.com, and works offline (a local-first app,
  P1). The libraries a user keeps are device data (`localStorage`), never in the graph.
- **React inside a Svelte app.** Excalidraw is a React component. `DiagramEditor.svelte` is a thin wrapper that
  `import()`s `react`, `react-dom/client` and `@excalidraw/excalidraw` **when a diagram is opened** (a separate chunk,
  not in the main bundle, not loaded for a note without one) and mounts it into a `<div>` with `createRoot`; closing
  the overlay unmounts it. Pin exact versions (at least two weeks old). The size of the chunk is checked (X6).
- **The overlay host is the one of drawings.** `openDrawing(path | null)` becomes `openDrawing(path | null, kind)`;
  the header (Name, Picture / Link, Delete…, Cancel, Done), the draft saver (DR-13) and the `DrawingHost` that rewrites a
  block's reference are reused. Only the canvas component differs.
- **Search.** Excalidraw's SVG export writes text as `<text>`, so the index reads it with the code of DR-14
  (`svgtext.rs`); a search for a word in a diagram finds the block that shows it. To be checked in X1: the embedded
  scene is not indexed as text (it sits in `<metadata>`, not in `<text>`).
- **Theme.** The editor follows the app's theme (Excalidraw `theme` prop); the saved SVG is always on white.
- **Conflicts (DR-23).** A diagram changed on two devices leaves a `diagram-<hex> (conflicted copy …).svg`. v1 offers
  the whole-file choice: keep this one, use the copy, or keep both (the copy stays beside). There is no merge inside a
  diagram (Excalidraw elements have ids and versions, so merging is possible later; not now).
- **Phone.** Touch and the on-screen keyboard are tested in the Android WebView. Excalidraw edits its text in a
  `<textarea>`, so the CodeMirror EditContext trap (D62) should not apply; the check "Enter in a bullet, then Search"
  is part of X5 all the same, because the overlay is mounted in the same web view.
- **Not in v1:** real-time collaboration, a library browser, exporting to excalidraw.com, importing `.excalidraw`
  files (the scene can be pasted into a diagram from the clipboard, which Excalidraw does itself), merging scenes,
  an iPad build.

## Tasks

### X1 Storage for diagrams

- `Graph::save_drawing`, `read_drawing`, `drawing_copies` and the trash and unused-image rules in
  `crates/graph/src/drawings.rs` accept `diagram-<8 hex>.svg` as well as `draw-<8 hex>.svg` (`is_drawing_path`,
  `drawing_of_copy`, the id generator takes a prefix). The commands stay `save_drawing`, `read_drawing` (the path says
  what it is); `save_drawing(None, …)` takes a `kind` (`draw` default, `diagram`).
- **Built:** `DrawingKind` (`draw`, `diagram`) in `crates/graph/src/drawings.rs`; `save_drawing_of(existing, svg, kind)`; the command
  `save_drawing` takes `kind` (absent = `draw`; an existing file keeps its name whatever `kind` says); `svgtext::drawing_paths`
  finds both names. Unused images and Delete use the generic asset rules and needed no change.
- **Verify:**
  - [x] a new diagram is stored as `assets/drawings/diagram-<8 hex>.svg` and the reply has `![](…)` relative to the page
  - [x] saving again with the path overwrites it; path and link stay the same
  - [x] a path outside `assets/drawings/`, a name that is neither `draw-` nor `diagram-`, text that is not an SVG, an SVG with
        a `<script>`, and more than 25 MB are refused (as for drawings)
  - [x] a diagram that a page links is not listed under Unused images; one that no page links is
  - [x] the words of the diagram's `<text>` are found by search, the embedded scene is not (`notatnik-index`)

### X2 Excalidraw as a lazy, offline island

- `ui/src/lib/diagram/excalidraw.ts` loads React and Excalidraw on first use; fonts and CSS are bundled
  (`EXCALIDRAW_ASSET_PATH` points at them); the UI options hide the library browser, the collaboration, share and
  excalidraw.com entries.
- **Built:** `ui/src/lib/diagram/excalidraw.ts` (`mountDiagram(container, {svg?, theme, onChange?})` returns
  `getSvg`, `isEmpty`, `changed`, `markSaved`, `unmount`); the build copies the fonts to `excalidraw/fonts/`
  (`excalidrawFiles` in `vite.config.ts`, the 13 MB CJK font Xiaolai is left out). Versions: `@excalidraw/excalidraw`
  0.18.1 (2026-04-20), `react` and `react-dom` 18.3.1, all exact. Tested on its own page of the Vite dev server
  (`tests/harness/diagram.html`, started by `tests/diagram-island.spec.ts`) because the overlay is X3.
- **Size (measured with a build of the harness page alone):** the island is a separate set of chunks, about 7.6 MB
  before and 2.5 MB after gzip (React 130 KB, Excalidraw 1.8 MB + 1.1 MB, CSS 140 KB, fonts 480 KB), loaded only when a
  diagram is opened. About half of it is a second copy of mermaid (11.17.2, Excalidraw's own dependency, next to the
  app's 11.4.1) with its diagram chunks; pinning both to one version would remove it (not done: it needs a check of the
  app's own mermaid blocks).
- **Verify:**
  - [x] the main bundle has no React and no Excalidraw (checked in the build output, again after X3 wired it in: the main chunk grew by about
        10 KB, React and Excalidraw are in chunks of their own); the harness page requests none of it until `mountDiagram` runs
  - [x] a browser test with the network blocked (`context.route('**/*', …)` for everything not from the app) opens the
        editor and draws a rectangle; no request leaves the app
  - [x] the editor shows no library browser, collaboration or share entries
  - [x] the scene → SVG → scene round trip (`exportToSvg` with the scene embedded, then `loadFromBlob`) gives back a shape that
        can be exported again; done in the browser test, not as a unit test (jsdom has no canvas or `FontFace`, which
        Excalidraw needs). Ids, text and positions are still to be compared in X3 with a drawing made by hand

### X3 The overlay

- `DiagramEditor.svelte` in the drawing overlay: Excalidraw full size, the header of D3b (Name, Picture / Link, Delete…,
  Cancel, Done), **Done** exports the SVG with the scene and stores it, **Cancel** and Escape ask first when something
  changed, drafts (DR-13) keyed `file:<path>` / `new:<page>`, the app's theme.
- **Built:** `DiagramEditor.svelte` (mounted by `App.svelte` when `app.drawing.kind === 'diagram'`); `DrawingOpen.kind`,
  `kindOfPath` and the wider `drawingPath` in `commands/drawing.ts`; `/diagram` in `slash.ts` and `completion.ts`; a click on
  a diagram picture or link opens it (`Inlines.svelte` passes `kindOfPath`); `DraftSaver` takes a function that may be
  async and `draftKey(target, kind)` keeps a new drawing's and a new diagram's drafts apart. `/diagram` and the click
  are part of X4 in the plan but were needed to test the overlay; X4 now only has to check the page side. The header, the
  save and the delete are the drawing editor's code copied and kept in step (a shared component is a later clean-up).
- **Verify** (`tests/diagram.spec.ts`, Chromium):
  - [x] a labelled dialog that takes the focus; Escape does not reach the editor behind it (as D2)
  - [x] a rectangle, an arrow and a text, Done: one SVG with the three shapes and an embedded scene
  - [x] Cancel after drawing asks first; Cancel on an empty diagram closes at once and stores nothing
  - [x] the draft of an unsaved diagram is offered again with Restore / Discard (DR-13)
  - [x] the dark theme shows a dark editor, and the stored picture is on white

### X4 In the page

- `/diagram` (Insert group; also found by `/flowchart`, `/excalidraw`, `/boxes`) opens the overlay and inserts the link at
  the cursor. A diagram in a block is an image whose click opens the overlay on that file; the picture is refreshed after
  a save; the rename, Picture / Link switch and Delete of D3b work on it.
- **Verify:**
  - [ ] `/diagram`, a shape, Done: the block text is `![](…/assets/drawings/diagram-xxxxxxxx.svg)` and the file exists
  - [ ] click the picture, move the shape, Done: the same file, now different; the block text is unchanged; the picture
        on the page is the new one
  - [ ] after the first save, opening it again shows separate, editable shapes (select one, change its text)
  - [ ] a drawing (`draw-…`) still opens in the handwriting editor, a diagram in Excalidraw
  - [ ] Delete asks; then the reference and the file are gone (and a diagram another page links keeps its file)

### X5 Copies, sync and the phone

- A conflict copy of a diagram is offered when it is opened: keep this one, use the copy, keep both (the copy stays as it
  is). On the phone: the editor works by touch (pan with two fingers, pinch to zoom, the text tool with the keyboard).
- **Built:** the banner of `DiagramEditor.svelte` (`drawing_copies`, `discard_drawing_copy` already covered diagram names
  after X1): **Keep this one** trashes the copy at once, **Use that version** loads the copy into the editor and trashes
  it when Done overwrites the file, **Keep both** leaves the copy as a file and stops asking in this window.
- **Verify:**
  - [x] a `diagram-<hex> (conflicted copy …).svg` is listed when the diagram is opened; each of the three choices does
        what it says (`tests/diagram.spec.ts`); nothing else can be read or trashed through these commands (an app test:
        the copy of a `draw-` file with the same digits is not listed, the diagram itself cannot be discarded)
  - [ ] a diagram made on the desktop opens, and can be changed, on the phone; the reverse
  - [ ] on the phone: after closing the editor, Enter in a bullet and then typing in Search works (D62)
  - [ ] on the phone: a text typed in a shape keeps the letters (no lost input)

### X6 Documentation, size and tests in WebKit

- `docs/keymap.md` (`/diagram`), `docs/file-format.md` (the diagram file, the embedded scene), `docs/requirements.md`
  (DG-1 to DG-10), `docs/architecture.md` (a decision row), `docs/TODO.md`; the size of the lazy chunk is written down
  here; the browser tests run in WebKit as well (`ui/tests/webkit.list`).
- **Verify:** `scripts/check.sh` and `scripts/check-webkit.sh tests/diagram.spec.ts` pass; the chunk size and the APK size
  increase are noted in this file.

## Risks to look at first

- **Bundle and APK size.** Excalidraw with React is a few MB before compression; it loads only when opened, but the
  APK and the desktop package carry it. Measure in X2 before the rest is built; if it is too much, the fallback is to
  load the editor from the app's own folder at the first use (still local) or to drop the phone.
- **React in the Svelte build.** Vite needs the React JSX transform only for the library itself (it ships compiled);
  our code stays Svelte and TypeScript, with no JSX.
- **WebKitGTK (the desktop app).** Excalidraw's clipboard and file-system calls may be refused there; the tests in
  WebKit show what works, and `save_export`-style desktop commands (DR-22) are the way out.
- **A scene stripped from the file** by an outside tool: the picture shows, but it is not editable. The editor says so
  and offers to start a new diagram from the picture (the SVG is added as an image element).
