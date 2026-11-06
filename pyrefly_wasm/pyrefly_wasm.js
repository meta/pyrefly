/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JavaScript bindings for the `wasm32-wasip1` build of `lib.rs`.
//
// The module needs no files, environment, or arguments, so the WASI imports
// give it an empty environment and no preopened directories. Standard output
// and standard error go to the console.

// The exports of the live instance. `undefined` before `init` and while a
// new instance replaces one that crashed.
let wasm;
// The compiled module, which replaces an instance that crashed.
let wasmModule;
// Resolves when `wasm` is set. `undefined` before `init` and after a failed
// attempt, so that the next `init` tries again.
let ready;
// Increases with each new instance, so that a `State` can tell that its
// playground is gone.
let generation = 0;

const ERRNO_SUCCESS = 0;
const ERRNO_BADF = 8;
const ERRNO_NOSYS = 52;
const CLOCKID_MONOTONIC = 1;
// The most bytes that one `crypto.getRandomValues` call accepts.
const MAX_RANDOM_BYTES = 65536;

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const outputBuffers = { 1: '', 2: '' };

// Wasm passes an `i32` pointer to JavaScript as a signed number, so a pointer
// at or above 2 GiB is negative. `>>> 0` makes it unsigned again.

function view() {
    return new DataView(wasm.memory.buffer);
}

function bytes(ptr, len) {
    return new Uint8Array(wasm.memory.buffer, ptr >>> 0, len);
}

function writeOutput(fd, text) {
    const lines = (outputBuffers[fd] + text).split('\n');
    outputBuffers[fd] = lines.pop();
    for (const line of lines) {
        (fd === 1 ? console.log : console.error)(line);
    }
}

const wasi = {
    random_get(ptr, len) {
        for (let offset = 0; offset < len; offset += MAX_RANDOM_BYTES) {
            crypto.getRandomValues(
                bytes(ptr + offset, Math.min(MAX_RANDOM_BYTES, len - offset)),
            );
        }
        return ERRNO_SUCCESS;
    },
    environ_sizes_get(countPtr, sizePtr) {
        view().setUint32(countPtr >>> 0, 0, true);
        view().setUint32(sizePtr >>> 0, 0, true);
        return ERRNO_SUCCESS;
    },
    environ_get() {
        return ERRNO_SUCCESS;
    },
    clock_time_get(id, _precision, timePtr) {
        const millis =
            id === CLOCKID_MONOTONIC ? performance.now() : Date.now();
        const nanos = BigInt(Math.round(millis * 1e6));
        view().setBigUint64(timePtr >>> 0, nanos, true);
        return ERRNO_SUCCESS;
    },
    fd_write(fd, iovs, iovsLen, writtenPtr) {
        if (fd !== 1 && fd !== 2) {
            return ERRNO_BADF;
        }
        let written = 0;
        for (let i = 0; i < iovsLen; i++) {
            const iov = (iovs + i * 8) >>> 0;
            const ptr = view().getUint32(iov, true);
            const len = view().getUint32(iov + 4, true);
            writeOutput(fd, decoder.decode(bytes(ptr, len)));
            written += len;
        }
        view().setUint32(writtenPtr >>> 0, written, true);
        return ERRNO_SUCCESS;
    },
    proc_exit(code) {
        throw new Error(`pyrefly_wasm exited with code ${code}`);
    },
    sched_yield() {
        return ERRNO_SUCCESS;
    },
    fd_close: () => ERRNO_BADF,
    fd_fdstat_get: () => ERRNO_BADF,
    fd_filestat_get: () => ERRNO_BADF,
    fd_prestat_get: () => ERRNO_BADF,
    fd_prestat_dir_name: () => ERRNO_BADF,
    fd_read: () => ERRNO_BADF,
    fd_readdir: () => ERRNO_BADF,
    path_filestat_get: () => ERRNO_BADF,
    path_open: () => ERRNO_BADF,
};

// A WASI function without an implementation above returns `ERRNO_NOSYS`, so
// that a new import in the module does not make instantiation fail.
const wasiImports = new Proxy(wasi, {
    get: (target, name) => target[name] ?? (() => ERRNO_NOSYS),
});

// Compiles the module from `source` if it is not compiled yet, and makes a new
// instance of it.
function load(source) {
    const promise = (async () => {
        wasmModule ??= await compile(source);
        const instance = await WebAssembly.instantiate(wasmModule, {
            wasi_snapshot_preview1: wasiImports,
        });
        wasm = instance.exports;
        generation++;
    })();
    ready = promise;
    promise.catch(() => {
        if (ready === promise) {
            ready = undefined;
        }
    });
    return promise;
}

// Calls the export `name`. A trap or a stack overflow can stop Rust code in
// the middle of any operation, so the memory of the instance is no longer
// usable. A new instance replaces it, and each `State` opens its playground
// again in the new instance.
function callExport(name, ...args) {
    if (wasm === undefined) {
        throw new Error('pyrefly_wasm is not ready: wait for init()');
    }
    try {
        return wasm[name](...args);
    } catch (e) {
        wasm = undefined;
        load();
        throw e;
    }
}

function passString(text) {
    const encoded = encoder.encode(text);
    const ptr = callExport('pyrefly_alloc', encoded.length) >>> 0;
    bytes(ptr, encoded.length).set(encoded);
    return [ptr, encoded.length];
}

function takeResponse(packed) {
    // Wasm passes a `u64` to JavaScript as a signed `BigInt`.
    const unsigned = BigInt.asUintN(64, packed);
    const ptr = Number(unsigned >> 32n);
    const len = Number(unsigned & 0xffffffffn);
    const response = JSON.parse(decoder.decode(bytes(ptr, len)));
    callExport('pyrefly_free', ptr, len);
    if ('err' in response) {
        throw new Error(response.err);
    }
    return response.ok;
}

export function version() {
    return takeResponse(callExport('pyrefly_version'));
}

// Drops a playground that is still in the live instance. A playground in an
// instance that crashed is gone with that instance.
function drop({ handle, generation: playgroundGeneration }) {
    if (wasm !== undefined && playgroundGeneration === generation) {
        callExport('pyrefly_drop', handle);
    }
}

const registry = new FinalizationRegistry((playground) => {
    // A finalizer must not throw. If the drop crashes, `callExport` already
    // replaced the instance.
    try {
        drop(playground);
    } catch {}
});

// `serde_json` rejects a lone surrogate, so it becomes U+FFFD, as in the
// `wasm-bindgen` bindings. `String.prototype.toWellFormed` does the same, but
// older browsers that the website supports do not have it.
const SURROGATES = /[\uD800-\uDBFF][\uDC00-\uDFFF]|[\uD800-\uDFFF]/g;

function toWellFormed(text) {
    return text.replace(SURROGATES, (match) =>
        match.length === 2 ? match : '\uFFFD',
    );
}

export class State {
    constructor(version) {
        this.version = version;
        // The files and the active file to open the playground again with
        // after a crash. A call that crashes does not change them.
        this.files = {};
        this.activeFile = undefined;
        this.open();
    }

    open() {
        this.generation = generation;
        this.handle = takeResponse(
            callExport('pyrefly_new', ...passString(this.version)),
        );
        registry.register(
            this,
            { handle: this.handle, generation: this.generation },
            this,
        );
    }

    // Drops the playground now, instead of when the garbage collector frees
    // this `State`. The `State` must not be used after this.
    free() {
        registry.unregister(this);
        drop(this);
    }

    // The methods below convert numbers with `| 0` and flags with `Boolean`,
    // as `wasm-bindgen` converts `i32` and `bool` arguments.
    call(method, args) {
        if (this.generation !== generation) {
            args = this.reopen(method, args);
        }
        const request = JSON.stringify({ method, args }, (_key, value) =>
            typeof value === 'string' ? toWellFormed(value) : value,
        );
        return takeResponse(
            callExport('pyrefly_call', this.handle, ...passString(request)),
        );
    }

    // Opens the playground in the new instance after a crash, and returns the
    // arguments to run `method` with. The new playground gets the files and
    // the active file of the last call that did not crash, with the new input
    // of `method` applied, so that new input can replace files that crash.
    reopen(method, args) {
        registry.unregister(this);
        this.open();
        if (method === 'updateSandboxFiles') {
            // The new playground has no files to keep, so the update is forced.
            return [args[0], true];
        }
        const files = { ...this.files };
        if (method === 'updateSingleFile' && args[0] in files) {
            files[args[0]] = args[1];
        }
        this.call('updateSandboxFiles', [files, true]);
        if (this.activeFile !== undefined) {
            this.call('setActiveFile', this.activeFile);
        }
        return args;
    }

    updateSandboxFiles(files, forceUpdate) {
        const version = this.call('updateSandboxFiles', [
            files,
            Boolean(forceUpdate),
        ]);
        // `null` means that the playground kept its files.
        if (version !== null) {
            this.files = { ...files };
            this.activeFile = undefined;
        }
        return version;
    }

    updateSingleFile(filename, content) {
        this.call('updateSingleFile', [filename, content]);
        if (filename in this.files) {
            this.files[filename] = content;
            this.activeFile = filename;
        }
    }

    setActiveFile(filename) {
        this.call('setActiveFile', filename);
        this.activeFile = filename;
    }

    getErrors() {
        return this.call('getErrors');
    }

    hover(line, column) {
        return this.call('hover', [line | 0, column | 0]);
    }

    semanticTokens(range) {
        // A field that is not a number becomes 0, which `Playground` reads as
        // no range, as the `wasm-bindgen` bindings did.
        return this.call(
            'semanticTokens',
            range == null
                ? null
                : {
                      startLineNumber: range.startLineNumber | 0,
                      startColumn: range.startColumn | 0,
                      endLineNumber: range.endLineNumber | 0,
                      endColumn: range.endColumn | 0,
                  },
        );
    }

    semanticTokensLegend() {
        return this.call('semanticTokensLegend');
    }

    gotoDefinition(line, column, withContents) {
        return this.call('gotoDefinition', [
            line | 0,
            column | 0,
            Boolean(withContents),
        ]);
    }

    autoComplete(line, column) {
        return this.call('autoComplete', [line | 0, column | 0]);
    }

    inlayHint(callArgumentNames) {
        return this.call('inlayHint', Boolean(callArgumentNames));
    }
}

// Compiles the module from `source`: a URL, a `Response` or a promise of
// one, the module bytes, or a compiled `WebAssembly.Module`.
async function compile(source) {
    if (source instanceof WebAssembly.Module) {
        return source;
    }
    if (source instanceof URL || typeof source === 'string') {
        source = fetch(source);
    }
    source = await source;
    // Some hosts, such as the jsdom environment of Jest, have no `Response`.
    if (typeof Response === 'undefined' || !(source instanceof Response)) {
        return WebAssembly.compile(source);
    }
    if (!source.ok) {
        throw new Error(
            `pyrefly_wasm: fetching ${source.url} failed with status ${source.status}`,
        );
    }
    // `compileStreaming` compiles while the module downloads, and lets the
    // browser cache the compiled code, but it needs the `application/wasm`
    // type.
    return source.headers.get('Content-Type')?.startsWith('application/wasm')
        ? WebAssembly.compileStreaming(source)
        : WebAssembly.compile(await source.arrayBuffer());
}

// Instantiates the module from `source`, which `compile` describes. The
// default is `pyrefly_wasm_bg.wasm` next to this file. After a crash, the
// returned promise resolves when the new instance is ready.
export default function init(
    source = new URL('pyrefly_wasm_bg.wasm', import.meta.url),
) {
    return ready ?? load(source);
}
