/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

// The `pyrefly_wasm` import resolves to `__mocks__/pyreflyWasmMock.ts`, as
// configured for every test but `pyrefly_wasm.test.ts`.

import { loadPyrefly } from '../sandbox/pyreflyWasmLoader';

describe('pyreflyWasmLoader', () => {
    test('loads the bundled build and caches it', async () => {
        const first = loadPyrefly(null);
        // A repeated load of the bundled build reuses the promise.
        expect(loadPyrefly(null)).toBe(first);
        const loaded = await first;
        // The bundled build resolves its `.wasm` file relative to itself.
        expect(loaded.wasmSource).toBeUndefined();
        expect(loaded.mod.version()).toBe('1.2.3-test');
    });
});
