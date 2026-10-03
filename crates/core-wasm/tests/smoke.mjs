// Smoke test: the WASM build must produce exactly the JSON that the Rust-side insta snapshots hold.
// Run via scripts/check.sh (after `wasm-pack build crates/core-wasm --target nodejs --out-dir pkg-node`).
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const wasm = require("../pkg-node/notatnik_core_wasm.js");
const root = new URL("../../../", import.meta.url).pathname;

const fixtures = [
  "basic/pages/work/projects/alpha.md",
  "edge-cases/pages/crlf.md",
  "edge-cases/pages/fences.md",
  "tasks/pages/repeats.md",
];

function snapshot(rel) {
  const name = rel.replaceAll("/", "__") + ".snap";
  const text = readFileSync(`${root}crates/core/tests/snapshots/fixtures/${name}`, "utf8");
  return JSON.parse(text.split("---\n").slice(2).join("---\n"));
}

for (const rel of fixtures) {
  test(`parse_page matches the Rust snapshot: ${rel}`, () => {
    const src = readFileSync(`${root}fixtures/graphs/${rel}`, "utf8");
    assert.deepEqual(JSON.parse(wasm.parse_page(src)), snapshot(rel));
  });
}

test("parse_query reports errors with position", () => {
  const r = JSON.parse(wasm.parse_query("tags: x"));
  assert.equal(r.errors[0].line, 1);
  assert.match(r.errors[0].message, /unknown key `tags`/);
});

test("resolve_link", () => {
  assert.deepEqual(JSON.parse(wasm.resolve_link(" work / alpha ")), { ok: "work/alpha" });
  assert.ok(JSON.parse(wasm.resolve_link("./x")).error);
});

test("apply_op round trip", () => {
  const r = JSON.parse(wasm.apply_op("- a\n- b\n", JSON.stringify({ op: "insert", parent: [], index: 1, text: "x" })));
  assert.equal(r.new_source, "- a\n- x\n- b\n");
});
