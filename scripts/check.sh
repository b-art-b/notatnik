#!/usr/bin/env bash
# Single pre-commit check. Stops at the first failing step and says which one.
# Usage: scripts/check.sh [--full]   (--full: slow suites; none yet)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
# Node via fnm (not always on PATH in non-interactive shells)
for d in "$HOME"/.local/share/fnm/node-versions/*/installation/bin; do [[ -d "$d" ]] && export PATH="$d:$PATH"; done

FULL=0
[[ "${1:-}" == "--full" ]] && FULL=1

step() {
  local name="$1"; shift
  echo "==> $name"
  if ! "$@"; then
    echo "FAILED at step: $name" >&2
    exit 1
  fi
}

step "fmt"    cargo fmt --all --check
step "clippy" cargo clippy --workspace --all-targets -- -D warnings
step "test"   cargo test --workspace
step "doc"    env RUSTDOCFLAGS="-D warnings" cargo doc -p notatnik-core --no-deps --quiet
step "wasm"   cargo check -p notatnik-core --target wasm32-unknown-unknown
step "wasm-pack" wasm-pack build crates/core-wasm --target nodejs --release --out-dir pkg-node --quiet
step "wasm smoke test" node --test crates/core-wasm/tests/smoke.mjs

step "ui lint"  pnpm -C ui lint
step "ui check" pnpm -C ui check
step "ui unit"  pnpm -C ui test
step "ui e2e (Chrome + real backend)" pnpm -C ui test:e2e

if [[ $FULL -eq 1 ]]; then
  step "perf targets (NF-2, NF-3, NF-5)" cargo test --release -p notatnik-graph --test perf -- --ignored
  step "ui performance (headless Chrome)" env PERF=1 pnpm -C ui exec playwright test tests/perf.spec.ts --workers=1 --retries=0
  step "benchmarks" cargo bench -p notatnik-graph --bench synthetic -- --quick
fi
echo "All checks passed."
