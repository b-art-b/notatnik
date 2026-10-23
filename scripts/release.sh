#!/usr/bin/env bash
# Makes a release: sets the version, runs the checks, builds the .deb, copies it with a checksum into dist/, commits
# the version change and tags it. Nothing is pushed.
# Usage: scripts/release.sh <X.Y.Z> [--skip-check] [--dry-run]
#        scripts/release.sh            (prints the current version)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
for d in "$HOME"/.local/share/fnm/node-versions/*/installation/bin; do [[ -d "$d" ]] && export PATH="$d:$PATH"; done

current=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' Cargo.toml)
if [[ $# -eq 0 ]]; then
  echo "current version: $current (tags: $(git tag --list 'v*' --sort=-v:refname | head -3 | tr '\n' ' '))"
  echo "usage: scripts/release.sh <X.Y.Z> [--skip-check] [--dry-run]"
  exit 0
fi

version="$1"; shift
skip_check=0; dry=0
for a in "$@"; do
  case "$a" in
    --skip-check) skip_check=1 ;;
    --dry-run) dry=1 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

die() { echo "release: $*" >&2; exit 1; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "the version must look like 1.2.3, got '$version'"
[[ "$(git rev-parse --abbrev-ref HEAD)" == "main" ]] || die "release from the main branch"
[[ -z "$(git status --porcelain)" ]] || die "the working tree is not clean; commit or stash first"
git rev-parse -q --verify "refs/tags/v$version" >/dev/null && die "tag v$version exists already"
# the new version must be higher than the current one
[[ "$(printf '%s\n%s\n' "$current" "$version" | sort -V | tail -1)" == "$version" && "$current" != "$version" ]] \
  || die "$version is not higher than the current version $current"

echo "release: $current -> $version (check: $([[ $skip_check == 1 ]] && echo skipped || echo yes))"
[[ $dry == 1 ]] && { echo "dry run: nothing changed"; exit 0; }

# put the files back if a step fails, so a failed release leaves nothing behind
trap 'git checkout -- Cargo.toml Cargo.lock src-tauri/tauri.conf.json 2>/dev/null || true' ERR

sed -i "/^\[workspace.package\]/,/^\[/s/^version = \".*\"/version = \"$version\"/" Cargo.toml
sed -i "0,/\"version\": \".*\"/s//\"version\": \"$version\"/" src-tauri/tauri.conf.json
cargo update --workspace --offline --quiet

if [[ $skip_check == 0 ]]; then
  echo "==> checks"
  scripts/check.sh
fi

echo "==> build"
cargo tauri build --bundles deb

deb=$(ls -t target/release/bundle/deb/*"$version"*.deb 2>/dev/null | head -1 || true)
[[ -n "$deb" ]] || die "no .deb for $version under target/release/bundle/deb/"
mkdir -p dist
cp "$deb" dist/
(cd dist && sha256sum "$(basename "$deb")" > "$(basename "$deb").sha256")

git add Cargo.toml Cargo.lock src-tauri/tauri.conf.json
git commit -q -m "release: v$version

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
git tag -a "v$version" -m "Notatnik $version"
trap - ERR

echo "release: v$version tagged; package in dist/$(basename "$deb")"
echo "install:  sudo apt install ./dist/$(basename "$deb")"
echo "not pushed: git push && git push --tags (needs a remote)"
