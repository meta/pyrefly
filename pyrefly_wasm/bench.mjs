/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Benchmarks the sandbox module that `build.sh` writes to `target/`.
//
// Usage: node bench.mjs [<bindings.js> <module.wasm>]
//
// It prints one JSON object: the module size, the one-time costs of loading
// the module and of the first check of a 544-line program, and the median time
// of the calls that the sandbox makes on each edit and for completion and
// hover. Most of the first check is loading the standard library stubs that
// the program imports. It fails if the module gives wrong results.

import assert from 'assert';
import { readFileSync } from 'fs';
import { pathToFileURL } from 'url';
import { gzipSync } from 'zlib';

const target = new URL('target/', import.meta.url);
const args = process.argv.slice(2);
if (args.length !== 0 && args.length !== 2) {
    console.error('Usage: node bench.mjs [<bindings.js> <module.wasm>]');
    process.exit(1);
}
const [bindingsPath, wasmPath] =
    args.length === 2
        ? args
        : [
              new URL('pyrefly_wasm.js', target),
              new URL('pyrefly_wasm_bg.wasm.opt', target),
          ];
const ITERATIONS = 30;

function time(f) {
    const start = performance.now();
    f();
    return performance.now() - start;
}

function median(f) {
    const times = Array.from({ length: ITERATIONS }, (_, i) => time(() => f(i)));
    return times.sort((a, b) => a - b)[Math.floor(ITERATIONS / 2)];
}

const lines = ['import os', ''];
for (let i = 0; i < 60; i++) {
    lines.push(
        `class C${i}:`,
        '    def __init__(self, x: int) -> None:',
        '        self.x = x',
        '    def get(self, y: str) -> list[int]:',
        '        return [self.x, len(y)]',
        '',
        `def f${i}(c: C${i}) -> int:`,
        '    return sum(c.get("a")) + os.getpid()',
        '',
    );
}
lines.push('reveal_type(f0(C0(1)))', 'os.');
const program = lines.join('\n');
const lastLine = lines.length;

const bindings = await import(
    bindingsPath instanceof URL ? bindingsPath.href : pathToFileURL(bindingsPath).href
);
const bytes = readFileSync(wasmPath);
const ms = {};
const start = performance.now();
await bindings.default(bytes);
ms.init = performance.now() - start;
let state;
ms.new_state = time(() => {
    state = new bindings.State('3.12');
});
ms.first_check = time(() => {
    state.updateSandboxFiles({ 'sandbox.py': program }, true);
    state.setActiveFile('sandbox.py');
    state.getErrors();
});
// The calls that the sandbox makes when the user types one character.
ms.edit = median((i) => {
    state.updateSingleFile('sandbox.py', program + ' '.repeat(i % 2));
    state.setActiveFile('sandbox.py');
    state.getErrors();
    state.inlayHint(false);
    state.semanticTokens(null);
});
ms.completion = median(() => state.autoComplete(lastLine, 4));
ms.hover = median(() => state.hover(lastLine - 1, 14));

assert(
    state.getErrors().some((error) => error.kind === 'reveal-type'),
    'the program should have a `reveal_type` result',
);
assert(
    state.autoComplete(lastLine, 4).length > 0,
    '`os.` should have completions',
);
assert.strictEqual(
    state.hover(lastLine - 1, 14)?.contents?.length,
    1,
    '`f0` should have a hover',
);

console.log(
    JSON.stringify({
        wasm_mb: +(bytes.length / 1e6).toFixed(1),
        wasm_gzip_mb: +(gzipSync(bytes).length / 1e6).toFixed(1),
        ...Object.fromEntries(
            Object.entries(ms).map(([name, t]) => [`${name}_ms`, +t.toFixed(1)]),
        ),
    }),
);
