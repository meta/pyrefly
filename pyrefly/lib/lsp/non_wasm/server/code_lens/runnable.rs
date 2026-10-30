/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Detection and rendering of Run and Test CodeLens commands.

use std::path::Path;

use lsp_types::CodeLens;
use lsp_types::Command;
use lsp_types::Range;
use lsp_types::Uri;
use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_name::ModuleNameWithKind;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::telemetry::EmptyResponseReason;
use serde_json::Value;

use crate::lsp::non_wasm::runnable::RunnableEntry;
use crate::lsp::non_wasm::runnable::RunnableKind;
use crate::lsp::non_wasm::runnable::runnable_entries;
use crate::lsp::non_wasm::server::Server;
use crate::state::state::Transaction;

/// Collect and render all enabled runnable lenses for one Python file.
pub(super) fn code_lenses(
    server: &Server,
    transaction: &Transaction<'_>,
    handle: &Handle,
    uri: &Uri,
) -> Result<Vec<CodeLens>, EmptyResponseReason> {
    let path = server
        .path_for_uri(uri)
        .ok_or(EmptyResponseReason::NoFilePath)?;
    let runnable_code_lens = server
        .workspaces
        .get_with(path.clone(), |(_, workspace)| workspace.runnable_code_lens);
    let info = transaction
        .get_module_info(handle)
        .ok_or(EmptyResponseReason::ModuleInfoNotFound)?;
    if !runnable_code_lens || uri.path().ends_with(".pyi") || uri.path().ends_with(".ipynb") {
        return Ok(Vec::new());
    }
    let ast = transaction
        .get_ast(handle)
        .ok_or(EmptyResponseReason::ModuleInfoNotFound)?;
    let entries = runnable_entries(&ast.body);
    let config = server.state.config_finder().python_file(
        ModuleNameWithKind::guaranteed(ModuleName::unknown()),
        &ModulePath::filesystem(path.clone()),
    );
    let cwd = config
        .source
        .root_from_file()
        .map(Path::to_path_buf)
        .or_else(|| {
            server
                .workspaces
                .get_with(path, |(workspace_root, _)| workspace_root.cloned())
        })
        .map(|path| path.to_string_lossy().into_owned());

    let mut lenses = Vec::new();
    for entry in entries {
        let range = info.to_lsp_range(entry.range);
        lenses.push(entry_to_lsp_code_lens(uri, range, entry, cwd.as_deref()));
    }
    Ok(lenses)
}

/// Render a semantic runnable entry as an executable LSP command.
fn entry_to_lsp_code_lens(
    uri: &Uri,
    range: Range,
    entry: RunnableEntry,
    cwd: Option<&str>,
) -> CodeLens {
    let (title, command, arguments) = match entry.kind {
        RunnableKind::Run => {
            let mut args = serde_json::Map::new();
            args.insert("uri".to_owned(), serde_json::json!(uri.to_string()));
            if let Some(cwd) = cwd {
                args.insert("cwd".to_owned(), serde_json::json!(cwd));
            }
            ("Run", "pyrefly.runMain", Some(vec![Value::Object(args)]))
        }
        RunnableKind::Test => {
            let mut args = serde_json::Map::new();
            args.insert("uri".to_owned(), serde_json::json!(uri.to_string()));
            if let Some(cwd) = cwd {
                args.insert("cwd".to_owned(), serde_json::json!(cwd));
            }
            args.insert(
                "position".to_owned(),
                serde_json::json!({
                    "line": range.start.line,
                    "character": range.start.character,
                }),
            );
            if let Some(test_name) = entry.test_name {
                args.insert("testName".to_owned(), serde_json::json!(test_name));
            }
            if let Some(class_name) = entry.class_name {
                args.insert("className".to_owned(), serde_json::json!(class_name));
            }
            args.insert(
                "isUnittest".to_owned(),
                serde_json::json!(entry.is_unittest),
            );
            ("Test", "pyrefly.runTest", Some(vec![Value::Object(args)]))
        }
    };

    CodeLens {
        range,
        command: Some(Command {
            title: title.to_owned(),
            command: command.to_owned(),
            arguments,
            tooltip: None,
        }),
        data: None,
    }
}
