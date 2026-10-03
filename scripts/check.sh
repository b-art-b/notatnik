#!/usr/bin/env bash
# Single pre-commit check. Stops at the first failing step and says which one.
# Usage: scripts/check.sh [--full]   (--full: slow suites; none yet)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

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
step "wasm"   cargo check -p notatnik-core --target wasm32-unknown-unknown

if [[ $FULL -eq 1 ]]; then
  echo "==> full: no slow suites yet"
fi
echo "All checks passed."
