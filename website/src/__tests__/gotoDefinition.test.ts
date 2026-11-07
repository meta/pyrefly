/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import { monaco } from '../sandbox/configured-monaco';
import {
    gotoDefinition,
    type PyreflyLocation,
} from '../sandbox/gotoDefinition';

const range = {
    startLineNumber: 3,
    startColumn: 5,
    endLineNumber: 3,
    endColumn: 6,
};

// A Pyrefly state with the sandbox file `utils.py`, whose definitions at
// every position are in `utils.py` and in the stub `typeshed/stdlib/os.pyi`.
function fakeState() {
    const calls: boolean[] = [];
    return {
        calls,
        gotoDefinition(
            _line: number,
            _column: number,
            withContents: boolean
        ): PyreflyLocation[] {
            calls.push(withContents);
            return [
                { filename: 'utils.py', range },
                {
                    filename: 'typeshed/stdlib/os.pyi',
                    range,
                    ...(withContents ? { contents: 'def getcwd(): ...' } : {}),
                },
            ];
        },
    };
}

test('asks for the text of a stub only until the stub has a model', () => {
    monaco.editor.createModel('', 'python', monaco.Uri.file('/utils.py'));
    const state = fakeState();

    expect(gotoDefinition(state, 1, 1)).toEqual([
        { uri: { scheme: 'file', path: '/utils.py' }, range },
        {
            uri: { scheme: 'pyrefly-bundled', path: '/typeshed/stdlib/os.pyi' },
            range,
        },
    ]);
    expect(state.calls).toEqual([false, true]);
    const stub = monaco.editor.getModel(
        monaco.Uri.from({
            scheme: 'pyrefly-bundled',
            path: '/typeshed/stdlib/os.pyi',
        })
    );
    expect(stub?.getValue()).toBe('def getcwd(): ...');
    expect(
        monaco.editor.getModel(monaco.Uri.file('/typeshed/stdlib/os.pyi'))
    ).toBeNull();

    gotoDefinition(state, 1, 1);
    expect(state.calls).toEqual([false, true, false]);
});
