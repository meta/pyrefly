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

## Filesystem-backed TSP sessions

The Rust library exposes
[`FilesystemSession`](../pyrefly/lib/tsp/session.rs) for native and WASIp1
hosts. It supports the existing `GetTypeParams` request shape for
`getComputedType`, session-local snapshots, and filesystem change invalidation.
It does not provide a transport or replace the sandbox's `State` class.

The host owns the filesystem and serializes file writes, change notifications,
and queries. Present unsaved editor contents at their ordinary filesystem
paths; no Pyrefly memory override is needed. Keep the saved baseline in the
host, restore it on discard, and notify the session after restoration.

1. Supply a `ConfigFile` with the intended Python environment and explicit
   search roots. WASI hosts must disable interpreter queries. The constructor
   reports configuration problems instead of initializing a fallback session.
2. Call `set_files` with absolute filesystem paths to analyze.
3. Write editor changes into the host filesystem, then call `files_changed`.
   Use creation/removal events for new or deleted files so import resolution
   is invalidated as well as file contents. Forward any returned
   `typeServer/snapshotChanged` notification to the client.
4. Get the snapshot with `get_snapshot` and include it in `GetTypeParams`.
   `get_computed_type` uses the shared transaction-aware converter and rejects
   outdated snapshots with the protocol's `ServerCancelled` error.

An empty event batch is a no-op. Avoid duplicate content notifications:
nonempty filesystem events count as external input changes. Reads on unchanged
inputs retain their snapshot. Snapshot values are scoped to a session and
compared for equality, not ordering.

File URI queries use UTF-16 ranges. Notebook cell projection, additional TSP
methods, cancellation, background scheduling, and Pylance integration are
outside this session's initial API. The host filesystem controls accessible
paths; the search roots are not a filesystem sandbox.

Declaration locations use the shared converter's existing path conventions.
Bundled stub locations cannot be materialized by the WASI guest. A host that
needs filesystem-addressable stub declarations should provide a complete
custom typeshed in its filesystem and configure `typeshed_path` accordingly.

## Benchmarking the WASM build

After `./build.sh`, run `node bench.mjs`. It prints the module size, the time
to load the module and check a program, and the time of the calls that the
sandbox makes on each edit.

## Common issues with the build

Certain features that are allowed in the other builds are unavailable in the
wasm build. You might errors like `could not find \`<>\` in the crate
root`. This can be because we've excluded that package from the wasm build with `#[cfg(not(target_arch
= "wasm32"))]`in the`lib.rs`/`mod.rs` file.
