#!/usr/bin/env bash
# Runs the browser tests in WebKit, the engine of the desktop app (WebKitGTK), next to the usual Chrome run of
# scripts/check.sh. The first run downloads the WebKit build for Playwright (~100 MB).
#   scripts/check-webkit.sh              the tests listed in tests/webkit.list
#   scripts/check-webkit.sh <pattern>    only the tests whose file matches the pattern
set -euo pipefail
export PATH="$HOME/.local/share/fnm/node-versions/v24.21.0/installation/bin:$PATH"
cd "$(dirname "$0")/../ui"
npx playwright install webkit >/dev/null
if [ "$#" -gt 0 ]; then
  exec npx playwright test -c playwright.webkit.config.ts "$@"
fi
exec npx playwright test -c playwright.webkit.config.ts $(grep -v '^#' tests/webkit.list | tr '\n' ' ')
