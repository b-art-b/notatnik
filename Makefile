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

help: ## list the targets
	@grep -hE '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-14s %s\n", $$1, $$2}'

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
	sudo apt install -y --reinstall "$$(ls -t dist/*$(VERSION)*.deb $(DEB_DIR)/*$(VERSION)*.deb 2>/dev/null | head -1)"

uninstall: ## remove the installed app (graphs and settings stay)
	sudo apt remove -y notatnik

release: ## make release RELEASE=X.Y.Z: set the version, run the checks, build the .deb into dist/, commit and tag (no push)
	@test -n "$(RELEASE)" || { echo "usage: make release RELEASE=X.Y.Z   (current: $(VERSION))"; exit 2; }
	scripts/release.sh $(RELEASE)

clean: ## remove build output (target/, ui/dist, dist/)
	cargo clean
	rm -rf ui/dist dist
