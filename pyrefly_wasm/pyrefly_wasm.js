/*
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

function view() {
    return new DataView(wasm.memory.buffer);
}

function bytes(ptr, len) {
    return new Uint8Array(wasm.memory.buffer, ptr, len);
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
        view().setUint32(countPtr, 0, true);
        view().setUint32(sizePtr, 0, true);
        return ERRNO_SUCCESS;
    },
    environ_get() {
        return ERRNO_SUCCESS;
    },
    clock_time_get(id, _precision, timePtr) {
        const millis =
            id === CLOCKID_MONOTONIC ? performance.now() : Date.now();
        const nanos = BigInt(Math.round(millis * 1e6));
        view().setBigUint64(timePtr, nanos, true);
        return ERRNO_SUCCESS;
    },
    fd_write(fd, iovs, iovsLen, writtenPtr) {
        if (fd !== 1 && fd !== 2) {
            return ERRNO_BADF;
        }
        let written = 0;
        for (let i = 0; i < iovsLen; i++) {
            const ptr = view().getUint32(iovs + i * 8, true);
            const len = view().getUint32(iovs + i * 8 + 4, true);
            writeOutput(fd, decoder.decode(bytes(ptr, len)));
            written += len;
        }
        view().setUint32(writtenPtr, written, true);
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
    const ptr = wasm.pyrefly_alloc(encoded.length);
    bytes(ptr, encoded.length).set(encoded);
    return [ptr, encoded.length];
}

function takeResponse(packed) {
    const ptr = Number(packed >> 32n);
    const len = Number(packed & 0xffffffffn);
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

export class State {
    constructor(version) {
        this.handle = takeResponse(wasm.pyrefly_new(...passString(version)));
        registry.register(this, this.handle);
    }

    call(method, args) {
        const request = JSON.stringify({ method, args });
        return takeResponse(wasm.pyrefly_call(this.handle, ...passString(request)));
    }

    updateSandboxFiles(files, forceUpdate) {
        return this.call('updateSandboxFiles', [files, forceUpdate]);
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
        return this.call('hover', [line, column]);
    }

    semanticTokens(range) {
        return this.call('semanticTokens', range ?? null);
    }

    semanticTokensLegend() {
        return this.call('semanticTokensLegend');
    }

    gotoDefinition(line, column) {
        return this.call('gotoDefinition', [line, column]);
    }

    autoComplete(line, column) {
        return this.call('autoComplete', [line, column]);
    }

    inlayHint(callArgumentNames) {
        return this.call('inlayHint', callArgumentNames);
    }
}

// Instantiates the module from `source`: a URL, or the module bytes. The
// default is `pyrefly_wasm_bg.wasm` next to this file.
export default async function init(
    source = new URL('pyrefly_wasm_bg.wasm', import.meta.url),
) {
    if (wasm !== undefined) {
        return;
    }
    if (source instanceof URL || typeof source === 'string') {
        source = await (await fetch(source)).arrayBuffer();
    }
    const { instance } = await WebAssembly.instantiate(source, {
        wasi_snapshot_preview1: wasiImports,
    });
    wasm = instance.exports;
}
