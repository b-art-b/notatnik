#!/usr/bin/env bash
# The Android app (PL-3), built on Linux or macOS. Finds the Android SDK, NDK and JDK, adds the Rust targets and runs
# the Tauri Android commands.
#   scripts/android.sh init     once: makes src-tauri/gen/android (the Gradle project)
#   scripts/android.sh dev      runs the app on a connected phone or an emulator (`cargo tauri android dev`)
#   scripts/android.sh build    builds a debug APK (`cargo tauri android build --apk --debug`); add --release for a
#                               release build, which needs a signing key (docs/phases/phase-8.md)
#   scripts/android.sh doctor   only reports what is found and what is missing
# The SDK is looked for in $ANDROID_HOME, else ~/Android/Sdk; it needs `platform-tools`, a `platforms;android-NN`, a
# `build-tools;NN` and an `ndk;NN` installed with `sdkmanager` (after `sdkmanager --licenses`).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
for d in "$HOME"/.local/share/fnm/node-versions/*/installation/bin; do [[ -d "$d" ]] && export PATH="$d:$PATH"; done

problems=()
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
[[ -d "$ANDROID_HOME" ]] || problems+=("no Android SDK in $ANDROID_HOME (set ANDROID_HOME)")
export ANDROID_SDK_ROOT="$ANDROID_HOME"

# the newest NDK that is installed
if [[ -z "${NDK_HOME:-}" && -d "$ANDROID_HOME/ndk" ]]; then
  NDK_HOME="$(find "$ANDROID_HOME/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)"
fi
export NDK_HOME="${NDK_HOME:-}"
[[ -n "$NDK_HOME" && -d "$NDK_HOME" ]] || problems+=("no NDK below $ANDROID_HOME/ndk (sdkmanager \"ndk;<version>\")")
[[ -d "$ANDROID_HOME/platform-tools" ]] || problems+=("no platform-tools (sdkmanager \"platform-tools\")")
ls -d "$ANDROID_HOME"/platforms/android-* >/dev/null 2>&1 || problems+=("no platform (sdkmanager \"platforms;android-36\")")
ls -d "$ANDROID_HOME"/build-tools/* >/dev/null 2>&1 || problems+=("no build-tools (sdkmanager \"build-tools;36.0.0\")")

# Gradle needs a JDK, not only a JRE: `javac` has to be there
if [[ -z "${JAVA_HOME:-}" ]] && command -v javac >/dev/null 2>&1; then
  JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")"
fi
export JAVA_HOME="${JAVA_HOME:-}"
[[ -n "$JAVA_HOME" && -x "$JAVA_HOME/bin/javac" ]] || problems+=("no JDK (javac): sudo apt install openjdk-21-jdk-headless")
command -v pnpm >/dev/null 2>&1 || problems+=("no pnpm (corepack enable)")
command -v cargo-tauri >/dev/null 2>&1 || problems+=("no tauri-cli (cargo install tauri-cli --version '^2' --locked)")

echo "ANDROID_HOME=$ANDROID_HOME"
echo "NDK_HOME=${NDK_HOME:-}"
echo "JAVA_HOME=${JAVA_HOME:-}"
if ((${#problems[@]})); then
  printf 'missing: %s\n' "${problems[@]}" >&2
  exit 1
fi
[[ "${1:-}" == "doctor" ]] && { echo "everything is there"; exit 0; }

# the release key lives outside the repository; a release build is signed with it when it is there
KEYDIR="${NOTATNIK_KEYDIR:-$HOME/.config/notatnik-android}"
if [[ -f "$KEYDIR/release.keystore" && -f "$KEYDIR/password" ]]; then
  export NOTATNIK_KEYSTORE="$KEYDIR/release.keystore"
  NOTATNIK_KEYSTORE_PASSWORD="$(cat "$KEYDIR/password")"
  export NOTATNIK_KEYSTORE_PASSWORD
fi

rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
(cd ui && pnpm install --frozen-lockfile)

case "${1:-}" in
  init) cargo tauri android init ;;
  dev) shift; cargo tauri android dev "$@" ;;
  build)
    shift
    args=("${@:---debug}")
    cargo tauri android build --apk "${args[@]}"
    # Gradle calls the file app-universal-release.apk; a copy in dist/ says what it is: notatnik-0.1.0-android-arm64.apk
    kind=release
    abi=universal
    for a in "${args[@]}"; do
      [[ "$a" == "--debug" ]] && kind=debug
    done
    case " ${args[*]} " in
      *" aarch64 "*) abi=arm64 ;;
      *" armv7 "*) abi=armv7 ;;
      *" x86_64 "*) abi=x86_64 ;;
      *" i686 "*) abi=x86 ;;
    esac
    version="$(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)"
    src="$(ls -t src-tauri/gen/android/app/build/outputs/apk/*/"$kind"/*.apk | head -1)"
    name="notatnik-$version-android-$abi"
    [[ "$kind" == debug ]] && name="$name-debug"
    mkdir -p dist
    cp "$src" "dist/$name.apk"
    echo "APK: dist/$name.apk"
    ;;
  *)
    echo "usage: scripts/android.sh init | dev | build | doctor" >&2
    exit 2
    ;;
esac
