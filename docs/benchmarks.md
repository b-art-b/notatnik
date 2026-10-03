# Benchmarks and measurements

Machine: record CPU / RAM / disk with each table.

## WASM size (P1.18)

| Date | Commit | `.wasm` raw | gzip -9 | Notes |
|---|---|---|---|---|
| 2026-10-03 | phase 1 (core: parser, edits, queries) | 720 KB | 289 KB | `wasm-pack --release`, wasm-opt -Oz. Target < 600 KB gzipped: met |
