# Fixtures

Test data shared by the Rust tests, the Node/WASM smoke test and (from phase 2) the mock backend.
Layout and rules: [docs/testing.md §2](../docs/testing.md#2-fixtures).

- `graphs/<name>/`: small hand-written graphs, each with a `README.md` saying what it covers.
  `expected-counts.json` (basic, namespaces, refs, tasks) and `expected-backlinks.json` (refs) were
  derived by hand, not generated. `queries/` also has `queries/*.query` + `expected/*.json`.
- `edits/<op>/<case>/{before.md, op.json, after.md}`: golden edit tests; a case directory ending in
  `-error` expects the op to be rejected. Regenerate `after.md` with `UPDATE_GOLDEN=1` and review the diff by hand.

## Coverage of the file format

Every syntax element of [docs/file-format.md](../docs/file-format.md) §3–§6 and where it is exercised.
Paths are below `graphs/`.

| Element (spec section) | Fixture files |
|---|---|
| Frontmatter, reserved keys, user keys (§3.1) | `basic/pages/work/projects/alpha.md`, `basic/pages/people/anna.md`, `refs/pages/source.md`, `queries/pages/work/projects/beta.md` |
| Invalid / unclosed frontmatter, only frontmatter, `---` as a rule (§3.1, §10) | `edge-cases/pages/bad-frontmatter.md`, `unclosed-frontmatter.md`, `only-frontmatter.md`, `hr-later.md` |
| Blocks and nesting by indentation (§3.2) | `basic/**`, `edge-cases/pages/tabs.md`, `four-space.md`, `mixed-indent.md`, `indent-jump.md`, `deep-nesting.md` |
| Preamble before the first bullet (§3.2) | `edge-cases/pages/text-between.md`, `hr-later.md` |
| Line endings, BOM, final newline, empty file (§3.2) | `edge-cases/pages/crlf.md`, `bom.md`, `no-trailing-newline.md`, `empty.md` |
| `*` / `+` bullets, numbered text, empty bullet (§10) | `edge-cases/pages/bullets.md` |
| Block properties `key:: value`, typed values, lists (§3.3) | `basic/pages/work/projects/alpha.md`, `queries/pages/reading/books.md`, `tasks/pages/dates.md`, `edge-cases/pages/bullets.md` |
| Block IDs `^id`, duplicates (§3.4) | `basic/pages/work/projects/alpha.md`, `refs/pages/source.md`, `edge-cases/pages/block-ids.md` |
| Page links, labelled links, journal links (§4) | `basic/pages/work/projects/alpha.md`, `refs/pages/other.md`, `edge-cases/pages/brackets.md` |
| Global names, namespaces, virtual parents, encoded file names (§2, §4) | `namespaces/**` |
| Tags, `#[[multi word]]`, tag boundaries (§4) | `edge-cases/pages/hash-cases.md`, `basic/pages/inbox.md`, `tasks/pages/work/tasks.md` |
| Block references and embeds `((id))`, `!((id))` (§4) | `refs/pages/linker.md`, `basic/pages/work/projects/alpha.md` |
| Page embeds `![[page]]` (§4) | `refs/pages/linker.md` |
| Aliases (§2, §3.1) | `refs/pages/source.md` (+ links in `linker.md`), `basic/pages/work/projects/alpha.md` |
| Highlights `==x==`, other Markdown (§4) | `edge-cases/pages/inline-misc.md` |
| Raw HTML stays text (§4) | `edge-cases/pages/html.md` |
| Code fences are opaque (§10) | `edge-cases/pages/fences.md`, `basic/pages/work/projects/alpha.md` (query block) |
| Task markers and near-misses (§5) | `tasks/pages/all-states.md` |
| `scheduled`, `deadline`, `priority`, `done`, dates and datetimes, bad dates (§5) | `tasks/pages/dates.md`, `queries/**` |
| `repeat` forms and `completed` log (§5) | `tasks/pages/repeats.md`, `basic/journals/2026/2026-10-03.md` |
| Query blocks and every query clause (§6) | `queries/queries/*.query` (43 queries), `basic/pages/work/projects/alpha.md` |
| Journals, one file per day (§1, §2) | `basic/journals/**`, `queries/journals/**` |
| Nextcloud conflict copies (§1, NC-5) | created inside tests (`graph.rs`, `index.rs`) |
| 100 KB lines, nesting beyond 100 levels | generated inside tests (`parse_fixtures.rs`) |

Not covered by files because they are not page content: calendar mapping (§7, phase 4) and `config.toml` (§8; unit tests in `notatnik-graph`).
