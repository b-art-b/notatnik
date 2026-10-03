# Fixture graph: edge-cases

Parser torture tests, one file per case; expected behaviour is stated in
`docs/file-format.md` §10. Files with special bytes: `crlf.md` (CRLF), `bom.md` (UTF-8 BOM),
`no-trailing-newline.md`, `empty.md`, `tabs.md`. A 100 KB line is generated inside the tests
(too big to keep readable in the repo); `deep-nesting.md` has 50 levels.
