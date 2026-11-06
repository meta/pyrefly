/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JavaScript bindings for the `wasm32-wasip1` build of `lib.rs`.
//
// The WASI imports give the module an empty environment and one preopened
// directory, `/`, whose files live in JavaScript memory (`files` below). The
// module can only read them: the host writes them. Standard output and
// standard error go to the console.

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
const ERRNO_INVAL = 28;
const ERRNO_NOENT = 44;
const ERRNO_NOSYS = 52;
const ERRNO_NOTDIR = 54;
const ERRNO_ROFS = 69;
const CLOCKID_MONOTONIC = 1;
const FILETYPE_CHARACTER_DEVICE = 2;
const FILETYPE_DIRECTORY = 3;
const FILETYPE_REGULAR_FILE = 4;
const OFLAGS_CREAT = 1;
const OFLAGS_DIRECTORY = 2;
const WHENCE_CUR = 1;
const WHENCE_END = 2;
const PREOPEN_FD = 3;
const ALL_RIGHTS = 0xffffffffffffffffn;
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

// The contents of each file in the filesystem, by absolute normalized path.
// A directory exists while a file is under it.
const fileContents = new Map();
// Each open file descriptor: its path, whether it is a directory, the file
// contents when it was opened, and the read position.
const descriptors = new Map([[PREOPEN_FD, { path: '/', isDirectory: true }]]);
let nextDescriptor = PREOPEN_FD + 1;

function normalize(path) {
    const parts = [];
    for (const part of path.split('/')) {
        if (part === '..') {
            parts.pop();
        } else if (part !== '' && part !== '.') {
            parts.push(part);
        }
    }
    return '/' + parts.join('/');
}

function isDirectory(path) {
    const prefix = path === '/' ? '/' : path + '/';
    for (const file of fileContents.keys()) {
        if (file.startsWith(prefix)) {
            return true;
        }
    }
    return false;
}

// The names and file types of the entries of the directory at `path`.
function directoryEntries(path) {
    const prefix = path === '/' ? '/' : path + '/';
    const entries = new Map();
    for (const file of fileContents.keys()) {
        if (file.startsWith(prefix)) {
            const [name, ...rest] = file.slice(prefix.length).split('/');
            entries.set(
                name,
                rest.length > 0 ? FILETYPE_DIRECTORY : FILETYPE_REGULAR_FILE,
            );
        }
    }
    return [...entries];
}

// Resolves the path at `ptr`/`len` against the directory descriptor `fd`.
function resolve(fd, ptr, len) {
    const directory = descriptors.get(fd);
    if (directory?.isDirectory !== true) {
        return undefined;
    }
    return normalize(`${directory.path}/${decoder.decode(bytes(ptr, len))}`);
}

function writeFilestat(ptr, filetype, size) {
    bytes(ptr, 64).fill(0);
    view().setUint8((ptr + 16) >>> 0, filetype);
    view().setBigUint64((ptr + 24) >>> 0, 1n, true);
    view().setBigUint64((ptr + 32) >>> 0, BigInt(size), true);
}

function stat(path, ptr) {
    if (fileContents.has(path)) {
        writeFilestat(ptr, FILETYPE_REGULAR_FILE, fileContents.get(path).length);
    } else if (isDirectory(path)) {
        writeFilestat(ptr, FILETYPE_DIRECTORY, 0);
    } else {
        return ERRNO_NOENT;
    }
    return ERRNO_SUCCESS;
}

// The filesystem that the module reads. Report each change to the workspaces
// that read the file, with `Workspace.filesChanged`.
export const files = {
    write(path, text) {
        fileContents.set(normalize(path), encoder.encode(text));
    },
    remove(path) {
        fileContents.delete(normalize(path));
    },
};

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
    fd_prestat_get(fd, ptr) {
        if (fd !== PREOPEN_FD) {
            return ERRNO_BADF;
        }
        view().setUint8(ptr >>> 0, 0);
        view().setUint32((ptr + 4) >>> 0, 1, true);
        return ERRNO_SUCCESS;
    },
    fd_prestat_dir_name(fd, ptr, len) {
        if (fd !== PREOPEN_FD) {
            return ERRNO_BADF;
        }
        bytes(ptr, len).set(encoder.encode('/').subarray(0, len));
        return ERRNO_SUCCESS;
    },
    fd_fdstat_get(fd, ptr) {
        const descriptor = descriptors.get(fd);
        if (descriptor === undefined && fd > 2) {
            return ERRNO_BADF;
        }
        bytes(ptr, 24).fill(0);
        view().setUint8(
            ptr >>> 0,
            descriptor === undefined
                ? FILETYPE_CHARACTER_DEVICE
                : descriptor.isDirectory
                  ? FILETYPE_DIRECTORY
                  : FILETYPE_REGULAR_FILE,
        );
        view().setBigUint64((ptr + 8) >>> 0, ALL_RIGHTS, true);
        view().setBigUint64((ptr + 16) >>> 0, ALL_RIGHTS, true);
        return ERRNO_SUCCESS;
    },
    path_open(
        fd,
        _dirflags,
        pathPtr,
        pathLen,
        oflags,
        _rightsBase,
        _rightsInheriting,
        _fdflags,
        fdPtr,
    ) {
        const path = resolve(fd, pathPtr, pathLen);
        if (path === undefined) {
            return ERRNO_BADF;
        }
        if (oflags & OFLAGS_CREAT) {
            return ERRNO_ROFS;
        }
        let descriptor;
        if (fileContents.has(path)) {
            if (oflags & OFLAGS_DIRECTORY) {
                return ERRNO_NOTDIR;
            }
            descriptor = {
                path,
                isDirectory: false,
                contents: fileContents.get(path),
                position: 0,
            };
        } else if (isDirectory(path)) {
            descriptor = { path, isDirectory: true };
        } else {
            return ERRNO_NOENT;
        }
        descriptors.set(nextDescriptor, descriptor);
        view().setUint32(fdPtr >>> 0, nextDescriptor++, true);
        return ERRNO_SUCCESS;
    },
    fd_close(fd) {
        if (fd === PREOPEN_FD || !descriptors.delete(fd)) {
            return ERRNO_BADF;
        }
        return ERRNO_SUCCESS;
    },
    fd_read(fd, iovs, iovsLen, readPtr) {
        const descriptor = descriptors.get(fd);
        if (descriptor === undefined || descriptor.isDirectory) {
            return ERRNO_BADF;
        }
        let read = 0;
        for (let i = 0; i < iovsLen; i++) {
            const iov = (iovs + i * 8) >>> 0;
            const chunk = descriptor.contents.subarray(
                descriptor.position,
                descriptor.position + view().getUint32(iov + 4, true),
            );
            bytes(view().getUint32(iov, true), chunk.length).set(chunk);
            descriptor.position += chunk.length;
            read += chunk.length;
        }
        view().setUint32(readPtr >>> 0, read, true);
        return ERRNO_SUCCESS;
    },
    fd_seek(fd, offset, whence, positionPtr) {
        const descriptor = descriptors.get(fd);
        if (descriptor === undefined || descriptor.isDirectory) {
            return ERRNO_BADF;
        }
        const base =
            whence === WHENCE_CUR
                ? descriptor.position
                : whence === WHENCE_END
                  ? descriptor.contents.length
                  : 0;
        const position = base + Number(offset);
        if (position < 0) {
            return ERRNO_INVAL;
        }
        descriptor.position = position;
        view().setBigUint64(positionPtr >>> 0, BigInt(position), true);
        return ERRNO_SUCCESS;
    },
    fd_filestat_get(fd, ptr) {
        const descriptor = descriptors.get(fd);
        if (descriptor === undefined) {
            return ERRNO_BADF;
        }
        if (descriptor.isDirectory) {
            writeFilestat(ptr, FILETYPE_DIRECTORY, 0);
        } else {
            writeFilestat(ptr, FILETYPE_REGULAR_FILE, descriptor.contents.length);
        }
        return ERRNO_SUCCESS;
    },
    path_filestat_get(fd, _flags, pathPtr, pathLen, ptr) {
        const path = resolve(fd, pathPtr, pathLen);
        return path === undefined ? ERRNO_BADF : stat(path, ptr);
    },
    // There are no symbolic links.
    path_readlink: () => ERRNO_INVAL,
    // Each entry is a 24-byte `dirent` and then its name. An entry that does
    // not fit is cut off, and the module reads it again with a bigger buffer.
    fd_readdir(fd, ptr, len, cookie, usedPtr) {
        const descriptor = descriptors.get(fd);
        if (descriptor?.isDirectory !== true) {
            return ERRNO_BADF;
        }
        const chunks = directoryEntries(descriptor.path)
            .slice(Number(cookie))
            .map(([name, filetype], i) => {
                const encoded = encoder.encode(name);
                const chunk = new Uint8Array(24 + encoded.length);
                const dirent = new DataView(chunk.buffer);
                dirent.setBigUint64(0, cookie + BigInt(i + 1), true);
                dirent.setUint32(16, encoded.length, true);
                dirent.setUint8(20, filetype);
                chunk.set(encoded, 24);
                return chunk;
            });
        let used = 0;
        for (const chunk of chunks) {
            const part = chunk.subarray(0, len - used);
            bytes(ptr + used, part.length).set(part);
            used += part.length;
            if (used === len) {
                break;
            }
        }
        view().setUint32(usedPtr >>> 0, used, true);
        return ERRNO_SUCCESS;
    },
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

// A project in `files` under the directory `root`, which is its import root.
// Pyrefly reads its files with filesystem paths, so the host edits a file by
// writing it with `files` and then calling `filesChanged`, as a file watcher
// would. A workspace does not survive a crash of the instance.
export class Workspace {
    constructor(root) {
        this.handle = takeResponse(
            callExport('pyrefly_workspace_new', ...passString(root)),
        );
    }

    call(method, args) {
        return takeResponse(
            callExport(
                'pyrefly_workspace_call',
                this.handle,
                ...passString(JSON.stringify({ method, args })),
            ),
        );
    }

    // Checks the files at the absolute `paths` from now on.
    setFiles(paths) {
        this.call('setFiles', paths);
    }

    // Reports which files the host created, modified, or removed in `files`.
    filesChanged({ created = [], modified = [], removed = [] }) {
        this.call('filesChanged', { created, modified, removed });
    }

    getErrors() {
        return this.call('getErrors');
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
