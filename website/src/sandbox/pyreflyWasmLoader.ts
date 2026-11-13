/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

import { pyreflyWasmUrls } from './pyreflyVersions';

/**
 * A loaded pyrefly module: the JS glue, which exports `State` and
 * `version()`, plus the source that `default(source)` instantiates.
 * `wasmSource` is `undefined` for the bundled build, which resolves its
 * `.wasm` file relative to itself; a fetched build passes its CDN URL, also
 * for `default(source)` after the module crashed and restarts.
 */
export interface LoadedPyrefly {
    mod: any;
    wasmSource: string | undefined;
}

// The sandbox default is `null`: the bundled build from the last website
// deploy, and the fallback when another version cannot be loaded.

const BUNDLED_KEY = 'bundled';

// Loads cached by version, so that switching back to a version is instant.
const loadedModules = new Map<string, Promise<LoadedPyrefly>>();

/**
 * Loads the pyrefly module for `version`, or the bundled build for `null`.
 * A repeated version resolves with the cached load, and a failed load is
 * removed from the cache so that selecting it again retries.
 */
export function loadPyrefly(version: string | null): Promise<LoadedPyrefly> {
    const key = version ?? BUNDLED_KEY;
    const cached = loadedModules.get(key);
    if (cached) {
        return cached;
    }
    const promise = (
        version === null ? loadBundled() : loadFetched(version)
    ).catch((error) => {
        loadedModules.delete(key);
        throw error;
    });
    loadedModules.set(key, promise);
    return promise;
}

async function loadBundled(): Promise<LoadedPyrefly> {
    // The stub module is replaced with the real bindings by the website
    // build. The import stays dynamic so that the sandbox still renders when
    // the module is missing or fails to initialize.
    const uninitialized: Promise<any> =
        typeof window !== 'undefined'
            ? import('./pyrefly_wasm')
            : new Promise<never>(() => {});
    const mod = await uninitialized;
    await mod.default();
    return { mod, wasmSource: undefined };
}

async function loadFetched(version: string): Promise<LoadedPyrefly> {
    const urls = pyreflyWasmUrls(version);
    // Each version ships its own glue, matched to its wasm ABI, so the glue
    // cannot be shared with the bundled build. Fetch the glue text and
    // import it as a blob module, outside the webpack bundle.
    const response = await fetch(urls.glue);
    if (!response.ok) {
        throw new Error(
            `Fetching ${urls.glue} failed with status ${response.status}`
        );
    }
    const blobUrl = URL.createObjectURL(
        new Blob([await response.text()], { type: 'text/javascript' })
    );
    // `webpackIgnore` keeps this a native dynamic import of the blob URL.
    const mod = await import(/* webpackIgnore: true */ blobUrl);
    URL.revokeObjectURL(blobUrl);
    // Handshake: reject a build whose glue does not provide the sandbox
    // interface, instead of failing later on a typechecker call.
    const reported = mod.version();
    if (typeof reported !== 'string' || reported.length === 0) {
        throw new Error(`pyrefly ${version} has no version handshake`);
    }
    // `default` accepts a URL and streams the compile, which lets the
    // browser cache the compiled module for later visits.
    await mod.default(urls.wasm);
    return { mod, wasmSource: urls.wasm };
}
