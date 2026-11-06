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

let wasm;

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

function passString(text) {
    const encoded = encoder.encode(text);
    const ptr = wasm.pyrefly_alloc(encoded.length) >>> 0;
    bytes(ptr, encoded.length).set(encoded);
    return [ptr, encoded.length];
}

function takeResponse(packed) {
    // Wasm passes a `u64` to JavaScript as a signed `BigInt`.
    const unsigned = BigInt.asUintN(64, packed);
    const ptr = Number(unsigned >> 32n);
    const len = Number(unsigned & 0xffffffffn);
    const response = JSON.parse(decoder.decode(bytes(ptr, len)));
    wasm.pyrefly_free(ptr, len);
    if ('err' in response) {
        throw new Error(response.err);
    }
    return response.ok;
}

export function version() {
    return takeResponse(wasm.pyrefly_version());
}

const registry = new FinalizationRegistry((handle) => wasm.pyrefly_drop(handle));

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
        this.handle = takeResponse(wasm.pyrefly_new(...passString(version)));
        registry.register(this, this.handle, this);
    }

    // Drops the playground now, instead of when the garbage collector frees
    // this `State`. The `State` must not be used after this.
    free() {
        registry.unregister(this);
        wasm.pyrefly_drop(this.handle);
    }

    // The methods below convert numbers with `| 0` and flags with `Boolean`,
    // as `wasm-bindgen` converts `i32` and `bool` arguments.
    call(method, args) {
        const request = JSON.stringify({ method, args }, (_key, value) =>
            typeof value === 'string' ? toWellFormed(value) : value,
        );
        return takeResponse(wasm.pyrefly_call(this.handle, ...passString(request)));
    }

    updateSandboxFiles(files, forceUpdate) {
        return this.call('updateSandboxFiles', [files, Boolean(forceUpdate)]);
    }

    updateSingleFile(filename, content) {
        this.call('updateSingleFile', [filename, content]);
    }

    setActiveFile(filename) {
        this.call('setActiveFile', filename);
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
// default is `pyrefly_wasm_bg.wasm` next to this file.
export default async function init(
    source = new URL('pyrefly_wasm_bg.wasm', import.meta.url),
) {
    if (wasm !== undefined) {
        return;
    }
    const instance = await WebAssembly.instantiate(await compile(source), {
        wasi_snapshot_preview1: wasiImports,
    });
    wasm = instance.exports;
}
