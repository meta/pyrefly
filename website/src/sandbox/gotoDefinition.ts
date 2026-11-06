/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import { monaco } from './configured-monaco';

// A definition. `contents` is the text of a file that is not a sandbox file,
// such as a typeshed stub, and is set only when the call asks for it.
export interface PyreflyLocation {
    filename: string;
    range: monaco.IRange;
    contents?: string;
}

function bundledUri(filename: string): monaco.Uri {
    return monaco.Uri.from({ scheme: 'pyrefly-bundled', path: `/${filename}` });
}

// Returns the Monaco locations of the definitions at a position. A file that
// is not a sandbox file gets a model of its own, so that Monaco can preview the
// definition. Monaco asks for definitions each time the pointer moves with Ctrl
// held, so the text of such a file is asked for only when it has no model yet.
export function gotoDefinition(
    pyreService: {
        gotoDefinition: (
            line: number,
            column: number,
            withContents: boolean
        ) => ReadonlyArray<PyreflyLocation> | null;
    },
    line: number,
    column: number
): monaco.languages.Location[] | null {
    const isSandboxFile = (filename: string) =>
        monaco.editor.getModel(monaco.Uri.file(`/${filename}`)) !== null;
    let locations = pyreService.gotoDefinition(line, column, false);
    if (
        locations?.some(
            ({ filename }) =>
                !isSandboxFile(filename) &&
                monaco.editor.getModel(bundledUri(filename)) === null
        )
    ) {
        locations = pyreService.gotoDefinition(line, column, true);
    }
    return (
        locations?.map(({ filename, range, contents }) => {
            if (isSandboxFile(filename)) {
                return { uri: monaco.Uri.file(`/${filename}`), range };
            }
            const uri = bundledUri(filename);
            if (monaco.editor.getModel(uri) === null && contents !== undefined) {
                monaco.editor.createModel(contents, 'python', uri);
            }
            return { uri, range };
        }) ?? null
    );
}
