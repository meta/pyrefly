#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set -e

cd "$(dirname "$0")"

# Make sure Rust is on the PATH
export PATH="$HOME/.cargo/bin:$PATH"
# If you are running into issues with compiling zstd on your mac, you'll need to install
# clang following the instructions here: https://github.com/briansmith/ring/issues/1824
# Without rustup, the Rust installation must already have the target.
if command -v rustup > /dev/null; then
    rustup target add wasm32-wasip1
fi
cargo build --release --target wasm32-wasip1

mkdir -p target
cp pyrefly_wasm.js target/pyrefly_wasm.js
cp "${CARGO_TARGET_DIR:-../target}/wasm32-wasip1/release/pyrefly_wasm.wasm" target/pyrefly_wasm_bg.wasm

# wasm-opt is required only for the optimized website bundle.
if [[ "$*" != *--no-opt* ]]; then
  wasm-opt target/pyrefly_wasm_bg.wasm -Os -o target/pyrefly_wasm_bg.wasm.opt
fi

# Publish the hashes only after both WASM build inputs are ready.
node write-manifest.cjs
