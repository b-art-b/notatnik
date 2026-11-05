# foliate-js (vendored)

[foliate-js](https://github.com/johnfactotum/foliate-js), MIT licence (`LICENSE`), the reader of the EPUB annotation
phase (`docs/phases/epub-annotation.md`).

- **Commit:** `78914aef4466eb960965702401634c2cb348e9b1` (2026-05-01).
- **Copied unmodified:** `view.js`, `epubcfi.js`, `progress.js`, `overlayer.js`, `text-walker.js`, `epub.js`,
  `paginator.js`, `fixed-layout.js`, `search.js`, `mobi.js`, `fb2.js`, `comic-book.js`, `tts.js`, `vendor/zip.js`,
  `vendor/fflate.js`.
- **Left out:** the demo reader (`reader.html`, `reader.js`, `ui/`), the tests, the build files, OPDS, the dictionary and
  quote-image modules, and `vendor/pdfjs/` (10 MB). `pdf.js` is **our stub** (it only throws), because the original
  imports the PDF.js files that are not here.
- **Only `ui/src/lib/epub/foliate.ts` imports from this folder.** Nothing else may.
- **Scripts in a book are not run because of the app's Content-Security-Policy** (`script-src 'self'`, no `blob:`), not
  because of foliate-js; see "Security" in its README and `docs/phases/epub-annotation.md`.

## Updating

1. Clone foliate-js, check out the new commit, and copy the same files over these (keep our `pdf.js`).
2. Change the commit above.
3. Run `pnpm test:e2e -- epub` in Chromium and `scripts/check-webkit.sh tests/epub.spec.ts` in WebKit. The tests include
   a highlight with a wrong CFI that must be repaired, a hostile book, and a 60-chapter book.
