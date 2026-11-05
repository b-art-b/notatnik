# Annotating web pages (W1 to W6)

**Goal:** keep a web page in the graph and annotate it. The app fetches the page, cuts out the article (Readability),
cleans it, and stores it as a **one-chapter EPUB** in `assets/web/` with the address and date in its metadata. From
then on it is a book: the EPUB reader, the highlights (CFI ranges in the hidden sidecar `hls/<name>`), the panel, the
comments and the `((id))` references of [epub-annotation.md](epub-annotation.md) work on it without a change. A saved
copy does not break when the site changes or disappears, syncs like any asset, and is read with the reader's themes and
text size.

**Requirements covered:** WA-1 to WA-10 (see `requirements.md`); IN-1 (web clipper) gets its first half.
**Depends on:** the EPUB annotation phase (reader, sidecar, panel), assets (ST-8), the link-title fetch (`linktitle.rs`,
ED-27) for the HTTP client.
**Status:** W1 to W4 done (the crate, the commands, the dialog, the reader; tested). W5 and W6 not started.

## Design

- **A clip is an EPUB.** `assets/web/<slug>-<hash8>.epub`: `<slug>` from the page's title, `<hash8>` the first 8 hex of
  the blake3 of the address (not of the content), so **clipping the same address again overwrites the same file** and
  the link in the notes, the sidecar and the highlights stay where they are. Highlights are found again in the new
  text by their quote (the machinery of E4: CFI first, then the quote and context in the chapter).
- **The EPUB is plain EPUB 3** that any reader opens: `mimetype` stored first, `META-INF/container.xml`, `content.opf`,
  `nav.xhtml`, one `chapter.xhtml`, the pictures below `images/`. Metadata: `dc:title`, `dc:creator` (the byline, if
  found), `dc:language`, `dc:source` (the final address after redirects), `dc:date` (the page's own date if found),
  `dcterms:modified` and a `meta` with the time of the clip. The chapter starts with the title, the byline and a line
  _Clipped from <address> on <date>_. The zip has no timestamps (a fixed date in every entry), so the same input gives the
  same bytes.
- **Where the work is done: in Rust**, in a new crate `notatnik-clip` (no UI, no database), so it is tested with plain
  `cargo test` against fixture pages, runs on the iPad (the webview's network rules do not apply to a Rust request),
  and can be used by the CLI later. Its parts:
  - **fetch:** the existing HTTP client (`ureq`), `http` and `https` only, no login in the address, no cookies, no
    referrer, a fixed user agent, redirects limited to 5, 15 s for the page and 10 s for a picture, the page at most
    5 MB, the character set from the header or the `<meta charset>` (`encoding_rs`).
  - **extract:** Readability. The first choice is a Rust port ([`dom_smoothie`](https://crates.io/crates/dom_smoothie),
    MIT, checked in W1 on the fixture pages); the fallback is Mozilla's `Readability.js` (Apache-2.0) run on the HTML in
    the webview, which would make the fetch-and-clean a round trip between Rust and the UI. If no article is found, the
    cleaned `<body>` is used and the clip says so.
  - **clean:** an allow-list (`ammonia`, MIT/Apache): headings, paragraphs, lists, quotes, code, tables, `a`, `img`,
    `figure`, `em`, `strong` and the like. No `script`, `style`, `iframe`, `form`, `object`, no `on…` attribute, no
    `javascript:` or `data:` address; links become absolute, `rel="noopener noreferrer"`.
  - **pictures:** the `img` addresses of the article are fetched (at most 40 pictures, 5 MB each, 25 MB together),
    sniffed by their bytes (png, jpeg, gif, webp; **no svg**), stored in the book, and the `src` rewritten. A picture
    that fails is dropped, its `alt` text stays.
  - **epub:** written with the `zip` crate the graph already uses.
- **A clip must not become a way to reach into the person's network.** The address comes from the person, but the
  picture addresses and the redirects come from a stranger's page. So **every address is resolved first and refused if
  it is loopback, private, link-local, unspecified, multicast or a cloud metadata address** (`127.0.0.0/8`, `10/8`,
  `172.16/12`, `192.168/16`, `169.254/16`, `::1`, `fc00::/7`, `fe80::/10`, IPv4-mapped forms), also after every
  redirect, and the connection is made to the address that was checked. Tests, and only tests, can switch this off
  (`ClipPolicy { allow_private: true }`, reached through the dev server's `--allow-local-fetch`).
- **Nothing the page contains runs.** The book is shown by the EPUB reader under the app's content policy (scripts
  blocked, remote resources blocked); the cleaning above is a second layer, not the only one.
- **Privacy in the log.** The address of a clipped page is the person's reading and is not logged above `debug`
  (`docs/logging.md` gets a line); errors name the host only.
- **Where the original address shows.** The reader shows an _Original page_ button when the book has an `http(s)`
  `dc:source` (it opens in the system's browser through `backend.openExternal`). The block that holds the link gets a
  property `source:: <address>`, so the address can be queried (`{{query (property source …)}}`) and shown.
- **How a person clips:** `/clip` in a block (and _Clip a web page…_ in the palette) asks for the address (pre-filled
  from the clipboard if it holds one), shows progress with a Cancel button, and writes
  `[Title](../assets/web/…epub)` with `source:: <address>` below it, where the command was typed.
- **Not in this phase:** pages that build their content with scripts (a plain fetch gets an empty shell: the clip says
  so), pages behind a login or paywall, a browser extension, a share target, clipping a selection or the clipboard's
  HTML, keeping the page's own look (a saved single-file HTML and its own viewer), clipping a PDF (it is simply
  copied as a file with `/file`), automatic re-clipping.

## Tasks

### W1 The clipping crate (`notatnik-clip`) (done)

- `crates/clip`: `fetch` (policy, limits, redirects, charset), `extract`, `clean`, `images`, `epub`, and
  `clip(url, &ClipPolicy) -> Result<Clip>` with `Clip { title, byline, source, date, language, epub: Vec<u8>, notes }`
  (`notes`: what the person should know, e.g. _no article found, the whole page was used_, _3 pictures were left out_).
  Fixture pages in `fixtures/web/` (an article with pictures and a byline, a blog with navigation and comments, a page in
  ISO-8859-2, one with a `<base>` and relative links, one without an article, a shell that needs scripts, a hostile one),
  served by a tiny `std::net` HTTP server in the tests.
- **Verify:**
  - [x] the article fixture gives its title, byline, date, the article text without the navigation, footer and comments,
        and its pictures inside the book; the book is a valid EPUB 3 (container, package, navigation, chapter; the
        `mimetype` entry first and stored) that foliate-js opens (a clip was opened in the reader by hand, text, title and picture shown; the automated check is W5) and a unit test re-reads with the `zip`
        crate
  - [x] the same input twice gives byte-identical output; a different time of the clip changes only the `meta` for it
  - [x] relative links and pictures are made absolute against the page and its `<base>`; links get `rel="noopener noreferrer"`
  - [x] the cleaned chapter has no `script`, `style`, `iframe`, `form`, `object`, no `on…` attribute, no `javascript:` or
        `data:` address (a hostile fixture with each of them; the output is parsed again and searched)
  - [x] a page in ISO-8859-2 comes out in correct UTF-8
  - [x] a page without an article gives its cleaned body and a note; a shell that needs scripts is reported as such
  - [x] pictures: the caps (40, 5 MB, 25 MB) are enforced; a picture of the wrong type, a 404 and an svg are dropped and
        keep their `alt`; the type comes from the bytes, not from the header
  - [x] **refused addresses:** loopback, private, link-local, `169.254.169.254`, `::1`, an IPv4-mapped IPv6, a name that
        resolves to one of them, and a redirect to one: the page and the pictures are refused (a test for each, with
        `allow_private: false`); `file:`, `ftp:` and an address with a login are refused; a page of more than 5 MB and a
        response that is not HTML are refused with a message that names the reason
  - [x] the timeouts and the redirect limit hold (a server that never answers, a redirect loop)
  - [x] clippy `-D warnings` and fmt clean; the crate does not depend on `notatnik-app` or the UI

### How W1 was built, where it differs from the plan

- **Readability:** `dom_smoothie` (MIT) did the job on every fixture, so Mozilla's `Readability.js` was not needed. Like
  Readability.js it keeps the site name in a title of four words or fewer (`Notatki o herbacie | Blog`) and cuts it off a
  longer one. It only gives up on a page of more than 30 000 elements; that is the only way into the whole-page
  fallback (a test builds such a page).
- **DNS rebinding is closed by construction:** the crate gives `ureq` its own `Resolver` (`net::Guard`) that resolves the
  name, refuses it when any answer is not public, and returns only the addresses it checked, so the connection goes
  where the check looked. An address written as a number (also `2130706433`, `0x7f.0.0.1`, `[::ffff:127.0.0.1]`) is
  checked before any request is made. Redirects are followed by hand, each one checked the same way.
- **Test hooks in `ClipPolicy`** (never set by the app): `allow_private`, `pretend_public` (a name that stands for a
  public one and resolves to this computer, to test a redirect from a "public" site to a private address) and
  shorter `page_timeout`/`picture_timeout`.
- **XHTML:** the cleaned article is written by a small serializer (`xhtml.rs`), because an EPUB chapter is XML and the
  HTML serializer writes `<br>`; the tests parse every XML file of the book with `quick-xml`.
- **Fixtures** are in `fixtures/web/` (article, blog in Polish, a page in ISO-8859-2, a page with `<base>`, a list of
  links, a script-only shell, a hostile page); `CLIP_DUMP=<dir> cargo test -p notatnik-clip` keeps the book of the article
  test, to open in a reader.
- **Found on the way:** the EPUB reader's content policy lacked `connect-src blob:`, which made it lose the style
  sheets of real books (fixed, with a test; see `epub-annotation.md`).

### W2 The command (`clip_page`) and the sidecar (done)

- `clip_page(url, page) -> { path, title, markdown, source, notes }` in `notatnik-app`: runs the clip on a blocking
  thread, reports where it is (`clip_progress`, which the dialog polls: fetching, cleaning, pictures n of m, writing), can be
  cancelled (`clip_cancel`), writes `assets/web/<slug>-<hash8>.epub` atomically through the graph (an existing file is replaced),
  and returns the Markdown that links it from `page` (relative path, written in `<…>` if the name needs it).
- `annotation_page` on a file it already knows (found by `file`) refreshes the `sha` property when the bytes changed
  (a clip clipped again), with a guarded write.
- The dev server takes `--allow-local-fetch` and passes the policy.
- **Verify:**
  - [x] a fixture page is clipped into `assets/web/…epub` and the reply has the link relative to the page; the same address
        again overwrites the same file and returns the same path
  - [x] two addresses with the same title get two files
  - [x] `clip_progress` answers in order (fetching, cleaning, pictures, writing); `clip_cancel` stops a slow clip and leaves no file and no temp file
  - [x] only a path below `assets/web/` is ever written; a title with `/`, `..`, `:` or emoji gives a safe file name
  - [x] after a re-clip the sidecar of the book (found by `file`) has the new `sha`, its highlights are untouched on
        disk
  - [x] the address of the page is not in the log at `info`

### How W2 was built, where it differs from the plan

- **Progress is asked for, not pushed:** `clip_progress` returns `{ running, step, done, total }` and the dialog polls it.
  Events would have needed a new channel in both transports for one use; a poll works unchanged in the browser, the
  desktop app and the iPad. `clip_cancel` ends the call within 50 ms (found in W3: the fetch runs on a thread of its own and `clip_page`
  waits for it looking at the flag, so a site that holds the connection open does not make Cancel wait for the 15 s
  timeout; the thread ends by itself and stores nothing), and the flag is checked once more before the book is stored,
  so a cancel never leaves a file.
- **A clip does not hold the graph.** The fetch runs outside the graph lock, so every other command keeps working during
  a slow site; only the write takes the lock. One clip at a time per graph (a second `clip_page` answers `conflict`).
- **The Tauri command is `async` now.** A plain Tauri command runs on the thread that draws the window, so a slow clip
  (and a slow sync before it) would have frozen the app and `clip_cancel` could not have got through.
  `#[tauri::command(async)]` runs `invoke_api` on a worker thread; the state has its own locks, which the dev server
  (one thread per request) already relies on.
- **The name stays when the title changes:** the file is found by the `-<hash8>` of the address, so a new headline does not
  make a second file; `clip_existing(url)` returns the book of an address (the dialog uses it to ask _clip again?_).
- **The sidecar follows a clip clipped again:** `annotation_page` on a file it knows finds the sidecar by `file` and
  rewrites its `sha` line when the bytes changed (a guarded write, one line, the highlights untouched).
- **The log:** a test captures every log line of a clip. The app's lines and everything at `info` and above are free of
  the address; the HTTP library's own `debug` line names it (off unless `RUST_LOG=debug`), as `docs/logging.md` now says.
- **Errors:** `invalid` for an address that is refused or not fetchable, `cancelled`, `conflict`, `no_graph`, and
  `clip_failed` for the rest; every message names the host at most.

### W3 The UI: `/clip`

- A slash command and a palette command; `ClipDialog.svelte` (address field filled from the clipboard when it holds a web
  address, _Clip_, progress, _Cancel_, the notes of the result, errors in words); the result is written as the link
  line with `source:: <address>` below it, in one undo step; an address that was clipped before asks _Clip it again
  (replaces the saved copy; your highlights stay)_.
- **Verify:**
  - [x] `/clip`, an address, _Clip_ writes `[Title](../assets/web/…epub)` and `source:: …` where the command was typed;
        the link opens the reader on the clip
  - [x] the dialog shows progress and Cancel works; an unreachable site, a 404, a non-HTML address and a refused
        address show their reasons and write nothing
  - [x] clipping the same address again asks first and replaces the file under the same name (that the highlights are
        drawn again on the new text is checked with the whole path in W5)
  - [x] the address field takes a web address from the clipboard on open
  - [x] Escape closes the dialog without clipping; the dialog is usable with a keyboard only and by touch (44 px targets)

### How W3 was built, where it differs from the plan

- **One dialog, two ways in.** `openClip` / `finishClip` (`ui/src/lib/commands/clip.ts`) work like the drawing editor's
  `openDrawing`: `/clip` removes the command, opens the dialog and writes `[Title](…)` and `source:: <address>` with the
  editor's `insertLinks` (one undo step); the block stays in edit mode meanwhile. The palette command _Clip a web page…_
  adds a new last block to the open page with the same two lines.
- **Notes before the link.** A clip without notes inserts at once; one with notes (no article found, pictures left out)
  shows them and waits for _Insert the link_, so they are read.
- **After Cancel or an error the focus goes back to the field**, otherwise the button that was pressed is gone and Esc
  reaches nothing (found by the test).
- **Tests:** `clip.spec.ts` (Chrome and WebKit; the two clipboard tests only in Chrome, the driver grants clipboard
  rights there only), `clip-refused.spec.ts` (without `--allow-local-fetch`), `clip-touch.spec.ts` (44 px), a page server
  in `web-fixture.ts`; `fixtures.ts` has the option `localFetch`.
- **The clipboard** is read once when the dialog opens, and only a lone `http(s)` address without a login in it is taken.
  A browser or webview that refuses the read leaves the field empty; the address is typed or pasted (Ctrl+V).

### W4 The reader shows where a clip came from

- `EpubViewer.svelte`: an _Original page_ button when the book has an `http(s)` source (through `openExternal`); the
  title bar shows the host. Styling for clips (pictures no wider than the page, long code and tables scroll).
- **Verify:**
  - [x] a clip shows _Original page_; it calls `openExternal` with the source address; a book without a source, or with a
        source that is not `http(s)`, shows no button
  - [x] a clipped picture is shown, not wider than the page, in paginated and scrolled layout

### How W4 was built, where it differs from the plan

- **`webSourceOf(book)`** (`ui/src/lib/epub/foliate.ts`) reads `dc:source` and takes the first `http(s)` address; `urn:`,
  `file:` and `javascript:` sources give no button, so a book cannot make the reader offer anything else.
- **Header:** the host (`example.org`, without `www.`; the full address as tooltip) and _Original page_ sit after the title;
  on a phone-width window the host is hidden. The button calls `backend.openExternal`.
- **Styling is not only for clips:** the reader's own style rules (`!important`, so a book cannot undo them) now keep
  pictures, SVG and video within the page width and make `pre` and tables scroll sideways. A test with a 3000 px picture
  fails without the rule.

### W5 Tests with a real page server

- Playwright starts a small Node HTTP server for the fixtures (pages, pictures, redirects, a slow page) and the dev
  server runs with `--allow-local-fetch`. A second spec runs **without** it and checks that a clip of the same local
  address is refused.
- **Verify:**
  - [ ] the whole path: `/clip` an article, open it, select a sentence, highlight it, comment on it, reference it with
        `((id))`, click the chip: the clip opens at the highlight
  - [ ] clip again after the page changed (a paragraph added above, the highlighted sentence kept): the highlight is
        drawn on the same words; if the sentence was changed, it is listed as not found
  - [ ] the hostile page: nothing runs when the clip is read, no request leaves for its remote pictures and styles
  - [ ] without `--allow-local-fetch`, `http://127.0.0.1:<port>/…` is refused with a message
  - [ ] Chromium and WebKit

### W6 Documents, iPad and checks

- Documents: `requirements.md` (WA rows, IN-1), `file-format.md` (the clip EPUB and `source::`), `keymap.md` (`/clip`, the
  dialog, _Original page_), `logging.md`, `testing.md`, `TODO.md`, `CLAUDE.md` (the doc list).
- **Verify:**
  - [ ] `scripts/check.sh` and `scripts/check-webkit.sh` pass with the new crate and specs
  - [ ] ten real pages (a news article, a long blog post, a Wikipedia article, a page with code, a page with a table,
        a page in a non-Latin script, one with many pictures, one that needs scripts, a paywalled one, a PDF address) are
        clipped; the result is judged by reading it; what fails is written down here
  - [ ] on a real iPad: the clip works from the app (the Rust request, not the webview's), and the clip reads and
        highlights like any book (stays open until done)
  - [ ] the desktop app: a clip works and the book opens under the app's policy

## Later (not in this phase, but not made impossible)

- **A browser extension** (IN-1): clips the page _as the browser sees it_ (logged in, scripts run) and sends the HTML to
  the app, which runs the same clean and EPUB steps; it also solves pages that need scripts and paywalls the person
  can read.
- **Clip from the clipboard or a selection** (HTML on the clipboard): the same steps without a fetch, so it works for
  anything the person can see and select, with no extension.
- **A share target** on Android and iOS (IN-1, AN-2).
- **Keeping the page's look**: a single-file HTML copy and its own viewer, annotated with quote selectors.
- **Re-clipping on a schedule**, a diff of what changed, and a list of clips whose source answers 404.
- **Highlights for a page that is still live**, shown on the page in the browser.

## Open questions

- **Which Readability.** Decided in W1: `dom_smoothie`.

- **File size of a clip.** Pictures dominate. The caps above are a start; the ten real pages show whether they are
  right.
- **Where `source::` goes** if the link is not at the start of the block (it is a property of the block, so it applies to
  the whole block); `/clip` always starts a block of its own.
- **Copyright and terms of use.** The saved copy is for the person's own notes and is not published; the app makes no
  claim about what a site allows, and the doc for users says so in one line.
