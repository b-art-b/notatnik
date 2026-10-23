#!/usr/bin/env bash
# Runs the desktop app in development (`cargo tauri dev`) with its own settings, data and cache folders, so it can run
# next to the installed app without sharing its settings, graph history or webview storage.
# Usage: scripts/dev.sh [cargo tauri dev arguments]
# The folders are under $NOTATNIK_DEV_HOME (default ~/.dev-notatnik); delete it to start from nothing.
# Not separated: the system keyring (a graph's key is stored under the graph's id), so open a scratch graph here,
# not the one the installed app uses.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
for d in "$HOME"/.local/share/fnm/node-versions/*/installation/bin; do [[ -d "$d" ]] && export PATH="$d:$PATH"; done

home="${NOTATNIK_DEV_HOME:-$HOME/.dev-notatnik}"
mkdir -p "$home/config" "$home/data" "$home/cache"
export XDG_CONFIG_HOME="$home/config" XDG_DATA_HOME="$home/data" XDG_CACHE_HOME="$home/cache"

echo "dev instance: settings in $XDG_CONFIG_HOME/notatnik, data in $XDG_DATA_HOME/notatnik"
cd src-tauri
exec cargo tauri dev "$@"
