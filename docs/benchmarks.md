# Benchmarks and measurements

Machine: record CPU / RAM / disk with each table.

## WASM size (P1.18)

| Date | Commit | `.wasm` raw | gzip -9 | Notes |
|---|---|---|---|---|
| 2026-10-03 | phase 1 (core: parser, edits, queries) | 720 KB | 289 KB | `wasm-pack --release`, wasm-opt -Oz. Target < 600 KB gzipped: met |

## Phase 1: synthetic graph (P1.39)

Graph: `notatnik dev gen-graph --seed 1 --pages 10000` = 10,000 pages + 730 journals, 288,393 blocks, 49 MB, 10,730 files
(14,092 pages including virtual ones).
Machine: AMD Ryzen 5 PRO 5650U (6 cores / 12 threads), 46 GB RAM, Samsung 970 EVO Plus NVMe, Linux 6.8, release build.
Reproduce: `cargo bench -p notatnik-graph` (criterion) or
`cargo test --release -p notatnik-graph --test perf -- --ignored --nocapture` (asserts the targets).

| Date | What | Result | Target |
|---|---|---|---|
| 2026-10-04 | Cold index (parse in parallel, one write transaction) | 8.7 s | < 30 s (NF-2) ✅ |
| 2026-10-04 | Reindex one 100-block file | 4.0 ms | < 50 ms (NF-3) ✅ |
| 2026-10-04 | Search, common word (`project`) | 19 ms | < 100 ms (NF-5) ✅ |
| 2026-10-04 | Search, prefix (`proj*`) | 44 ms | < 100 ms ✅ |
| 2026-10-04 | Query: open tasks due within 7 days, sorted | 36 ms | < 100 ms ✅ |
| 2026-10-04 | Query: `tag: urgent` + any task | 9 ms | < 100 ms ✅ |
| 2026-10-04 | Query: `ns: work`, todo, grouped by page | 16 ms | < 100 ms ✅ |
| 2026-10-04 | Query: property filter, sorted by property, limit 50 | 15 ms | < 100 ms ✅ |
| 2026-10-04 | Query: full-text `text: budget` | 24 ms | < 100 ms ✅ |
| 2026-10-04 | Fuzzy title search, 3 characters, 14,092 titles | 2.1 ms | < 10 ms ✅ |
| 2026-10-04 | Parse the largest page (70 KB, 2,000 blocks) | 0.84 ms | n/a |

### Query plans and lessons

`EXPLAIN QUERY PLAN` for the five typical queries is printed by
`cargo test -p notatnik-index --test queries query_plans -- --nocapture`; each uses an index
(`blocks_task`, `blocks_deadline`, `pages` key range for `ns:`, `refs_target`, `props_owner`).

Two slow plans were found by this benchmark and fixed:

- `tag:` / `links:` used a correlated `EXISTS` per block (475 ms). Now `b.id IN (SELECT … FROM refs …)`, which
  SQLite drives from the `refs_target` index (9 ms).
- `prop.x: value` probed `props_key (key, value_fold)` once per block and then filtered by owner (over a minute).
  The owner index is now `(owner_kind, owner_id, key)` and positive property filters are `owner IN (SELECT …)` (15 ms).
  Schema version 2.
