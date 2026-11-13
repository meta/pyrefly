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
  - For Ubuntu and other Linux distributions where the C headers are not in
    `/usr/include`: install [wasi-sdk](https://github.com/WebAssembly/wasi-sdk)
    and set `WASI_SDK_PATH` to its folder.
- Install Mercurial: `sudo apt install mercurial`

## Building Pyrefly for WASM

Run `./build.sh`

## Shared TSP type conversion

Native and WASI library hosts can call
`pyrefly::tsp::convert_type_in_transaction` after computing a type in a
transaction. Pass that same transaction and its source handle so declaration
ranges, imports, and builtin types use the already-computed Python environment.
The Rust interface is internal and unstable; the native TSP transports are not
part of the WASI interface.

## Benchmarking the WASM build

After `./build.sh`, run `node bench.mjs`. It prints the module size, the time
to load the module and check a program, and the time of the calls that the
sandbox makes on each edit.

## Common issues with the build

Certain features that are allowed in the other builds are unavailable in the
wasm build. You might errors like `could not find \`<>\` in the crate
root`. This can be because we've excluded that package from the wasm build with `#[cfg(not(target_arch
= "wasm32"))]`in the`lib.rs`/`mod.rs` file.
