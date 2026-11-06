/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

import '@testing-library/jest-dom';
import { PyreflyState } from '../sandbox/Sandbox';
import { DEFAULT_SANDBOX_PROGRAM } from '../sandbox/DefaultSandboxProgram';
import {
    createPyreflyState,
    createPyreflyWasmModule,
    findError,
    isPyreflyWasmAvailable,
} from './__test_utils__/PyreflyWasmTestUtils';
import { TextEncoder, TextDecoder } from 'util';

Object.assign(global, { TextDecoder, TextEncoder });

// These tests require the actual wasm module to be built and copied to the test directory.
// They will be skipped if only the stub is present.
describe('pyrefly_wasm', () => {
    let pyreService: PyreflyState;
    let wasmAvailable = false;

    beforeAll(async () => {
        wasmAvailable = await isPyreflyWasmAvailable();
        if (!wasmAvailable && process.env.PYREFLY_WASM_REQUIRED) {
            throw new Error(
                'PYREFLY_WASM_REQUIRED is set, but the wasm module is not built.'
            );
        }
        if (!wasmAvailable) {
            console.log(
                'Skipping pyrefly_wasm tests: wasm module not built. ' +
                    'See src/__tests__/wasm/pyrefly_wasm_for_testing.js for details.'
            );
            return;
        }
        try {
            // Create a new PyreflyState instance using our test utility
            pyreService = await createPyreflyState();
        } catch (error) {
            console.error('Error initializing pyrefly_wasm:', error);
            throw error;
        }
    });

    beforeEach(() => {
        if (!wasmAvailable) return;
        // Initialize sandbox files with main.py
        pyreService.updateSandboxFiles(
            {
                'main.py': DEFAULT_SANDBOX_PROGRAM,
            },
            true
        );
    });

    describe('getErrors', () => {
        it('simple python program, checks for errors for reveal type, bad assignment, parse error', () => {
            if (!wasmAvailable) return;
            const programWithError = `
x: int = ""
import
`;
            pyreService.updateSingleFile(
                'main.py',
                DEFAULT_SANDBOX_PROGRAM + programWithError
            );
            const errors = pyreService.getErrors();

            // Should have at least one error for reveal_type
            expect(errors.length).toBeGreaterThan(0);

            // Find the reveal_type error
            const revealTypeError = findError(errors, 'revealed type:');
            expect(revealTypeError).toBeDefined();
            expect(revealTypeError.kind).toEqual('reveal-type');
            // The revealed type should be 'int'
            const match = revealTypeError.message_header.match(
                /revealed type: ([^\s]+)/
            );
            const revealedType = match[1];
            expect(revealedType).toBe('int');

            const badAssignmentError = findError(
                errors,
                "`Literal['']` is not assignable to `int`"
            );
            expect(badAssignmentError).toBeDefined();
            expect(badAssignmentError.kind).toEqual('bad-assignment');

            const parseError = findError(
                errors,
                'Parse error: Expected one or more symbol names after import'
            );
            expect(parseError).toBeDefined();
            expect(parseError.kind).toEqual('parse-error');
        });

        it('complex python program, error with typedDict', () => {
            if (!wasmAvailable) return;
            // Update source with a complete program
            pyreService.updateSingleFile(
                'main.py',
                `
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

movie: Movie = {'name': 'Blade Runner',
                'year': '1982'}
`.trimStart()
            );

            // Test getErrors
            const errors = pyreService.getErrors();
            expect(errors.length).toEqual(1);

            // Should have revealed types for result and difference
            const typedDictError = findError(
                errors,
                "`Literal['1982']` is not assignable to TypedDict key `year` with type `int`"
            );
            expect(typedDictError).toBeDefined();
            expect(typedDictError.kind).toBe('bad-assignment');
        });
    });

    describe('gotoDefinition', () => {
        it('should return definition location for function call', () => {
            if (!wasmAvailable) return;
            pyreService.setActiveFile('main.py');
            // Position of "test" in "reveal_type(test(42))" on line 6
            const definitions = pyreService.gotoDefinition(6, 13, true);

            expect(definitions).toBeDefined();
            expect(definitions).not.toBeNull();
            expect(definitions!.length).toBeGreaterThan(0);

            const definition = definitions![0];
            expect(definition.filename).toBe('main.py');
            expect(definition.range).toEqual({
                startLineNumber: 3,
                startColumn: 5,
                endLineNumber: 3,
                endColumn: 9,
            });
            expect(definition.contents).toBeUndefined();
        });

        it('should return the file of a definition in another file', () => {
            if (!wasmAvailable) return;
            pyreService.updateSandboxFiles(
                {
                    'main.py':
                        'import os\nfrom utils import f\nf()\nos.getcwd()\n',
                    'utils.py': '\n\ndef f() -> None: ...\n',
                },
                true
            );
            pyreService.setActiveFile('main.py');
            expect(pyreService.gotoDefinition(3, 1, true)).toEqual([
                {
                    filename: 'utils.py',
                    range: {
                        startLineNumber: 3,
                        startColumn: 5,
                        endLineNumber: 3,
                        endColumn: 6,
                    },
                },
            ]);
            const [stub] = pyreService.gotoDefinition(4, 4, true)!;
            expect(stub.filename).toBe('typeshed/stdlib/os/__init__.pyi');
            expect(stub.contents).toContain('def getcwd()');
            const [stubOnly] = pyreService.gotoDefinition(4, 4, false)!;
            expect(stubOnly.contents).toBeUndefined();
        });
    });

    describe('autoComplete', () => {
        it('should return completion items for function name', () => {
            if (!wasmAvailable) return;
            const typingForAutocomplete = `
tes
`;
            pyreService.updateSingleFile(
                'main.py',
                DEFAULT_SANDBOX_PROGRAM + typingForAutocomplete
            );

            const completions = pyreService.autoComplete(8, 4);
            expect(completions.length).toBeGreaterThan(0);

            // Check that 'test' function appears in completions
            const testCompletion = completions.find((c) => c.label === 'test');
            expect(testCompletion).toBeDefined();
            expect(testCompletion.detail).toContain('(x: int) -> int');
            expect(testCompletion.kind).toBe(1); // Monaco Function kind (LSP's is 3)
        });
    });

    describe('hover', () => {
        it('should return type information for expressions', () => {
            if (!wasmAvailable) return;
            // Set active file to main.py
            pyreService.setActiveFile('main.py');
            // Position of "test(42)" in reveal_type on line 6
            const hoverInfo = pyreService.hover(6, 13);

            expect(hoverInfo).toBeDefined();
            expect(hoverInfo.contents).toBeDefined();
            expect(hoverInfo.contents).toHaveLength(1);

            const hoverInfoContent = hoverInfo.contents[0];
            expect(hoverInfoContent.value).toEqual(
                '```python\n(function) test: def test(x: int) -> int: ...\n```'
            );
        });

        it('should return type information for None', () => {
            if (!wasmAvailable) return;
            pyreService.updateSingleFile('main.py', 'None\n');

            const hoverInfo = pyreService.hover(1, 1);

            expect(hoverInfo.contents[0].value).toEqual(
                '```python\n(class) NoneType: None\n```\n---\n' +
                    '```python\n(keyword) None\n```\n---\n' +
                    'The singleton value used to represent the absence of a value.\n\n' +
                    '[Python language reference](https://docs.python.org/3/library/constants.html#None)'
            );
        });
    });

    describe('inlayHint', () => {
        it('should return inlay hints', () => {
            if (!wasmAvailable) return;
            pyreService.setActiveFile('main.py');
            const hints = pyreService.inlayHint(false);
            expect(hints).toBeDefined();
            expect(hints.length).toBeGreaterThan(0);

            // Check the first hint
            const firstHint = hints[0];
            expect(firstHint.position).toEqual({ lineNumber: 3, column: 17 });
            expect(firstHint.label).toEqual(' -> int');
        });

        it('should return parameter name hints when enabled', () => {
            if (!wasmAvailable) return;
            pyreService.updateSingleFile(
                'main.py',
                'def f(value: int) -> None:\n    pass\n\nf(1)\n'
            );

            expect(
                pyreService
                    .inlayHint(false)
                    .some((hint: any) => hint.label === 'value= ')
            ).toBe(false);
            expect(
                pyreService
                    .inlayHint(true)
                    .some((hint: any) => hint.label === 'value= ')
            ).toBe(true);
        });
    });

    describe('semanticTokens', () => {
        it('should return tokens that use the legend', () => {
            if (!wasmAvailable) return;
            pyreService.setActiveFile('main.py');
            const legend = pyreService.semanticTokensLegend();
            expect(legend.tokenTypes).toContain('function');
            expect(
                pyreService.semanticTokens(null).data.length
            ).toBeGreaterThan(0);
        });
    });

    describe('bindings', () => {
        it('should return the Pyrefly version', async () => {
            if (!wasmAvailable) return;
            const pyreflyWasm = await createPyreflyWasmModule();
            expect(pyreflyWasm.version()).toMatch(/^\d+\.\d+\.\d+/);
        });

        it('should throw for an unsupported Python version', async () => {
            if (!wasmAvailable) return;
            const pyreflyWasm = await createPyreflyWasmModule();
            expect(() => new pyreflyWasm.State('2.7')).toThrow('not supported');
        });

        it('should keep each state separate', async () => {
            if (!wasmAvailable) return;
            const pyreflyWasm = await createPyreflyWasmModule();
            const other = new pyreflyWasm.State('3.12');
            other.updateSandboxFiles({ 'main.py': 'x: int = ""\n' }, true);
            other.setActiveFile('main.py');
            expect(other.getErrors().length).toBe(1);
            pyreService.setActiveFile('main.py');
            expect(
                findError(pyreService.getErrors(), 'is not assignable to `int`')
            ).toBeUndefined();
        });

        it('should return ENOSYS for an unimplemented WASI import', async () => {
            if (!wasmAvailable) return;
            const compile = jest
                .spyOn(WebAssembly, 'compile')
                .mockResolvedValue({} as WebAssembly.Module);
            const instantiate = jest
                .spyOn(WebAssembly, 'instantiate')
                .mockResolvedValue({ exports: {} } as any);
            try {
                // A fresh copy of the bindings, because `init` instantiates only once.
                await jest.isolateModulesAsync(async () => {
                    const pyreflyWasm = (await import(
                        './wasm/pyrefly_wasm_for_testing'
                    )) as unknown as {
                        default: (source: Uint8Array) => Promise<void>;
                    };
                    await pyreflyWasm.default(new Uint8Array());
                });
                const wasi = instantiate.mock.calls[0][1]!
                    .wasi_snapshot_preview1 as Record<string, () => number>;
                // ERRNO_NOSYS
                expect(wasi.args_get()).toBe(52);
                // ERRNO_BADF, which ends the preopened directory scan in wasi-libc.
                expect(wasi.fd_prestat_get()).toBe(8);
            } finally {
                compile.mockRestore();
                instantiate.mockRestore();
            }
        });

        it('should omit unset completion fields', () => {
            if (!wasmAvailable) return;
            pyreService.updateSingleFile('main.py', 'value = 1\nval');
            pyreService.setActiveFile('main.py');
            const items = pyreService.autoComplete(2, 4);
            const value = items.find((item: any) => item.label === 'value');
            expect(value).toBeDefined();
            expect('additionalTextEdits' in value).toBe(false);
            expect(
                items.every((item: any) => !Object.values(item).includes(null))
            ).toBe(true);
        });

        it('should open each playground again after a crash', async () => {
            if (!wasmAvailable) return;
            const pyreflyWasm = await createPyreflyWasmModule();
            const state = new pyreflyWasm.State('3.12');
            state.updateSandboxFiles({ 'main.py': 'x: int = ""\n' }, true);
            state.setActiveFile('main.py');
            // Nesting this deep overflows the stack.
            const deep = `x = ${'['.repeat(5000)}${']'.repeat(5000)}\n`;
            expect(() => state.updateSingleFile('main.py', deep)).toThrow();
            expect(() => state.getErrors()).toThrow('not ready');

            // Waits for the new instance.
            await createPyreflyWasmModule();
            // The playground opens with the last input that did not crash.
            expect(state.getErrors().length).toBe(1);

            // New input after a crash replaces the files of the last input
            // that did not crash.
            expect(() => state.updateSingleFile('main.py', deep)).toThrow();
            await createPyreflyWasmModule();
            state.updateSingleFile('main.py', 'y: str = 1\nz: int = ""\n');
            expect(state.getErrors().length).toBe(2);

            pyreService.setActiveFile('main.py');
            expect(
                findError(pyreService.getErrors(), 'revealed type:')
            ).toBeDefined();
        });

        it('should check text with a lone surrogate', () => {
            if (!wasmAvailable) return;
            pyreService.updateSingleFile(
                'main.py',
                'from typing import reveal_type\nreveal_type("\ud800")\n'
            );
            pyreService.setActiveFile('main.py');
            expect(
                findError(pyreService.getErrors(), "Literal['\ufffd']")
            ).toBeDefined();
        });

        it('should convert arguments as wasm-bindgen does', () => {
            if (!wasmAvailable) return;
            pyreService.setActiveFile('main.py');
            expect(pyreService.hover(NaN, undefined as any)).toBeNull();
            const whole = pyreService.semanticTokens(null).data;
            expect(pyreService.semanticTokens({} as any).data).toEqual(whole);
        });
    });

    it('should check 450 levels of nested lists', () => {
        if (!wasmAvailable) return;
        // With Rust's default 1 MiB wasm stack, this fails with `memory
        // access out of bounds`.
        const depth = 450;
        pyreService.updateSingleFile(
            'main.py',
            `x = ${'['.repeat(depth)}${']'.repeat(depth)}\n`
        );
        pyreService.setActiveFile('main.py');
        expect(pyreService.getErrors()).toEqual([]);
    });
});
