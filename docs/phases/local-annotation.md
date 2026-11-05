# Annotating PDFs and EPUBs where they are on this computer (LV1 to LV4)

**Goal:** a link to a PDF or EPUB on this computer (`[Report](file:///other/nc/Rozrachunki%20do%20luty%202024_0001.pdf)`,
what `/file` → This computer → _Link to the file where it is_ writes) opens in the app's viewer and can be annotated,
**without copying the file into the graph**. The file is read where it is and never changed. The highlights are blocks of
the usual hidden sidecar page `hls/<name>`, which syncs with the graph; the file is recognised by its content, so the
highlights follow it when it is moved or renamed.

The system's own viewer stays one click away. A plain click on the link opens the app's viewer, and there are three
ways to open the system's program instead (see _Two ways to open_).

**Requirements covered:** LV-1 to LV-8 (see `requirements.md`); LN-12 (linked files) gets a viewer for `file:///` links.
**Depends on:** PDF and EPUB annotation (viewers, sidecar, `annotation_page`), and the Nextcloud phase
([nextcloud-annotation.md](nextcloud-annotation.md)), whose address-and-content-hash sidecar rule, `RemoteBar` and
cache-style asset route this phase reuses.
**Status:** LV1 to LV4 not started (plan written).

## Decisions

- **Only where a computer's files can be read:** the desktop app (Linux first; the same code serves Windows and macOS).
  A browser, the iPad and Android cannot read a path of a computer, so there a `file:///` link behaves as it does now
  (the system opens it or nothing does) and says in one line that the viewer needs the desktop app.
- **No copy, no cache.** The viewer reads the file in place. A button _Keep a copy in the graph_ (the one of the Nextcloud
  phase) is there for when you want the file to travel with the graph (up to 100 MB; the 25 MB limit of `attach_file` is
  for the picker's own copy and stays).
- **Whole file, up to 100 MB,** like the Nextcloud documents; a larger file opens in the system's program.
- **The content hash is the identity,** `file:` of the sidecar is a hint (the address as written). So moving the file or
  opening it from another computer's path finds the same sidecar; two different files with the same name get two.
- **A plain click opens the app's viewer; the system's program is a modifier, a small button and a setting** (below).
  The setting is per device, so the iPad and the desktop can differ.

## Design

- **Which links.** `file:` addresses with an empty host or `localhost`, decoded to a path (`file:///C:/Users/me/a.pdf` on
  Windows), that end in `.pdf` or `.epub` in any case. A host, a `..` segment that would be resolved away, a path with
  a NUL and anything that is not a regular file are refused. Everything else (a folder, another type, a web address)
  opens as today.
- **A command, not a free path.** `local_open(url)` checks the link, the file and its first bytes (`%PDF-` within the
  first 1024 bytes; `PK\x03\x04` for an EPUB, so a renamed file is not taken for a document), reads its size and
  returns `{ kind, name, size, address, path: "local/<id>" }`, where `<id>` is a random token that the state keeps for
  as long as the app runs and that maps to that one file. The viewers fetch `local/<id>` through the same asset route
  as everything else; the route serves nothing but a registered token. No URL can name a path of the computer, so a
  note cannot make the app serve `/etc/passwd`, and a registered file is read-only.
- **Only the desktop shell registers files.** `local_open` answers `unsupported` in the browser's dev server unless it
  was started with `--allow-local-files` (for the tests, like `--allow-local-fetch`), and on mobile. The UI never
  tries on a device that cannot: it checks the platform first and falls back to the old behaviour.
- **The sidecar.** `annotation_page` takes the address and the token path the way it does for a Nextcloud document: found
  by `file` (the link as written, normalised), else by the content hash whatever `file` says, else made, named after the
  file (`hls/<name>`). A `file` that is a path of the graph is not touched. `AnnotationPageArgs.remote` is renamed
  `origin` (it now names "where the document came from": an `http(s)` or a `file:` address); the Nextcloud code uses the
  new name.
- **Viewer targets.** A viewer target whose path is a `file:` address is resolved with `local_open` (the same place the
  Nextcloud viewers call `remote_open`), shows the file's name as the title, and its bar says _On this computer_ with
  _Keep a copy in the graph_ and _Open in the system's viewer_. A file that has gone says so and offers the system's
  program; the sidecar and its highlights stay.
- **Chips and references.** A `((id))` chip whose sidecar has a `file:` address opens the document the same way (the kind
  comes from the highlight, as for Nextcloud documents); the page name names it.
- **Privacy in the log.** Paths and names of local files are not logged above `debug`; `local_open` logs the size only
  (`docs/logging.md`).

## Two ways to open

| You want                             | Do                                                                                                                                                                               |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The app's viewer (the default)       | click the link                                                                                                                                                                   |
| The system's program, for one link   | **Ctrl+click** (Cmd+click on a Mac) or middle click; or the small **↗** after the link, which shows on hover and on keyboard focus and is always shown on a touch screen (44 px) |
| The system's program for every click | _Settings → Linked files → Open PDF and EPUB links in_ → _the system's program_ (the default is _this app_); then Ctrl+click and ↗ open the app's viewer                         |
| The system's program from the viewer | the viewer's _Open in the system's viewer_ (also for files in the graph and for Nextcloud documents; a Nextcloud document opens its copy, a file of the graph where it is)       |

The same two ways apply to PDFs and EPUBs in `assets/`, so a link to a copy in the graph works like one to a file in
place. A link that is not a PDF or an EPUB, or a PDF on another server, is not touched by any of this.

## Tasks

### LV1 The backend: `local_open`, the token route, the sidecar by origin

- `local_open`, the token map and the `local/<id>` route of `read_asset`/`asset_file` in `crates/app/src/state/local_docs.rs`;
  `file:` parsing in a pure function with a table test; `AnnotationPageArgs.remote` becomes `origin` and accepts
  `file:` too; `--allow-local-files` for the dev server.
- **Verify:**
  - [ ] a table of addresses: `file:///home/me/a.pdf`, `file://localhost/home/me/a.pdf`, spaces and non-ASCII percent
        decoded (`Rozrachunki%20do%20luty…`), `file:///C:/Users/me/a.pdf`, upper-case extension; refused: a host, `..`, an
        encoded `..`, a NUL, a folder, `.txt`, a PDF-named file that is not a PDF, an EPUB-named file that is not a zip
  - [ ] `local_open` of a real file gives its kind, name, size and a token; the route serves exactly its bytes; the
        same file twice gives a working token each time; a token of nothing, `local/../x`, `local//etc/passwd` and a path
        written instead of a token are refused
  - [ ] a file over 100 MB is refused before it is read; a file that is deleted after `local_open` gives not found, not a crash
  - [ ] the sidecar: made with the address, found again by the address, found by the hash after the file was moved or
        renamed (same page, `file` updated only when the old path is gone), a second file of the same name gets its own
        sidecar, and a sidecar whose `file` is a graph path is left as it is
  - [ ] the Nextcloud tests pass unchanged with the argument renamed
  - [ ] the dev server without `--allow-local-files`, and a mobile build, answer `unsupported`; the file is never read
  - [ ] nothing above `debug` in the log names a path or a file

### LV2 The viewers

- `PdfViewer` and `EpubViewer` resolve a `file:` target with `local_open`; `RemoteBar` becomes a bar for any document
  that is not in the graph (_From your Nextcloud_ / _On this computer_, _Keep a copy_, _Open in the system's viewer_); a
  missing file says so.
- **Verify:**
  - [ ] a PDF and an EPUB opened from a `file:///` link show their text, highlight, take a comment and a reference like
        files of the graph; nothing is written outside the sidecar; no file appears in the graph
  - [ ] the title is the file's name; the bar says _On this computer_
  - [ ] _Keep a copy in the graph_ copies the file into `assets/`, points the sidecar at it, and the old link finds the
        same sidecar
  - [ ] the file moved, the link edited to the new place: the highlights are there
  - [ ] the file deleted: the viewer says so, offers the system's program and keeps the sidecar

### LV3 Opening: the click, the modifier, the ↗, the setting

- `Inlines.svelte`: a `file:` PDF or EPUB link and an asset link are routed by one function (`openDocumentLink`) that reads
  the modifier and the setting; the ↗ is a real `<button>` after the link, with a label; _Settings → Linked files_ has the
  choice; the viewer's button calls `backend.openAsset` / `openExternal`; `/file` writes the same links as before.
- **Verify:**
  - [ ] a click opens the viewer; Ctrl+click, Cmd+click, middle click and the ↗ call the system's open with the link
        (`window.open` stubbed in the browser tests, `open_asset`/`openExternal` in the shell) and open no viewer
  - [ ] with the setting on _the system's program_ it is the other way round, and the setting survives a restart
  - [ ] the ↗ is reachable by Tab, has an accessible name, shows on focus and hover, is always shown and 44 px on a touch
        screen; a link that is no PDF/EPUB has none
  - [ ] on a browser and a phone a `file:///` link behaves as before and says the viewer needs the desktop app
  - [ ] a link in a table, a query result, an embed and the journal behaves the same (they all render through `Inlines`)

### LV4 Tests, documents, checks

- Playwright with the dev server started with `--allow-local-files` and PDFs and EPUBs written to a temp folder; Chromium
  and WebKit. Docs: `requirements.md` (LV rows), `keymap.md`, `file-format.md` (`file:` as a `file:///` address),
  `logging.md`, `testing.md`, `TODO.md`, `CLAUDE.md`.
- **Verify:**
  - [ ] the whole path: `/file` → This computer → _Link to the file where it is_ is replaced here by a note with a
        `file:///` link; click, highlight, comment, `((id))` and its chip, move the file, fix the link, the highlight stays
  - [ ] the default click, the modifier, the ↗ and the setting (LV3), by keyboard and by touch
  - [ ] `scripts/check.sh` and `scripts/check-webkit.sh` pass
  - [ ] by hand, in the real desktop app: the file from the question (`/other/nc/…pdf` with spaces and Polish letters), a
        file on a removable drive, a file that is unreadable, Ctrl+click opening the system's viewer

## Later (not in this phase, but not made impossible)

- **Follow the file while it is open** (a PDF that is rewritten by another program reloads the viewer).
- **Folders:** a link to a folder of PDFs as a reading list.
- **Other types** that a viewer could take (images, text).
- **Mobile:** the system's document picker gives a bookmark the app may read; the same `local_open` with a platform
  reader behind it.

## Open questions

- **The default of a plain click.** The plan makes it the app's viewer, as asked; the setting flips it. If the setting
  proves to be wanted by almost everyone the other way, only its default changes.
- **A file on a network or removable drive that is gone** when the link is clicked: the viewer says so. Whether to show the
  highlights anyway (from the sidecar, without the document) is left for later.
- **Windows and macOS paths** are handled in the parser (drive letters, `localhost`) and tested as strings; they are not run
  in this phase (the first target is Linux).
