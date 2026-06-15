#!/usr/bin/env bash
# Build the vellum-wasm crate for the browser (web target) and emit the JS
# loader + .wasm into the TS view package. Output dir is gitignored — these
# are build artifacts, not source.
#
# Requires: wasm-pack (https://drager.github.io/wasm-pack/) and the
# wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown`).
set -euo pipefail

wasm-pack build crates/wasm \
  --target web \
  --out-dir ../../ts/view/wasm \
  --out-name vellum
