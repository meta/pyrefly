# Pyrefly WASM Build

Pyrefly's WASM build is used in browser-based applications like the
[Pyrefly sandbox](https://pyrefly.org/sandbox/). It targets `wasm32-wasip1`.
`pyrefly_wasm.js` loads the module, provides the WASI imports, and exposes the
`State` class.

## Prerequisites

- Ensure you have clang installed:
  - For mac: `brew install llvm`
    ([source](https://github.com/briansmith/ring/issues/1824))
  - For CentOS: `sudo dnf install clang`
- Install Mercurial: `sudo apt install mercurial`

## Building Pyrefly for WASM

Run `./build.sh`

## Benchmarking the WASM build

After `./build.sh`, run `node bench.mjs`. It prints the module size, the time
to load the module and check a program, and the time of the calls that the
sandbox makes on each edit.

## Common issues with the build

Certain features that are allowed in the other builds are unavailable in the
wasm build. You might errors like `could not find \`<>\` in the crate
root`. This can be because we've excluded that package from the wasm build with `#[cfg(not(target_arch
= "wasm32"))]`in the`lib.rs`/`mod.rs` file.
