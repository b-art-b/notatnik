#!/usr/bin/env bash
# Regenerates ui/src/lib/api/bindings.ts from the Rust API types (crates/app).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
UPDATE_BINDINGS=1 cargo test -q -p notatnik-app --test bindings
echo "wrote ui/src/lib/api/bindings.ts"
