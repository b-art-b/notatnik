# Spike S2: parser choice

**Date:** 2026-10-03. **Code:** `spikes/s2-parser/` (throwaway).

**Question:** which base parser gives reliable byte offsets for blocks and inline elements (D7, ST-3)?

## Setup

Three samples (CRLF, tab-indented, fenced code containing `- ` and `[[x]]`), plus a generated
2,000-block file. Candidates: `pulldown-cmark` 0.13 (offset iterator), `comrak` 0.40 (line/column
sourcepos), and a hand-written line-based outline parser (block start = `indent + "- "`, fence tracking).

## Results

|                                          | hand-written outline                 | pulldown-cmark                                      | comrak                          |
| ---------------------------------------- | ------------------------------------ | --------------------------------------------------- | ------------------------------- |
| CRLF sample                              | exact spans                          | exact                                               | line:col only (no byte offsets) |
| Tab sample                               | exact, depths 0/1/2/1                | item spans start with `\n` (CommonMark list quirks) | line:col only                   |
| Fence sample                             | `- ` inside the fence is not a block | correct                                             | correct                         |
| 2,000 blocks (release)                   | 39 µs                                | 2.8 ms                                              | 8.8 ms                          |
| WASM (release, opt-level z, no wasm-opt) | negligible                           | about 220 KB                                        | not measured (larger)           |

Block structure in our format is _ours_ (`- ` bullets, continuation lines, properties), not
CommonMark lists: CommonMark rules (4 spaces = code block, loose/tight lists, tab stops) would
fight the format. `pulldown-cmark` is good for inline events inside one block's text, with offsets
that we add to the block's start offset.

## Decision

- **Block/outline layer:** hand-written line-based parser (blocks, depth, properties, fences). It
  owns all spans, so byte-exact round trips don't depend on a CommonMark library.
- **Inline layer:** `pulldown-cmark` over a single block's content for emphasis, code spans and
  links; our own scanner for `[[ ]]`, `((id))`, `#tag`, `==hl==`, run only on text events (so code
  spans and fences are skipped). Spans from inline events are shifted by the block's offset.
- `comrak` is not used.

Recorded as D15 in the architecture decisions log.
