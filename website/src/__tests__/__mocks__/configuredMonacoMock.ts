/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Mock implementation of configured-monaco module for testing

import { SANDBOX_FILE_NAME } from '../../pages/sandbox';

/**
 * Mock editor model interface
 */
interface MockUri {
    scheme: string;
    path: string;
}

interface MockEditorModel {
    uri: MockUri;
    getValue: () => string;
    setValue: (value: string) => void;
    updateOptions: (options: any) => void;
    dispose: () => void;
    onDidChangeContent: (listener: any) => { dispose: () => void };
}

/**
 * Monaco editor mock
 */
const mockModels = new Map<string, MockEditorModel>();
const uriKey = (uri: MockUri) => `${uri.scheme}:${uri.path}`;

const monaco = {
    editor: {
        onDidCreateModel: jest.fn(),
        setModelMarkers: jest.fn(),
        getModels: (): MockEditorModel[] => Array.from(mockModels.values()),
        getModel: (uri: MockUri): MockEditorModel | null =>
            mockModels.get(uriKey(uri)) ?? null,
        createModel: (content: string, language: string, uri?: MockUri): MockEditorModel => {
            const modelUri = uri ?? { scheme: 'file', path: '/mock-file.py' };
            const model: MockEditorModel = {
                uri: modelUri,
                getValue: () => content,
                setValue: jest.fn((value: string) => { content = value; }),
                updateOptions: jest.fn(),
                dispose: jest.fn(),
                onDidChangeContent: jest.fn(() => ({ dispose: jest.fn() })),
            };
            mockModels.set(uriKey(modelUri), model);
            return model;
        },
    },
    Uri: {
        file: (path: string): MockUri => ({ scheme: 'file', path }),
        from: (components: MockUri): MockUri => components,
    },
    languages: {
        register: jest.fn(),
        registerCompletionItemProvider: jest.fn(),
        registerDefinitionProvider: jest.fn(),
        registerHoverProvider: jest.fn(),
        registerInlayHintsProvider: jest.fn(),
        registerDocumentSemanticTokensProvider: jest.fn(),
        registerDocumentSemanticTokensLegendProvider: jest.fn(),
    },
};

/**
 * Sets the auto-complete function for Monaco
 * @param _model The editor model
 * @param _autoCompleteFunction The auto-complete function
 */
function setAutoCompleteFunction(
    _model: MockEditorModel,
    _autoCompleteFunction: (line: number, column: number) => any
): void {}

/**
 * Sets the go-to-definition function for Monaco
 * @param _model The editor model
 * @param _getDefFunction The go-to-definition function
 */
function setGetDefFunction(
    _model: MockEditorModel,
    _getDefFunction: (line: number, column: number) => any[] | null
): void {}

/**
 * Sets the hover function for Monaco
 * @param _model The editor model
 * @param _hoverFunction The hover function
 */
function setHoverFunctionForMonaco(
    _model: MockEditorModel,
    _hoverFunction: (line: number, column: number) => any
): void {}

/**
 * Sets the inlay hint function for Monaco
 * @param _model The editor model
 * @param _inlayHintFunction The inlay hint function
 */
function setInlayHintFunctionForMonaco(
    _model: MockEditorModel,
    _inlayHintFunction: () => any
): void {}

/**
 * Sets the semantic tokens function for Monaco
 * @param _model The editor model
 * @param _semanticTokensFunction The semantic tokens function
 */
function setSemanticTokensFunctionForMonaco(
    _model: MockEditorModel,
    _semanticTokensFunction: (range: any) => any
): void {}

/**
 * Sets the semantic tokens legend function for Monaco
 * @param _semanticTokensLegendFunction The semantic tokens legend function
 */
function setSemanticTokensLegendForMonaco(
    _semanticTokensLegendFunction: () => any
): void {}

module.exports = {
    __esModule: true,
    monaco,
    setAutoCompleteFunction,
    setGetDefFunction,
    setHoverFunctionForMonaco,
    setInlayHintFunctionForMonaco,
    setSemanticTokensFunctionForMonaco,
    setSemanticTokensLegendForMonaco
};
