/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

import * as assert from 'assert';
import {promises as fs} from 'fs';
import {tmpdir} from 'os';
import {join} from 'path';
import * as vscode from 'vscode';

suite('Extension Test Suite', () => {
  const extension: vscode.Extension<unknown> | undefined = vscode.extensions.getExtension('meta.pyrefly');

  test('Test activation', async function () {
    // On macos-13, we've noticed successful test activation take up to 3500ms.
    this.timeout(10000);
    await extension?.activate();
    assert.ok(true);
  });

  test('Infer types in the current file', async function () {
    this.timeout(30000);
    assert.ok(extension);
    await extension.activate();

    const directory = await fs.mkdtemp(join(tmpdir(), 'pyrefly-infer-'));
    const uri = vscode.Uri.file(join(directory, 'test.py'));
    try {
      await vscode.workspace.fs.writeFile(
        uri,
        Buffer.from('def foo():\n    return 1\n'),
      );
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document);

      await vscode.commands.executeCommand('pyrefly.infer');

      const result = Buffer.from(
        await vscode.workspace.fs.readFile(uri),
      ).toString();
      assert.strictEqual(result, 'def foo() -> int:\n    return 1\n');
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
      await fs.rm(directory, {recursive: true, force: true});
    }
  });

  test('Show reference CodeLens arguments through the VS Code command', async function () {
    this.timeout(30000);
    assert.ok(extension);
    await extension.activate();

    const directory = await fs.mkdtemp(join(tmpdir(), 'pyrefly-references-'));
    const uri = vscode.Uri.file(join(directory, 'references.py'));
    try {
      await vscode.workspace.fs.writeFile(
        uri,
        Buffer.from('def foo(): pass\nfoo()\n'),
      );
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document);
      // The built-in command rejects the serialized URI and position without conversion.
      await vscode.commands.executeCommand(
        'pyrefly.showReferences',
        uri.toString(),
        {line: 0, character: 4},
        [{
          uri: uri.toString(),
          range: {
            start: {line: 1, character: 0},
            end: {line: 1, character: 3},
          },
        }],
      );
    } finally {
      await vscode.commands.executeCommand('closeReferenceSearch');
      await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
      await fs.rm(directory, {recursive: true, force: true});
    }
  });
});
