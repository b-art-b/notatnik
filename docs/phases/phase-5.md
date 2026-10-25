# Phase 5: V1 polish

**Goal:** close the remaining V1 gaps: a proper conflict-merge tool, multiple graphs, assets,
aliases and unlinked references, full namespace operations, templates, and editor comfort features.
After this phase, the desktop app is "V1 complete".

**Requirements covered:** ST-6 (full), ST-7, ST-8, ST-9 (UI), LN-4, LN-6, NS-8, NS-10,
JR-4, JR-6, ED-10, ED-11, ED-14, NF-10 (string externalisation).
**Depends on:** phases 2–4.

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh --full` passes.
2. Every requirement marked V1 in requirements.md (except plugins PG-* and AI-*, which are phases 6–7) is implemented and traceable to a verified task.
3. Manual acceptance checklist passes.

## Milestones

|     | Milestone                                 | Tasks       |
| --- | ----------------------------------------- | ----------- |
| 5A  | Data safety: conflict merge, integrity UI | P5.1–P5.3   |
| 5B  | Graph-level features                      | P5.4–P5.6   |
| 5C  | Links and namespaces                      | P5.7–P5.10  |
| 5D  | Templates and journals                    | P5.11–P5.12 |
| 5E  | Editor comfort                            | P5.13–P5.16 |

---

## 5A: Data safety

### P5.1 Three-way merge engine

- [x] In `core`: `merge(base, ours, theirs) -> MergeResult { merged_text, conflicts: Vec<BlockConflict> }`, working at **block level** (block trees aligned using block IDs, then content similarity, then position), with line-level diff3 inside a block only when both sides changed it
- [x] Base = the page as last synced, kept by the sync (`<replica>.sync-bases`); for a conflict copy its ancestor; else a guess from the history snapshots (P1.36)

**Verify**

- [x] Table tests (≥ 15 cases): different blocks edited → clean merge; same block edited identically → clean; same block edited differently → conflict; one side deletes a block the other edited → conflict; both add blocks at the same place → both kept (ours first); property changes merged per key
- [x] Property test: `merge(base, x, base) == x` and `merge(base, base, x) == x` for random trees
- [x] A clean merge result reparses with no new diagnostics

### P5.2 Conflict resolution view (ST-6)

- [x] For each Nextcloud conflict file: automatic merge attempt with P5.1; if clean → show the merged preview, one-click accept; else side-by-side view per conflicting block (ours / theirs / both / edit manually)
- [x] On accept: write the merged file (guarded), move the conflict file to `.notatnik/trash/` (recoverable), never delete it directly

**Verify**

- [x] A conflict made by the sync engine (two devices, in-memory server) appears in the conflict view; resolving yields a file containing both edits (`conflicts.spec.ts`; not yet tried with a real Nextcloud client's copy)
- [ ] The resolved conflict file is in `.notatnik/trash/` (done, tested); a restore button is not built, it is copied back by hand
- [x] A clean merge with one click: the file on disk equals the previewed result exactly

### P5.3 Integrity check UI (ST-9)

- [ ] A "Graph health" screen running `check`: broken links, duplicate block IDs (with a fix action: re-ID one), orphan assets, parse diagnostics with jump-to-location

**Verify**

- [ ] On fixture `edge-cases`, every reported issue is navigable, and each auto-fix produces the expected golden diff

## 5B: Graph-level features

### P5.4 Multiple graphs (ST-7)

- [ ] Graph switcher (recent graphs); each graph has its own cache dir, watcher and CalDAV config; only one graph open per window (decide: allow multiple windows?)

**Verify**

- [ ] Switching graphs stops the old watcher (no events from the old graph afterwards) and starts the new one
- [ ] Two graphs with identical page names don't share cache or search results

### P5.5 Assets (ST-8)

- [x] Paste an image or drag-drop an image file (png, jpeg, gif, webp; 25 MB) → copied to `assets/YYYY/MM/<name>-<short-hash>.<ext>` → `![](../assets/…)` inserted (path relative to the page file so other Markdown tools render it)
- [x] Image rendering in blocks (the desktop shell serves `asset://localhost/…` from `read_asset`); non-images as attachment chips are not built
- [x] Orphan asset detection (P5.3): `Graph::orphan_assets` and the Settings → Graph "Unused images" row; the health screen itself is still open

**Verify**

- [x] Paste a screenshot → the file exists in `assets/`, the link renders, and the same link works in the Nextcloud web Markdown viewer
- [x] Pasting the same image twice → one file (hash dedupe)
- [x] Renaming or moving a page (P5.10) rewrites its relative asset paths correctly

### P5.6 Trash

- [ ] Deleted pages and resolved conflict files go to `.notatnik/trash/` (synced) with a restore UI; auto-purge after N days (default 30)

**Verify**

- [ ] Delete a page → it vanishes from search and the tree; restore → back, with all links working again

## 5C: Links and namespaces

### P5.7 Aliases (LN-6)

- [ ] `aliases:` frontmatter; links via an alias resolve to the page; autocomplete shows aliases (with the target name); backlinks via an alias are included

**Verify**

- [ ] On fixture `refs`: `[[alpha]]` (an alias) opens `work/projects/alpha`; its backlinks include blocks linking via the alias
- [ ] Two pages claiming the same alias → a health-check warning; links resolve deterministically (documented rule)

### P5.8 Unlinked references (LN-4)

_Built early (with phase 2/3 work), see decision D26._ Aliases: the panel uses the folded alias keys as search terms until P5.7 adds display names.

- [x] Panel below the backlinks: plain-text mentions of the page title and aliases (FTS-based, whole-word, case-insensitive); a "Link" action converts one mention to `[[…]]`

**Verify**

- [x] A page "Anna" with mentions "anna", "Anna's" and "Annabelle" → the first two are listed, not "Annabelle" (`app.rs`, `mentions.rs` unit tests, `navigation.spec.ts`)
- [x] "Link" changes only that occurrence in the source file (`notes.md` compared byte for byte in `app.rs` and the browser test)

### P5.9 ~~Relative links in the UI~~ (removed)

Removed on 2026-10-03: all links are global (NS-6 removed). The task number is kept so other references stay valid.
Namespace-aware autocomplete ranking (NS-7, phase 2) covers the convenience part.

### P5.10 Namespace move / rename (NS-8)

_Built early: `Graph::plan_rename` / `apply_rename` / `revert_rename`, app commands `rename_plan`, `rename_apply`, `rename_revert`, the rename dialog. Tested in `graph.rs` (subtree move, root move, virtual root, conflict + revert, full revert), `app.rs` and `navigation.spec.ts`._

- [x] Rename/move a whole subtree: folder move + rewrite all links (and tags) pointing into the subtree + relative asset paths in moved pages; preview listing all files and the link count; guarded writes; if something fails mid-way, report exactly which files were changed and offer to finish or revert

**Verify**

- [x] Fixture `namespaces`: move `work/projects` → `archive/2026/projects` → `notatnik check` reports 0 broken links; the folder structure matches; unaffected files are byte-identical
- [x] Moving into its own descendant → rejected ("can't move `x` into itself")
- [x] Simulated write conflict (test: file 2 of 2) → the error lists the files already written; "Revert" in the dialog (`rename_revert`) restores them byte-identically; pressing Preview/Rename again finishes the move
- Relative link and image paths inside a moved page are rewritten so they still resolve (test `moving_a_page_rewrites_its_relative_image_paths`; code, URLs, absolute paths and anchors are left alone). Links _to_ a moved page written as relative Markdown paths in other pages are not rewritten

## 5D: Templates and journals

### P5.11 Templates (NS-10, JR-6)

- [x] Templates are Markdown files in `templates/` of the graph, one per page type (PR-10); variables: `{{date}}`, `{{title}}`, `{{namespace}}`, `{{cursor}}`
- [x] Per-namespace default templates (config `[namespaces."people"] template = …`, applies to new pages below `people`), the journal template, and a `/template` slash command for inserting any template as blocks

**Verify**

- [x] Creating `people/jan` (via a link + first edit, or via the tree) → the page starts with the person template, with variables filled
- [x] Today's journal, when created, contains the journal template
- [x] A template with invalid syntax → a clear error, and no partial insert

### P5.12 Journal calendar widget (JR-4)

- [ ] A month calendar in the sidebar; days with journal content marked; days with tasks or events marked differently; click → open that journal

**Verify**

- [ ] On fixture `basic`, the marked days match the existing journal files; clicking a day opens it

## 5E: Editor comfort

### P5.13 Drag-and-drop blocks (ED-10)

- [ ] Drag by the bullet; drop indicators for sibling/child placement; across pages (main view ↔ sidebar) = `Move` within a page, or delete + insert across files (one undo group)

**Verify**

- [ ] Drag within a page → the same result as the keyboard move (golden diff)
- [ ] Drag across pages → both files change; one Ctrl+Z restores both
- [ ] A block with a `^id` moved across pages keeps its ID, and refs to it still resolve

### P5.14 Smart paste (ED-11)

- [ ] Paste of Markdown (with headings, nested lists, code), HTML (from a browser, converted to Markdown), plain text with blank-line paragraphs → sensible block trees; a URL pasted over selected text → a Markdown link

- [x] Links (ED-27, D54): an address pasted over selected text → `[text](address)`; a link copied from a page keeps its text; tracking parameters dropped; `notatnik://` and Nextcloud file addresses become `[[Page]]`, `((id))` and `…/f/<id>`; opt-in title lookup (_Settings → Appearance_). The Markdown/HTML-to-blocks conversion above is still open.

**Verify**

- [ ] Vitest table: ≥ 10 clipboard inputs (incl. a copied Wikipedia section, a copied Logseq/Obsidian outline, a GitHub README section) → expected block trees
- [ ] Pasting a 1,000-line text completes in < 1 s and is one undo step

### P5.15 Themes and user CSS (ED-14)

- [ ] Light/dark themes via CSS variables; `.notatnik/custom.css` loaded if present (synced, so it applies on all devices)

**Verify**

- [ ] Switching the theme updates the UI without a restart; custom CSS that changes the bullet colour takes effect on reload
- [ ] Contrast of default themes passes WCAG AA (axe check)

### P5.16 String externalisation (NF-10)

- [ ] All UI strings in a message catalogue (e.g. a small i18n helper); English complete; Polish can be added as a file

**Verify**

- [ ] A lint/test fails if a `.svelte` file contains a hard-coded user-visible string outside the catalogue (allow-list for exceptions)
- [ ] A pseudo-locale (e.g. strings wrapped as `[!! … !!]`) renders with no unwrapped text in the main views

---

## Manual acceptance checklist

- [ ] Produce and resolve a real Nextcloud conflict end to end
- [ ] Reorganise your real namespaces (move one subtree); links still work everywhere
- [ ] Use templates for people and meeting pages for a week
- [ ] Paste content from 3 different sources (browser, PDF reader, another notes app) and check the results

Record: date / commit / notes:
