# Annotating documents that live in Nextcloud (NX1 to NX4)

**Goal:** open a PDF or EPUB that is in the account of the graph's sync server (Nextcloud) in the app's viewer and
annotate it, **without putting the file in the graph**. A link to the file (`[Report](https://cloud.example/f/12345)`, the
kind `/file` already writes) opens the PDF viewer or the EPUB reader. The app downloads the file with the sync login into a
cache on the device, shows it, and keeps the highlights and comments as blocks of the usual hidden sidecar page
`hls/<name>`, which syncs with the graph. A button _Keep a copy in the graph_ turns it into an ordinary asset.

**Requirements covered:** NA-1 to NA-8 (see `requirements.md`); LN-12 (linked files) gets a second way to open a link.
**Depends on:** PDF and EPUB annotation (viewers, sidecar, `annotation_page`), the file sources and the graph's own server
(`filesources.rs`, `GRAPH_SOURCE`), the WebDAV client of `notatnik-sync`, the web phase (the sidecar follows a file that
changed, W2).
**Status:** NX1 to NX4 not started (proposal accepted; plan written).

## Decisions

- **Open in place, with a cache; a copy only on request.** Nothing of the document is stored in the graph (ST-2: nothing
  in the graph that is not meant to be synced). The cache is on the device and can be thrown away at any time.
- **Only the graph's own sync server.** The login is the one the sync already uses (`GRAPH_SOURCE`, the password in the
  keyring). Other servers of the file-source list keep opening in the browser. No new setting, no new password.
- **The whole file is downloaded, up to 100 MB.** No range requests in this phase (pdf.js and foliate get a complete
  local file). A larger file is not downloaded; the message offers _Open in Nextcloud_ (the browser).
- **The document is never changed or uploaded.** Only the sidecar is written, and it is in the graph.

## Design

- **Which links.** A web address of the graph's server in one of two forms, matched against the server of the sync
  address: `<server>/f/<id>` (what `/file` writes and what pasting a Nextcloud Files address makes, ED-27) and
  `<server>/remote.php/dav/files/<user>/<path>`. Anything else keeps opening in the browser.
- **Resolve before opening.** `remote_resolve(url)` answers `{ id, name, kind: pdf | epub | other, size, etag }`.
  - `<id>` becomes a path with a WebDAV `SEARCH` on `oc:fileid` (Nextcloud supports it); a path address gets its
    `oc:fileid` with a `PROPFIND`. The id is the identity from then on, so a renamed or moved file is still found.
  - `kind` comes from the file name that resolution returns, not from the link text. `other` (and a name with another
    extension) opens in the browser as today, so the click does not take over a Word file.
  - A server that cannot search by id: the picker writes the path form too (see NX3), and the click of an id-only link
    says so. **This is the main risk;** NX1 starts by testing it against a real Nextcloud.
- **Fetch into a cache.** `remote_document(url)` downloads to `<data dir>/cache/remote/<graph id>/<fileid>/<etag>.<ext>`
  (a temp name first, then a rename, so a stopped download leaves no half file). On each open it asks the server for the
  ETag and downloads only when it changed; without a connection the newest cached copy opens, with a note
  (_Opened from the copy of 3 Nov_). No redirect leaves the server (the login is never sent to another host). The cache
  is cleaned least-recently-used above 500 MB and by _Settings → … → Clear_.
- **How the viewer gets the bytes.** The viewers read a document through `backend.assetUrl(path)`. A cached file is served
  by the same route under a `cache/` prefix that only reaches `cache/remote/` (the path is checked, no `..`, no
  symlinks), in the dev server and in the Tauri asset protocol. Nothing is read through the webview's network.
- **A document target.** The viewers take `{ kind: 'graph', path } | { kind: 'remote', url, id, name }` where they took a
  path. Only the place the bytes come from differs; pdf.js, foliate, highlights and the panel are unchanged.
- **The sidecar of a remote file.** Frontmatter `file:` is the canonical address `<server>/f/<id>` and `sha:` the hash of
  the cached bytes, as for any file. `annotation_page` finds the sidecar by `file`, else by `sha`; a file that changed on
  the server gets its `sha` rewritten and its highlights found again by quote and context (W2, PA-4/EA-4). **The content
  hash is the identity, `file:` is a hint:** a sidecar whose `file:` is a path of the graph (after _Keep a copy_) is also
  found from the remote link when the bytes match, and its `file:` is not flipped back.
- **Keep a copy in the graph.** Copies the cached file to `assets/YYYY/MM/<name>-<hash8>.<ext>` (the existing
  attach rules, 25 MB limit lifted to the 100 MB of this phase) and sets the sidecar's `file:` to that path. Links in notes
  are not rewritten; they keep working either way.
- **References.** `((id))` of a highlight in a remote document shows the quote and a chip with the document's name; the
  chip opens the document at the highlight through the sidecar's `file:` (remote or local).
- **Privacy in the log.** Addresses, paths and names of remote files are not logged above `debug`; `remote_document` logs
  the file id, the size and whether the cache was used (`docs/logging.md`).
- **iPad and desktop.** The download is a Rust request with the keyring password, so it works the same on both and is not
  subject to the webview's network rules.

## Tasks

### NX1 The backend: resolve, fetch, cache

- `remote_resolve`, `remote_document`, `remote_cache_clear` in `crates/app/src/state/remote_docs.rs`; the WebDAV pieces
  (`SEARCH` by id, `PROPFIND` for id and ETag, a conditional `GET`, streaming to disk) in `notatnik-sync`'s client next to
  the existing calls.
- **Verify:**
  - [ ] `SEARCH` by file id is tried against a real Nextcloud (by hand, written down here); the fake server
        (`notatnik-fakedav`) answers it in the tests
  - [ ] a link of either form resolves to the same id; a renamed or moved file is found by its id; `other` is reported
        for a file that is not a PDF or an EPUB
  - [ ] the first open downloads, the second sends `If-None-Match` and downloads nothing; a changed ETag downloads again
        and the old copy is replaced
  - [ ] offline (server stopped) the cached copy opens and the reply says so; with no copy the error says the server cannot
        be reached
  - [ ] a download over 100 MB is refused before the body is read (`Content-Length`) and also when it grows past the
        limit while streaming; a stopped download leaves no partial file
  - [ ] only the graph's server is accepted; a redirect to another host is refused and the password is not sent there
  - [ ] a wrong password, a 404 and a 403 give their reasons; none logs an address above `debug`
  - [ ] the cache is cleaned above 500 MB, oldest first, and never the file being read; clearing it works

### NX2 Document targets in the viewers and the sidecar

- `PdfViewer` and `EpubViewer` take a document target; `annotation_page` accepts a remote address; the asset route serves
  `cache/remote/`.
- **Verify:**
  - [ ] a remote PDF and a remote EPUB open, highlight, comment and reference exactly like local ones (the existing
        specs run with a remote target as well)
  - [ ] the sidecar has `file: <server>/f/<id>` and a `sha`; the same document opened again finds it
  - [ ] the file changed on the server (a paragraph added, the highlighted sentence kept): the highlight is drawn on the
        same words; a changed sentence is listed as not found
  - [ ] the asset route refuses `..`, a path outside `cache/remote/`, and a symlink
  - [ ] the viewers never write the document: no request but `GET` and the conditional check leaves the app

### NX3 Entry points

- A click on a link of the graph's server resolves first and opens the viewer (a short busy state in the link; _Open in
  Nextcloud_ stays one click away); `/file` on the graph's server gets _Open for annotation_ beside _Link_; the viewers
  get _Keep a copy in the graph_ and show _Opened from the copy of …_ when offline; chips open remote documents.
- **Verify:**
  - [ ] clicking `[Report](…/f/<id>)` to a PDF opens the viewer; to a `.docx` it opens the browser as before
  - [ ] _Open for annotation_ in the picker writes the link and opens the viewer
  - [ ] _Keep a copy_ puts the file under `assets/…`, points the sidecar at it, keeps the highlights, and the old link
        still opens the same sidecar (not a second one)
  - [ ] a `((id))` chip of a remote document opens it at the highlight, also on a second device that has the same login
  - [ ] a link of another server, `file:///` and a plain web address keep opening outside
  - [ ] keyboard only and by touch (44 px targets); Esc closes the viewer

### NX4 Tests with a fake server, documents, checks

- Playwright with `notatnik-fakedav` (PDF and EPUB served from its tree, ETags that change, a stopped server, a wrong
  password) in Chromium and WebKit; docs: `requirements.md` (NA rows), `file-format.md` (`file:` of a remote document),
  `keymap.md`, `logging.md`, `testing.md`, `TODO.md`, `CLAUDE.md`.
- **Verify:**
  - [ ] the whole path: the link in a note, the viewer, a highlight, a comment, a reference and its chip, the file changed on
        the server, offline, _Keep a copy_
  - [ ] a graph that is not synced, or a link of another server, shows the browser fallback, never a login prompt
  - [ ] `scripts/check.sh` and `scripts/check-webkit.sh` pass
  - [ ] by hand: a real Nextcloud, a 60 MB PDF, a renamed file, the iPad (the Rust download, the cache across restarts)

## Later (not in this phase, but not made impossible)

- **Range requests** for very large PDFs (pdf.js can read pages on demand), so page 300 opens without the whole file.
- **Other servers** of the file-source list (the login already exists for them), and other WebDAV servers without file ids
  (opened by path).
- **Nextcloud's "Open with"** (a small server app or a registered link scheme) to start the viewer from the web interface;
  with IN-1 and the share target.
- **Other file types** (images, Office files converted on the server).
- **Write-back** of a flattened or annotated PDF to the server, as an explicit action.

## Open questions

- **Search by file id on all servers.** Nextcloud answers `SEARCH` for `oc:fileid`; some installations restrict search or
  use another backend. NX1 decides whether the path form has to be written next to the id.
- **Where the cache lives on iOS/Android** (it must not be backed up or synced; the data directory of the app is the
  default).
- **Two accounts on one server.** The sync login is one account; a link to a file only another account can see fails with
  a 404/403 and says so.
