/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

// jsdom does not implement `fetch`, so tests that fetch stub it and restore
// the original, which may be absent.

const originalFetch = global.fetch;

export function stubFetch(
    implementation: (
        input: RequestInfo,
        init?: RequestInit
    ) => Promise<Response>
): void {
    global.fetch = implementation as unknown as typeof fetch;
}

export function restoreFetch(): void {
    global.fetch = originalFetch;
}
