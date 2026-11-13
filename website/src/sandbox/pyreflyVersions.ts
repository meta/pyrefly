/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

// The sandbox bundles one pyrefly build, from whatever `main` was at the last
// website deploy. The version selector loads other builds on demand from the
// `pyrefly-wasm` npm package through jsDelivr, so that a page load only ever
// downloads the bundled build. Publishing the package is a release-workflow
// task; the sandbox shows only the bundled build until it exists.

export const PYREFLY_NPM_PACKAGE = 'pyrefly-wasm';

// jsDelivr's data API lists the versions of an npm package, with CORS
// headers, so that the browser can fetch the list directly.
const JSDELIVR_DATA_URL = `https://data.jsdelivr.com/v1/packages/npm/${PYREFLY_NPM_PACKAGE}`;

// Caps keep the dropdown from growing without bound as the package ages.
const MAX_RELEASE_VERSIONS = 15;
const MAX_PRERELEASE_VERSIONS = 10;

// npm versions are always full semver; the manifest parser drops anything
// else, so that the version comparison stays well-defined.
const SEMVER = /^\d+\.\d+\.\d+(?:-.+)?(?:\+.+)?$/;

export interface PyreflyVersionInfo {
    /** The npm version, which matches the pyrefly release tag. */
    version: string;
    prerelease: boolean;
}

/** The glue and wasm files of one released build. */
export interface PyreflyWasmUrls {
    glue: string;
    wasm: string;
}

export function pyreflyWasmUrls(version: string): PyreflyWasmUrls {
    const base = `https://cdn.jsdelivr.net/npm/${PYREFLY_NPM_PACKAGE}@${version}`;
    return {
        glue: `${base}/pyrefly_wasm.js`,
        wasm: `${base}/pyrefly_wasm_bg.wasm`,
    };
}

/** Whether an npm version is a prerelease, such as `1.2.0-dev.2`. */
export function isPrerelease(version: string): boolean {
    // A `+` suffix is build metadata, which does not make a prerelease.
    return version.split('+', 1)[0].includes('-');
}

/** Fetches the package versions for the selector, newest first. */
export async function fetchPyreflyVersions(): Promise<PyreflyVersionInfo[]> {
    const response = await fetch(JSDELIVR_DATA_URL);
    if (!response.ok) {
        throw new Error(
            `Fetching ${JSDELIVR_DATA_URL} failed with status ${response.status}`
        );
    }
    return parsePyreflyVersions(await response.json());
}

/**
 * Parses the `versions` of a jsDelivr data API response into the selector
 * entries: recent stable releases followed by recent prereleases, each
 * newest first.
 */
export function parsePyreflyVersions(data: unknown): PyreflyVersionInfo[] {
    const versions = versionsOf(data);
    const sorted = versions.sort((a, b) => compareVersions(b, a));
    const releases = sorted
        .filter((version) => !isPrerelease(version))
        .slice(0, MAX_RELEASE_VERSIONS);
    const prereleases = sorted
        .filter(isPrerelease)
        .slice(0, MAX_PRERELEASE_VERSIONS);
    return [
        ...releases.map((version) => ({ version, prerelease: false })),
        ...prereleases.map((version) => ({ version, prerelease: true })),
    ];
}

function versionsOf(data: unknown): string[] {
    if (typeof data !== 'object' || data === null || !('versions' in data)) {
        return [];
    }
    const versions = (data as { versions: unknown }).versions;
    if (!Array.isArray(versions)) {
        return [];
    }
    return versions
        .map((entry) =>
            typeof entry === 'object' && entry !== null && 'version' in entry
                ? (entry as { version: unknown }).version
                : null
        )
        .filter((version): version is string => typeof version === 'string')
        .filter((version) => SEMVER.test(version));
}

/** Compares two npm (semver) versions: positive when `a` is later. */
export function compareVersions(a: string, b: string): number {
    const [aCore, aPre] = splitVersion(a);
    const [bCore, bPre] = splitVersion(b);
    const core = compareSegments(aCore, bCore);
    if (core !== 0) {
        return core;
    }
    if (aPre === undefined && bPre === undefined) {
        return 0;
    }
    if (aPre === undefined) {
        return 1; // A release is later than its prereleases.
    }
    if (bPre === undefined) {
        return -1;
    }
    return comparePrereleases(aPre, bPre);
}

/** Splits `core` and the optional prerelease from an npm version. */
function splitVersion(version: string): [string, string | undefined] {
    const [noBuild] = version.split('+', 1);
    const prerelease = noBuild.indexOf('-');
    return prerelease === -1
        ? [noBuild, undefined]
        : [noBuild.slice(0, prerelease), noBuild.slice(prerelease + 1)];
}

function compareSegments(a: string, b: string): number {
    const aParts = a.split('.');
    const bParts = b.split('.');
    for (let i = 0; i < Math.max(aParts.length, bParts.length); i++) {
        // A missing segment is `0`, as in semver (`1.2` equals `1.2.0`).
        const aNumber = Number(aParts[i] ?? 0);
        const bNumber = Number(bParts[i] ?? 0);
        if (aNumber !== bNumber) {
            return aNumber - bNumber;
        }
    }
    return 0;
}

/** Compares prerelease identifiers by the semver precedence rules. */
function comparePrereleases(a: string, b: string): number {
    const aParts = a.split('.');
    const bParts = b.split('.');
    for (let i = 0; i < Math.max(aParts.length, bParts.length); i++) {
        const aPart = aParts[i];
        const bPart = bParts[i];
        if (aPart === bPart) {
            continue;
        }
        if (aPart === undefined) {
            return -1; // A shorter prerelease sorts first.
        }
        if (bPart === undefined) {
            return 1;
        }
        const aNumber = isNumeric(aPart);
        const bNumber = isNumeric(bPart);
        if (aNumber && bNumber) {
            return Number(aPart) - Number(bPart);
        }
        if (aNumber) {
            return -1; // Numeric identifiers sort before alphanumeric ones.
        }
        if (bNumber) {
            return 1;
        }
        return aPart < bPart ? -1 : aPart > bPart ? 1 : 0;
    }
    return 0;
}

function isNumeric(part: string): boolean {
    return /^\d+$/.test(part);
}
