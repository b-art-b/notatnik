# Notatnik: build, test, package and install. `make help` lists the targets.
# The scripts do the work (scripts/*.sh); this file only gives them short names.

SHELL := /usr/bin/env bash
# Node comes from fnm (see CLAUDE.md)
NODE_BIN := $(lastword $(sort $(wildcard $(HOME)/.local/share/fnm/node-versions/*/installation/bin)))
export PATH := $(NODE_BIN):$(HOME)/.cargo/bin:$(PATH)

VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' Cargo.toml)
DEB_DIR := target/release/bundle/deb
# the newest .deb of the current version: dist/ (from `make release`) first, then the plain build
DEB = $(shell ls -t dist/*$(VERSION)*.deb $(DEB_DIR)/*$(VERSION)*.deb 2>/dev/null | head -1)

.DEFAULT_GOAL := help
.PHONY: help build test check check-webkit bindings dev deb install uninstall release version clean
.PHONY: android-doctor android-apk android-apk-debug android-install android-dev
.PHONY: ios-team ios-doctor ios-check ios-init ios-dev ios-ipa ios-first-run

help: ## list the targets
	@grep -hE '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-16s %s\n", $$1, $$2}'

version: ## print the current version
	@echo $(VERSION)

build: ## cargo build (debug)
	cargo build

test: ## cargo test
	cargo test

check: ## everything: fmt, clippy, Rust tests, UI lint/check/unit/browser tests
	scripts/check.sh

check-webkit: ## the browser tests of ui/tests/webkit.list in WebKit (the desktop app's engine)
	scripts/check-webkit.sh

bindings: ## regenerate the TypeScript bindings after changing Rust API types
	scripts/gen-bindings.sh

dev: ## cargo tauri dev with its own XDG dirs (~/.dev-notatnik), next to an installed app
	scripts/dev.sh

deb: ## build the .deb only (no version change, no checks): target/release/bundle/deb/
	cargo tauri build --bundles deb
	@echo; ls -t $(DEB_DIR)/*$(VERSION)*.deb | head -1

install: ## install the newest .deb of this version (builds it when there is none); asks for sudo
	@if [ -z "$(DEB)" ]; then $(MAKE) deb; fi
	@deb="$$(ls -t dist/*$(VERSION)*.deb $(DEB_DIR)/*$(VERSION)*.deb 2>/dev/null | head -1)"; \
	test -n "$$deb" || { echo "no .deb found"; exit 1; }; \
	echo "installing $$deb"; \
	sudo apt install -y --reinstall "$(CURDIR)/$$deb"

uninstall: ## remove the installed app (graphs and settings stay)
	sudo apt remove -y notatnik

release: ## make release RELEASE=X.Y.Z: set the version, run the checks, build the .deb into dist/, commit and tag (no push)
	@test -n "$(RELEASE)" || { echo "usage: make release RELEASE=X.Y.Z   (current: $(VERSION))"; exit 2; }
	scripts/release.sh $(RELEASE)

android-doctor: ## check the Android SDK, NDK and JDK (scripts/android.sh doctor)
	scripts/android.sh doctor

android-apk: ## signed release APK for arm64 phones and tablets: dist/notatnik-X.Y.Z-android-arm64.apk
	scripts/android.sh build --target aarch64

android-apk-debug: ## debug APK for arm64 (about 380 MB, for debugging only): dist/notatnik-X.Y.Z-android-arm64-debug.apk
	scripts/android.sh build --debug --target aarch64

android-install: ## install the release APK on the phone that is connected with adb (builds it when there is none)
	@apk="dist/notatnik-$(VERSION)-android-arm64.apk"; \
	if [ ! -f "$$apk" ]; then $(MAKE) android-apk; fi; \
	adb="$${ANDROID_HOME:-$$HOME/Android/Sdk}/platform-tools/adb"; \
	echo "installing $$apk"; \
	"$$adb" install -r "$$apk"

android-dev: ## run the app on a connected phone or an emulator (cargo tauri android dev)
	scripts/android.sh dev

# iOS apps are built on a Mac with Xcode only; on Linux these stop with a message (docs/phases/ipad.md).
# They need APPLE_DEVELOPMENT_TEAM, the 10 character team ID of an Apple account.
ios-team: ## once, on a Mac: remember your Apple team ID: make ios-team TEAM=XXXXXXXXXX
	@test -n "$(TEAM)" || { echo "usage: make ios-team TEAM=XXXXXXXXXX   (developer.apple.com/account -> Membership)"; exit 2; }
	@mkdir -p $(HOME)/.config/notatnik-ios && printf '%s\n' "$(TEAM)" > $(HOME)/.config/notatnik-ios/team && echo "saved the team ID"

ios-doctor: ## on a Mac: check Xcode, Rust, Node and the team ID
	scripts/ios.sh doctor

ios-check: ## on a Mac: does the Rust code compile for the iPad? (no Xcode project, no account needed)
	scripts/ios.sh check

ios-first-run: ## on a Mac, the first time: check, make the Xcode project, run it in the simulator
	scripts/ios.sh check
	scripts/ios.sh init
	scripts/ios.sh dev

ios-init: ## once, on a Mac: make the Xcode project src-tauri/gen/apple
	scripts/ios.sh init

ios-dev: ## on a Mac: run the app on a connected iPad or in the simulator
	scripts/ios.sh dev

ios-ipa: ## on a Mac: build the .ipa for a device or TestFlight
	scripts/ios.sh build

clean: ## remove build output (target/, ui/dist, dist/)
	cargo clean
	rm -rf ui/dist dist
