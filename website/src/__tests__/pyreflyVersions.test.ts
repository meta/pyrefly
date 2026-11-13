/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

import {
    compareVersions,
    fetchPyreflyVersions,
    isPrerelease,
    parsePyreflyVersions,
    pyreflyWasmUrls,
} from '../sandbox/pyreflyVersions';
import { restoreFetch, stubFetch } from './__test_utils__/fetchStub';

describe('pyreflyVersions', () => {
    test('pyreflyWasmUrls builds jsDelivr URLs', () => {
        expect(pyreflyWasmUrls('1.2.0')).toEqual({
            glue: 'https://cdn.jsdelivr.net/npm/pyrefly-wasm@1.2.0/pyrefly_wasm.js',
            wasm: 'https://cdn.jsdelivr.net/npm/pyrefly-wasm@1.2.0/pyrefly_wasm_bg.wasm',
        });
    });

    test('isPrerelease', () => {
        expect(isPrerelease('1.2.0')).toBe(false);
        expect(isPrerelease('1.2.0-dev.2')).toBe(true);
        expect(isPrerelease('1.2.0-rc.1')).toBe(true);
        // Build metadata does not make a prerelease.
        expect(isPrerelease('1.2.0+build')).toBe(false);
    });

    test('compareVersions orders semver', () => {
        expect(compareVersions('1.2.0', '1.1.9')).toBeGreaterThan(0);
        expect(compareVersions('1.10.0', '1.9.0')).toBeGreaterThan(0);
        expect(compareVersions('1.2.0', '1.2.0')).toBe(0);
        // A missing segment is zero.
        expect(compareVersions('1.2', '1.2.0')).toBe(0);
        // Prereleases sort before their release, and numerically.
        expect(compareVersions('1.2.0', '1.2.0-dev.2')).toBeGreaterThan(0);
        expect(compareVersions('1.2.0-dev.2', '1.2.0-dev.10')).toBeLessThan(0);
        // A longer prerelease sorts later.
        expect(compareVersions('1.2.0-dev.2', '1.2.0-dev')).toBeGreaterThan(0);
        // Numeric identifiers sort before alphanumeric ones.
        expect(compareVersions('1.2.0-dev.2', '1.2.0-alpha')).toBeGreaterThan(
            0
        );
        expect(compareVersions('1.2.0-2', '1.2.0-alpha')).toBeLessThan(0);
    });

    test('parsePyreflyVersions sorts and groups', () => {
        expect(
            parsePyreflyVersions({
                versions: [
                    { version: '1.0.0' },
                    { version: '1.2.0-dev.2' },
                    { version: '1.1.0' },
                    { version: '1.2.0-dev.1' },
                    { version: 'not-a-version' },
                    { version: '1.1.0+build' },
                ],
            })
        ).toEqual([
            // `1.1.0+build` compares equal to `1.1.0` (build metadata is
            // ignored), so the stable sort keeps the input order.
            { version: '1.1.0', prerelease: false },
            { version: '1.1.0+build', prerelease: false },
            { version: '1.0.0', prerelease: false },
            { version: '1.2.0-dev.2', prerelease: true },
            { version: '1.2.0-dev.1', prerelease: true },
        ]);
    });

    test('parsePyreflyVersions caps the list', () => {
        const stable = [];
        const prereleases = [];
        for (let minor = 0; minor < 20; minor++) {
            stable.push({ version: `1.${minor}.0` });
            prereleases.push({ version: `2.${minor}.0-dev.1` });
        }
        const parsed = parsePyreflyVersions({
            versions: [...stable, ...prereleases],
        });
        expect(parsed.filter((info) => !info.prerelease)).toHaveLength(15);
        expect(parsed.filter((info) => info.prerelease)).toHaveLength(10);
    });

    test('parsePyreflyVersions tolerates a malformed response', () => {
        expect(parsePyreflyVersions(null)).toEqual([]);
        expect(parsePyreflyVersions({})).toEqual([]);
        expect(parsePyreflyVersions({ versions: 'oops' })).toEqual([]);
    });

    test('fetchPyreflyVersions parses the jsDelivr data API', async () => {
        const fetchMock = jest.fn().mockResolvedValue({
            ok: true,
            json: async () => ({ versions: [{ version: '1.2.0' }] }),
        });
        stubFetch(fetchMock);
        try {
            await expect(fetchPyreflyVersions()).resolves.toEqual([
                { version: '1.2.0', prerelease: false },
            ]);
            expect(fetchMock).toHaveBeenCalledWith(
                'https://data.jsdelivr.com/v1/packages/npm/pyrefly-wasm'
            );
        } finally {
            restoreFetch();
        }
    });

    test('fetchPyreflyVersions rejects on an error status', async () => {
        stubFetch(jest.fn().mockResolvedValue({ ok: false, status: 503 }));
        try {
            await expect(fetchPyreflyVersions()).rejects.toThrow('503');
        } finally {
            restoreFetch();
        }
    });
});
