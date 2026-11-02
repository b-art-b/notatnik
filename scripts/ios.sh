#!/usr/bin/env bash
# The iPad (and iPhone) app, PL-5. iOS apps can only be built and signed on a Mac with Xcode: this script does nothing
# on another system. It checks the tools, adds the Rust targets, and runs the Tauri iOS commands.
#   scripts/ios.sh doctor   only reports what is found and what is missing
#   scripts/ios.sh check    compiles the Rust code for the iPad (`cargo check --target aarch64-apple-ios`), no Xcode project needed
#   scripts/ios.sh init     once: makes src-tauri/gen/apple (the Xcode project)
#   scripts/ios.sh dev      runs the app on a connected iPad or in the simulator (`cargo tauri ios dev`)
#   scripts/ios.sh build    builds an .ipa for TestFlight or a device (`cargo tauri ios build`)
# Needs APPLE_DEVELOPMENT_TEAM (your 10 character team ID, https://developer.apple.com/account) to sign the app: set it
# in the environment, or write it once into ~/.config/notatnik-ios/team (`make ios-team TEAM=XXXXXXXXXX`).
# `docs/phases/ipad.md` has the steps around it (Xcode, Apple account, trusting the app on the iPad).
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "iOS apps are built on a Mac with Xcode; this is $(uname -s). See docs/phases/ipad.md." >&2
  exit 1
fi
export PATH="$HOME/.cargo/bin:$PATH"
for d in "$HOME"/.local/share/fnm/node-versions/*/installation/bin /opt/homebrew/bin /usr/local/bin; do
  [[ -d "$d" ]] && export PATH="$d:$PATH"
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1 ($2)" >&2; exit 1; }; }
need xcodebuild "install Xcode from the App Store, open it once and accept the licence"
need rustup "https://rustup.rs"
need pnpm "npm install -g pnpm, or corepack enable"
if ! xcode-select -p >/dev/null 2>&1; then
  echo "run: xcode-select --install" >&2
  exit 1
fi

TEAMFILE="${NOTATNIK_IOS_TEAMFILE:-$HOME/.config/notatnik-ios/team}"
if [[ -z "${APPLE_DEVELOPMENT_TEAM:-}" && -s "$TEAMFILE" ]]; then
  APPLE_DEVELOPMENT_TEAM="$(tr -d '[:space:]' <"$TEAMFILE")"
fi
export APPLE_DEVELOPMENT_TEAM="${APPLE_DEVELOPMENT_TEAM:-}"
have_team() { [[ -n "$APPLE_DEVELOPMENT_TEAM" ]]; }
need_team() {
  have_team || { echo "no Apple team ID: make ios-team TEAM=XXXXXXXXXX (developer.apple.com/account → Membership; a free account works for your own devices)" >&2; exit 1; }
}

if [[ "${1:-}" == "doctor" ]]; then
  echo "Xcode: $(xcodebuild -version | head -1)"
  have_team && echo "team: $APPLE_DEVELOPMENT_TEAM" || echo "team: not set (make ios-team TEAM=XXXXXXXXXX)"
  echo "everything is there"
  exit 0
fi

# a real iPad, the simulator on Apple silicon, the simulator on an Intel Mac
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
if [[ "${1:-}" == "check" ]]; then
  cargo check --target aarch64-apple-ios -p notatnik-desktop
  echo "the Rust code compiles for the iPad"
  exit 0
fi
cargo tauri --version >/dev/null 2>&1 || cargo install tauri-cli --version '^2' --locked
(cd ui && pnpm install --frozen-lockfile)

case "${1:-}" in
  init) need_team; cargo tauri ios init ;;
  dev) need_team; shift; cargo tauri ios dev "$@" ;;
  build) need_team; shift; cargo tauri ios build "$@" ;;
  *)
    echo "usage: scripts/ios.sh doctor | check | init | dev | build" >&2
    exit 2
    ;;
esac
