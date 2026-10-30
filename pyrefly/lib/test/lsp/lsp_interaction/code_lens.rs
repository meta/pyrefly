/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use lsp_server::ErrorCode;
use lsp_types::CodeLens;
use lsp_types::CodeLensOptions;
use lsp_types::CodeLensRefreshRequest;
use lsp_types::CodeLensRequest;
use lsp_types::CodeLensResolveRequest;
use lsp_types::Location;
use lsp_types::Position;
use lsp_types::Range;
use lsp_types::RenameRequest;
use lsp_types::SymbolInformation;
use lsp_types::Uri;
use pyrefly_lsp_test::IndexingMode;
use pyrefly_lsp_test::LspArgs;
use pyrefly_lsp_test::Message;
use pyrefly_lsp_test::object_model::CellKind;
use pyrefly_lsp_test::object_model::ExternalProvider;
use pyrefly_lsp_test::object_model::InitializeSettings;
use pyrefly_lsp_test::object_model::LspInteraction;
use pyrefly_lsp_test::object_model::LspInteractionArgs;
use pyrefly_lsp_test::object_model::TestTelemetry;
use pyrefly_util::telemetry::SubTaskTelemetry;
use pyrefly_util::telemetry::TelemetryEventKind;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

use crate::lsp::non_wasm::code_lens::OverrideCodeLensRequest;
use crate::lsp::non_wasm::code_lens::OverrideCodeLensTargets;
use crate::test::lsp::lsp_interaction::util::get_test_files_root;
use crate::test::lsp::lsp_interaction::util::wait_for_disk_invalidation;

fn runnable_code_lens_config() -> serde_json::Value {
    json!([{
        "pyrefly": {
            "runnableCodeLens": true
        }
    }])
}

fn indexed_interaction(root_path: &Path) -> LspInteraction {
    let scope_uri = Uri::from_file_path(root_path).unwrap();
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        args: LspArgs {
            indexing_mode: IndexingMode::LazyBlocking,
            ..LspInteractionArgs::default().args
        },
        ..Default::default()
    });
    interaction.set_root(root_path.to_path_buf());
    interaction
        .initialize(InitializeSettings {
            workspace_folders: Some(vec![("test".to_owned(), scope_uri)]),
            initialization_options: Some(json!({"pyrefly": {
                "supportsReferencesCodeLens": true,
                "referencesCodeLens": true
            }})),
            ..Default::default()
        })
        .unwrap();
    interaction
}

fn request_code_lenses(interaction: &LspInteraction, uri: Uri) -> Vec<CodeLens> {
    let lenses = RefCell::new(Vec::new());
    interaction
        .client
        .send_request::<CodeLensRequest>(json!({
            "textDocument": {
                "uri": uri
            }
        }))
        .expect_response_with(|response| {
            lenses.replace(response.unwrap_or_default());
            true
        })
        .unwrap();
    lenses.into_inner()
}

fn resolve_code_lens(interaction: &LspInteraction, lens: CodeLens) -> CodeLens {
    let resolved = RefCell::new(None);
    interaction
        .client
        .send_request::<CodeLensResolveRequest>(serde_json::to_value(lens).unwrap())
        .expect_response_with(|lens| {
            resolved.replace(Some(lens));
            true
        })
        .unwrap();
    resolved.into_inner().unwrap()
}

#[test]
fn test_code_lens_advertises_resolve_provider() {
    let interaction = LspInteraction::new();
    interaction
        .client
        .send_initialize(
            interaction
                .client
                .get_initialize_params(&InitializeSettings::default()),
        )
        .expect_response_with(|response| {
            response.capabilities.code_lens_provider
                == Some(CodeLensOptions {
                    resolve_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })
        })
        .unwrap();
    interaction.client.send_initialized();
    interaction.shutdown().unwrap();
}

#[test]
fn test_code_lens_for_tests_and_main() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    let test_root = root.path().join("code_lens");
    interaction.set_root(test_root.clone());
    interaction
        .initialize(InitializeSettings {
            configuration: Some(Some(runnable_code_lens_config())),
            ..Default::default()
        })
        .unwrap();

    interaction.client.did_open("main_and_tests.py");

    let path = test_root.join("main_and_tests.py");
    let uri = Uri::from_file_path(&path).unwrap();

    interaction
        .client
        .send_request::<CodeLensRequest>(json!({
            "textDocument": {
                "uri": uri.to_string()
            },
        }))
        .expect_response_with(|response: Option<Vec<CodeLens>>| {
            let Some(lenses) = response else {
                return false;
            };

            let mut has_run = false;
            let mut pytest_test = false;
            let mut unittest_test = false;
            let mut top_level_test = false;
            for lens in lenses {
                let Some(command) = lens.command else {
                    continue;
                };
                if command.command == "pyrefly.runMain" && command.title == "Run" {
                    has_run |= lens.range.start.line == 26;
                }
                if command.command == "pyrefly.runTest" && command.title == "Test" {
                    let args = command.arguments.clone().unwrap_or_default();
                    let Some(Value::Object(obj)) = args.first() else {
                        continue;
                    };
                    let is_unittest = obj
                        .get("isUnittest")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let test_name = obj.get("testName").and_then(Value::as_str);
                    let class_name = obj.get("className").and_then(Value::as_str);
                    match (is_unittest, class_name, test_name) {
                        (false, Some("TestPytest"), Some("test_method")) => pytest_test = true,
                        (true, Some("MyTestCase"), Some("test_unittest")) => unittest_test = true,
                        (false, None, Some("test_top_level")) => top_level_test = true,
                        _ => {}
                    }
                }
            }

            has_run && pytest_test && unittest_test && top_level_test
        })
        .unwrap();

    interaction.shutdown().unwrap();
}

#[test]
fn test_code_lens_uses_config_root_for_cwd() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    let test_root = root.path().join("code_lens");
    interaction.set_root(test_root.clone());
    interaction
        .initialize(InitializeSettings {
            configuration: Some(Some(runnable_code_lens_config())),
            ..Default::default()
        })
        .unwrap();

    interaction
        .client
        .did_open("nested_project/main_and_tests.py");

    let path = test_root.join("nested_project/main_and_tests.py");
    let uri = Uri::from_file_path(&path).unwrap();
    let expected_cwd = test_root
        .join("nested_project")
        .to_string_lossy()
        .into_owned();

    interaction
        .client
        .send_request::<CodeLensRequest>(json!({
            "textDocument": {
                "uri": uri.to_string()
            },
        }))
        .expect_response_with(|response: Option<Vec<CodeLens>>| {
            let Some(lenses) = response else {
                return false;
            };
            let mut saw_lens = false;
            lenses.into_iter().all(|lens| {
                saw_lens = true;
                lens.command
                    .and_then(|command| command.arguments)
                    .and_then(|args| args.into_iter().next())
                    .and_then(|arg| arg.get("cwd").and_then(Value::as_str).map(str::to_owned))
                    .is_some_and(|cwd| cwd == expected_cwd)
            }) && saw_lens
        })
        .unwrap();

    interaction.shutdown().unwrap();
}

#[test]
fn test_code_lens_ignores_stub_files() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    let test_root = root.path().join("code_lens");
    interaction.set_root(test_root.clone());
    interaction
        .initialize(InitializeSettings {
            configuration: Some(Some(runnable_code_lens_config())),
            ..Default::default()
        })
        .unwrap();

    interaction.client.did_open("main_and_tests.pyi");

    let path = test_root.join("main_and_tests.pyi");
    let uri = Uri::from_file_path(&path).unwrap();

    interaction
        .client
        .send_request::<CodeLensRequest>(json!({
            "textDocument": {
                "uri": uri.to_string()
            },
        }))
        .expect_response_with(|response: Option<Vec<CodeLens>>| {
            response.is_some_and(|lenses| lenses.is_empty())
        })
        .unwrap();

    interaction.shutdown().unwrap();
}

#[test]
fn test_code_lens_disabled_by_default() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    let test_root = root.path().join("code_lens");
    interaction.set_root(test_root.clone());
    interaction
        .initialize(InitializeSettings {
            configuration: Some(Some(json!([{}]))),
            ..Default::default()
        })
        .unwrap();

    interaction.client.did_open("main_and_tests.py");

    let path = test_root.join("main_and_tests.py");
    let uri = Uri::from_file_path(&path).unwrap();

    interaction
        .client
        .send_request::<CodeLensRequest>(json!({
            "textDocument": {
                "uri": uri.to_string()
            },
        }))
        .expect_response_with(|response: Option<Vec<CodeLens>>| {
            response.is_some_and(|lenses| lenses.is_empty())
        })
        .unwrap();

    interaction.shutdown().unwrap();
}

#[test]
fn test_override_code_lens_batch_and_document_version() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    let test_root = root.path().join("code_lens");
    interaction.set_root(test_root.clone());
    interaction.initialize(Default::default()).unwrap();
    let uri = Uri::from_file_path(test_root.join("overrides.py")).unwrap();
    interaction.client.did_open_uri(
        &uri,
        "python",
        "class Base:\n    def f(self): pass\nclass Child(Base):\n    def f(self): pass\n",
    );
    let params = json!({
        "textDocument": {"uri": uri.to_string(), "version": 1},
        "ranges": [
            {"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 9}},
            {"start": {"line": 3, "character": 8}, "end": {"line": 3, "character": 9}}
        ]
    });
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params.clone())
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 2
                && targets[0].parents.is_empty()
                && targets[0].implementations.len() == 1
                && targets[0].implementations[0].location.range.start.line == 3
                && targets[0].implementations[0].name == "Child.f"
                && targets[1].parents.len() == 1
                && targets[1].parents[0].location.range.start.line == 1
                && targets[1].parents[0].name == "Base.f"
                && targets[1].implementations.is_empty()
        })
        .unwrap();
    let mut stale_params = params;
    stale_params["textDocument"]["version"] = json!(0);
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(stale_params)
        .expect_response_error_code(-32800)
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_override_code_lens_notebook_cell_versions() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    interaction.set_root(root.path().to_path_buf());
    interaction.initialize(Default::default()).unwrap();
    interaction.open_notebook(
        "overrides.ipynb",
        vec!["class Base:\n    def f(self): pass\nclass Child(Base):\n    def f(self): pass\n"],
    );
    let uri = interaction.cell_uri("overrides.ipynb", "cell1");
    let mut params = json!({
        "textDocument": {"uri": uri.to_string(), "version": 1},
        "ranges": [{"start": {"line": 3, "character": 8}, "end": {"line": 3, "character": 9}}]
    });
    for notebook_version in [1, 2] {
        if notebook_version == 2 {
            interaction.change_notebook(
                "overrides.ipynb",
                2,
                json!({"metadata": {"language_info": {"name": "python"}}}),
            );
        }
        interaction
            .client
            .send_request::<OverrideCodeLensRequest>(params.clone())
            .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
                targets.len() == 1
                    && targets[0].parents.len() == 1
                    && targets[0].parents[0].name == "Base.f"
                    && targets[0].parents[0].location.uri == uri
                    && targets[0].parents[0].location.range.start.line == 1
            })
            .unwrap();
    }
    // The cell text version changes independently of the notebook version.
    interaction.change_notebook("overrides.ipynb", 3, json!({"cells": {"textContent": [{
        "document": {"uri": uri.to_string(), "version": 7},
        "changes": [{"text": "class Base:\n    def f(self): pass\nclass Child(Base):\n    def f(self): pass\n"}]
    }]}}));
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params.clone())
        .expect_response_error_code(-32800)
        .unwrap();
    params["textDocument"]["version"] = json!(7);
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params)
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1 && targets[0].parents.len() == 1
        })
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_override_code_lens_unsaved_notebook() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    interaction.set_root(root.path().to_path_buf());
    interaction.initialize(Default::default()).unwrap();
    interaction.open_notebook_with_uri(
        "untitled:Untitled-Overrides.ipynb",
        "jupyter-notebook",
        "unsaved_overrides.ipynb",
        vec![(
            CellKind::Code,
            "class Base:\n    def f(self): pass\nclass Child(Base):\n    def f(self): pass\n",
        )],
    );
    let uri = interaction.cell_uri("unsaved_overrides.ipynb", "cell1");
    let mut params = json!({
        "textDocument": {"uri": uri.to_string(), "version": 1},
        "ranges": [{"start": {"line": 3, "character": 8}, "end": {"line": 3, "character": 9}}]
    });
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params.clone())
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1
                && targets[0].parents.len() == 1
                && targets[0].parents[0].name == "Base.f"
                && targets[0].parents[0].location.uri == uri
                && targets[0].parents[0].location.range.start.line == 1
        })
        .unwrap();
    params["textDocument"]["version"] = json!(0);
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params)
        .expect_response_error_code(-32800)
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_override_code_lens_mixed_notebook_cells() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    interaction.set_root(root.path().to_path_buf());
    interaction.initialize(Default::default()).unwrap();
    interaction.open_notebook_with_kinds(
        "mixed_overrides.ipynb",
        vec![
            (CellKind::Markdown, "# Models"),
            (CellKind::Code, "class Base:\n    def f(self): pass\n"),
            (
                CellKind::Code,
                "class Child(Base):\n    def f(self): pass\n",
            ),
        ],
    );
    let base_uri = interaction.cell_uri("mixed_overrides.ipynb", "cell2");
    let child_uri = interaction.cell_uri("mixed_overrides.ipynb", "cell3");
    let ranges = json!([{
        "start": {"line": 1, "character": 8},
        "end": {"line": 1, "character": 9}
    }]);
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(json!({
            "textDocument": {"uri": base_uri.to_string(), "version": 1},
            "ranges": ranges
        }))
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1
                && targets[0].implementations.len() == 1
                && targets[0].implementations[0].name == "Child.f"
                && targets[0].implementations[0].location.uri == child_uri
                && targets[0].implementations[0].location.range.start.line == 1
        })
        .unwrap();
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(json!({
            "textDocument": {"uri": child_uri.to_string(), "version": 1},
            "ranges": ranges
        }))
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1
                && targets[0].parents.len() == 1
                && targets[0].parents[0].name == "Base.f"
                && targets[0].parents[0].location.uri == base_uri
                && targets[0].parents[0].location.range.start.line == 1
        })
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_override_code_lens_refresh_after_subclass_change() {
    let root = get_test_files_root();
    let mut interaction = LspInteraction::new();
    let test_root = root.path().join("code_lens");
    interaction.set_root(test_root.clone());
    interaction.initialize(Default::default()).unwrap();
    let uri = Uri::from_file_path(test_root.join("base.py")).unwrap();
    interaction
        .client
        .did_open_uri(&uri, "python", "class Base:\n    def f(self): pass\n");
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(json!({
            "textDocument": {"uri": uri.to_string(), "version": 1},
            "refreshSupport": true,
            "ranges": [{"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 9}}]
        }))
        .expect_response_with(|_: Vec<OverrideCodeLensTargets>| true)
        .unwrap();
    let child_uri = Uri::from_file_path(test_root.join("child.py")).unwrap();
    interaction.client.did_open_uri(
        &child_uri,
        "python",
        "from base import Base\nclass Child(Base):\n    def f(self): pass\n",
    );
    interaction.client.did_change(
        "child.py",
        "from base import Base\nclass Child(Base):\n    def f(self): return 1\n",
    );
    interaction
        .client
        .expect_message("override CodeLens invalidation", |message| match message {
            Message::Notification(notification)
                if notification.method == "pyrefly/overrideCodeLens/refresh" =>
            {
                Some(Ok(()))
            }
            _ => None,
        })
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_override_code_lens_refresh_after_indexed_file_change() {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("pyrefly.toml"),
        "project-includes = ['*.py']\n",
    )
    .unwrap();
    fs::write(
        root.path().join("base.py"),
        "class Base:\n    def f(self): pass\n",
    )
    .unwrap();
    let child = root.path().join("child.py");
    fs::write(
        &child,
        "from base import Base\nclass Child(Base):\n    def f(self): pass\n",
    )
    .unwrap();
    let uri = Uri::from_file_path(root.path().join("base.py")).unwrap();
    let scope = Uri::from_file_path(root.path()).unwrap();
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        args: LspArgs {
            indexing_mode: IndexingMode::LazyBlocking,
            ..LspInteractionArgs::default().args
        },
        ..Default::default()
    });
    interaction.set_root(root.path().to_path_buf());
    interaction
        .initialize(InitializeSettings {
            workspace_folders: Some(vec![("test".to_owned(), scope)]),
            configuration: Some(None),
            ..Default::default()
        })
        .unwrap();
    interaction.client.did_open("base.py");
    let params = json!({
        "textDocument": {"uri": uri.to_string(), "version": 1},
        "refreshSupport": true,
        "ranges": [{"start": {"line": 1, "character": 8}, "end": {"line": 1, "character": 9}}]
    });
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params.clone())
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1 && targets[0].implementations.len() == 1
        })
        .unwrap();
    fs::write(
        &child,
        "from base import Base\nclass Child(Base):\n    def g(self): pass\n",
    )
    .unwrap();
    interaction.client.file_modified("child.py");
    interaction
        .client
        .expect_message(
            "CodeLens invalidation after indexing",
            |message| match message {
                Message::Notification(notification)
                    if notification.method == "pyrefly/overrideCodeLens/refresh" =>
                {
                    Some(Ok(()))
                }
                _ => None,
            },
        )
        .unwrap();
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params)
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1 && targets[0].implementations.is_empty()
        })
        .unwrap();
    interaction.shutdown().unwrap();
}

// https://github.com/meta/pyrefly/issues/5485
#[test]
fn test_override_code_lens_parent_after_defining_file_shifts() {
    let root = TempDir::with_prefix("pyrefly_override_code_lens_parent_shift").unwrap();
    let base = "class Base:\n    def f(self): pass\n";
    fs::write(root.path().join("base.py"), base).unwrap();
    fs::write(
        root.path().join("child.py"),
        "from base import Base\nclass Child(Base):\n    def f(self): pass\n",
    )
    .unwrap();
    let uri = Uri::from_file_path(root.path().join("child.py")).unwrap();

    let telemetry = TestTelemetry::new();
    let telemetry_events = telemetry.subscribe();
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        telemetry: Box::new(telemetry),
        ..Default::default()
    });
    interaction.set_root(root.path().to_path_buf());
    interaction.initialize(Default::default()).unwrap();
    interaction.client.did_open("base.py");
    interaction.client.did_open("child.py");
    let params = json!({
        "textDocument": {"uri": uri.to_string(), "version": 1},
        "ranges": [{"start": {"line": 2, "character": 8}, "end": {"line": 2, "character": 9}}]
    });
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params.clone())
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1
                && targets[0].parents.len() == 1
                && targets[0].parents[0].location.range.start == Position::new(1, 8)
        })
        .unwrap();

    // `child.py` imports the disk copy of `base.py`, so save the edit as an editor does.
    let shifted_base = base.replacen("\n", "\n\n", 1);
    fs::write(root.path().join("base.py"), &shifted_base).unwrap();
    interaction.client.did_change("base.py", &shifted_base);
    interaction.client.did_save("base.py");
    wait_for_disk_invalidation(&telemetry_events);
    interaction
        .client
        .send_request::<OverrideCodeLensRequest>(params)
        .expect_response_with(|targets: Vec<OverrideCodeLensTargets>| {
            targets.len() == 1
                && targets[0].parents.len() == 1
                && targets[0].parents[0].location.range.start == Position::new(2, 8)
        })
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_code_lens_shows_reference_counts() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let symbols_uri = Uri::from_file_path(root_path.join("symbols.py")).unwrap();
    let interaction = indexed_interaction(&root_path);

    interaction.client.did_open("symbols.py");
    interaction.client.did_open("usage.py");

    let lenses = request_code_lenses(&interaction, symbols_uri);
    assert_eq!(lenses.len(), 3);
    assert!(
        lenses
            .iter()
            .all(|lens| lens.command.is_none() && lens.data.is_some())
    );
    for (line, title, count) in [
        (0, "3 references", 3),
        (1, "2 references", 2),
        (4, "2 references", 2),
    ] {
        let lens = lenses
            .iter()
            .find(|lens| lens.range.start.line == line)
            .unwrap()
            .clone();
        let lens = resolve_code_lens(&interaction, lens);
        assert!(has_reference_lens(&lens, line, title, count));
    }

    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_request_from_notebook_is_empty() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let interaction = indexed_interaction(&root_path);
    interaction.open_notebook("notebook.ipynb", vec!["class NotebookSymbol:\n    pass"]);

    let lenses = request_code_lenses(
        &interaction,
        interaction.cell_uri("notebook.ipynb", "cell1"),
    );
    assert!(lenses.is_empty());

    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_excludes_notebook_references() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let symbols_uri = Uri::from_file_path(root_path.join("symbols.py")).unwrap();
    let interaction = indexed_interaction(&root_path);
    interaction.client.did_open("symbols.py");
    interaction.client.did_open("usage.py");
    interaction.open_notebook(
        "notebook.ipynb",
        vec!["from symbols import Greeter", "Greeter()"],
    );
    let notebook_uris = [
        interaction.cell_uri("notebook.ipynb", "cell1"),
        interaction.cell_uri("notebook.ipynb", "cell2"),
    ];

    interaction
        .client
        .references("symbols.py", 0, 7, false)
        .expect_response_with(|locations| {
            let locations = locations.expect("References should be available");
            notebook_uris
                .iter()
                .all(|uri| locations.iter().any(|location| &location.uri == uri))
        })
        .unwrap();

    let lenses = request_code_lenses(&interaction, symbols_uri);
    let greeter_lens = lenses
        .into_iter()
        .find(|lens| lens.range.start.line == 0)
        .unwrap();
    let resolved = resolve_code_lens(&interaction, greeter_lens);
    let locations = reference_locations(&resolved);
    assert_eq!(locations.len(), 3);
    assert!(
        locations
            .iter()
            .all(|location| !notebook_uris.contains(&location.uri))
    );

    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_requires_client_setting_and_indexing() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    for (indexing_mode, supports, enabled) in [
        (IndexingMode::LazyBlocking, true, None),
        (IndexingMode::LazyBlocking, true, Some(false)),
        (IndexingMode::LazyBlocking, false, Some(true)),
        (IndexingMode::None, true, Some(true)),
    ] {
        let mut options = json!({"pyrefly": {"supportsReferencesCodeLens": supports}});
        if let Some(enabled) = enabled {
            options["pyrefly"]["referencesCodeLens"] = json!(enabled);
        }
        let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
            args: LspArgs {
                indexing_mode,
                ..LspInteractionArgs::default().args
            },
            ..Default::default()
        });
        interaction.set_root(root_path.clone());
        interaction
            .initialize(InitializeSettings {
                initialization_options: Some(options),
                ..Default::default()
            })
            .unwrap();
        interaction.client.did_open("symbols.py");
        let uri = Uri::from_file_path(root_path.join("symbols.py")).unwrap();
        assert!(request_code_lenses(&interaction, uri).is_empty());
        interaction.shutdown().unwrap();
    }
}

#[test]
fn test_reference_code_lens_rejects_invalid_data() {
    let root = get_test_files_root();
    let interaction = indexed_interaction(&root.path().join("code_lens_references"));
    for data in [
        Value::Null,
        json!({}),
        json!({"uri": false}),
        json!({"uri": "file:///test.py", "version": "invalid"}),
    ] {
        interaction
            .client
            .send_request::<CodeLensResolveRequest>(json!({
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
                "data": data
            }))
            .expect_response_error_code(ErrorCode::InvalidParams as i32)
            .unwrap();
    }
    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_refreshes_after_another_file_changes() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        args: LspArgs {
            indexing_mode: IndexingMode::LazyBlocking,
            ..LspInteractionArgs::default().args
        },
        ..Default::default()
    });
    interaction.set_root(root_path.clone());
    interaction.initialize(InitializeSettings {
        workspace_folders: Some(vec![("test".to_owned(), Uri::from_file_path(&root_path).unwrap())]),
        capabilities: Some(json!({"workspace": {"codeLens": {"refreshSupport": true}}})),
        initialization_options: Some(json!({"pyrefly": {"supportsReferencesCodeLens": true, "referencesCodeLens": true}})),
        ..Default::default()
    }).unwrap();
    interaction.client.did_open("symbols.py");
    interaction.client.did_open("usage.py");
    let uri = Uri::from_file_path(root_path.join("symbols.py")).unwrap();
    let lens = request_code_lenses(&interaction, uri.clone()).remove(0);
    assert!(has_reference_lens(
        &resolve_code_lens(&interaction, lens),
        0,
        "3 references",
        3
    ));
    interaction
        .client
        .did_change("usage.py", "# No references remain in this file.\n");
    interaction
        .client
        .expect_request::<CodeLensRefreshRequest>(Value::Null)
        .unwrap()
        .send_response(Value::Null);
    let lens = request_code_lenses(&interaction, uri).remove(0);
    assert!(has_reference_lens(
        &resolve_code_lens(&interaction, lens),
        0,
        "1 reference",
        1
    ));
    interaction
        .client
        .references("symbols.py", 0, 7, false)
        .expect_response_with(|locations| locations.is_some_and(|locations| locations.len() == 1))
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_disabling_reference_code_lens_preserves_runnable_lenses() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens");
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        args: LspArgs {
            indexing_mode: IndexingMode::LazyBlocking,
            ..LspInteractionArgs::default().args
        },
        ..Default::default()
    });
    interaction.set_root(root_path.clone());
    interaction.initialize(InitializeSettings {
        initialization_options: Some(json!({"pyrefly": {
            "supportsReferencesCodeLens": true, "referencesCodeLens": false, "runnableCodeLens": true
        }})),
        ..Default::default()
    }).unwrap();
    interaction.client.did_open("main_and_tests.py");
    let lenses = request_code_lenses(
        &interaction,
        Uri::from_file_path(root_path.join("main_and_tests.py")).unwrap(),
    );
    assert!(!lenses.is_empty());
    assert!(
        lenses
            .iter()
            .all(|lens| lens.command.as_ref().is_some_and(|command| {
                matches!(
                    command.command.as_str(),
                    "pyrefly.runMain" | "pyrefly.runTest"
                )
            }))
    );
    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_displays_missing_definition() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let interaction = indexed_interaction(&root_path);
    let uri = Uri::from_file_path(root_path.join("missing_definition.py")).unwrap();
    interaction
        .client
        .did_open_uri(&uri, "python", "def defined():\n    pass\nunknown\n");
    let lens = request_code_lenses(&interaction, uri).remove(0);
    for line in [0, 2] {
        let mut unresolved = lens.clone();
        unresolved.range.start.line = line;
        unresolved.range.start.character = 0;
        unresolved.range.end = unresolved.range.start;
        let resolved = resolve_code_lens(&interaction, unresolved);
        let command = resolved
            .command
            .expect("An unavailable lens must have a visible label");
        assert_eq!(command.title, "References unavailable");
        assert!(command.command.is_empty());
        assert!(resolved.data.is_none());
    }
    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_rejects_old_document_version() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let interaction = indexed_interaction(&root_path);
    interaction.client.did_open("symbols.py");
    let uri = Uri::from_file_path(root_path.join("symbols.py")).unwrap();
    let lens = request_code_lenses(&interaction, uri).remove(0);
    interaction
        .client
        .did_change("symbols.py", "\nclass Different:\n    pass\n");
    interaction
        .client
        .send_request::<CodeLensResolveRequest>(json!(lens))
        .expect_response_error_code(ErrorCode::ContentModified as i32)
        .unwrap();
    interaction.shutdown().unwrap();
}

#[test]
fn test_reference_code_lens_zero_and_one_reference() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let interaction = indexed_interaction(&root_path);
    let uri = Uri::from_file_path(root_path.join("counts.py")).unwrap();
    interaction.client.did_open_uri(
        &uri,
        "python",
        "def unused():\n    pass\ndef used():\n    pass\nused()\n",
    );
    let lenses = request_code_lenses(&interaction, uri);
    assert_eq!(lenses.len(), 2);
    for (lens, (line, title, count)) in lenses
        .into_iter()
        .zip([(0, "0 references", 0), (2, "1 reference", 1)])
    {
        let lens = resolve_code_lens(&interaction, lens);
        assert!(has_reference_lens(&lens, line, title, count));
    }
    interaction.shutdown().unwrap();
}

/// Block the first external search so the test can fill and cancel queued requests.
struct BlockingReferences {
    calls: Sender<String>,
    release: Receiver<()>,
    first: AtomicBool,
    uri: Uri,
}

impl ExternalProvider for BlockingReferences {
    fn find_references(
        &self,
        qualified_name: &str,
        _source_uri: &Uri,
        timeout: Duration,
        _telemetry: Option<SubTaskTelemetry>,
    ) -> anyhow::Result<Vec<(Uri, Vec<Range>)>> {
        self.calls.send(qualified_name.to_owned()).unwrap();
        if self.first.swap(false, Ordering::Relaxed) {
            self.release
                .recv_timeout(timeout)
                .expect("The test must release the first search");
        }
        Ok(vec![(self.uri.clone(), vec![Range::default()])])
    }

    fn workspace_symbols(
        &self,
        _query: &str,
        _workspace_uri: &Uri,
        _timeout: Duration,
        _telemetry: Option<SubTaskTelemetry>,
    ) -> anyhow::Result<Vec<SymbolInformation>> {
        Ok(Vec::new())
    }
}

#[test]
fn test_reference_code_lens_does_not_block_interactive_references() {
    let root = get_test_files_root();
    let root_path = root.path().join("code_lens_references");
    let (calls_tx, calls) = crossbeam_channel::unbounded();
    let (release, release_rx) = crossbeam_channel::bounded(1);
    let telemetry = TestTelemetry::new();
    let events = telemetry.subscribe();
    let external_uri = Uri::from_file_path(root_path.join("external.py")).unwrap();
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        args: LspArgs {
            indexing_mode: IndexingMode::LazyBlocking,
            ..LspInteractionArgs::default().args
        },
        telemetry: Box::new(telemetry),
        external_references: Arc::new(BlockingReferences {
            calls: calls_tx,
            release: release_rx,
            first: AtomicBool::new(true),
            uri: external_uri.clone(),
        }),
        ..Default::default()
    });
    interaction.set_root(root_path.clone());
    interaction.initialize(InitializeSettings {
        workspace_folders: Some(vec![("test".to_owned(), Uri::from_file_path(&root_path).unwrap())]),
        initialization_options: Some(json!({"pyrefly": {"supportsReferencesCodeLens": true, "referencesCodeLens": true}})),
        ..Default::default()
    }).unwrap();
    interaction.client.did_open("symbols.py");
    interaction.client.did_open("usage.py");
    let lenses = request_code_lenses(
        &interaction,
        Uri::from_file_path(root_path.join("symbols.py")).unwrap(),
    );
    let timeout = Duration::from_secs(10);
    let first = interaction
        .client
        .send_request::<CodeLensResolveRequest>(json!(lenses[0]));
    assert_eq!(calls.recv_timeout(timeout).unwrap(), "symbols.Greeter");

    // Telemetry marks the end of dispatch, so the search is queued before cancellation.
    let wait_for_dispatch = |id: String| {
        let deadline = Instant::now() + timeout;
        loop {
            let event = events.recv_deadline(deadline).unwrap();
            if event.event.request_id.as_ref() == Some(&id)
                && matches!(&event.event.kind, TelemetryEventKind::LspEvent(name) if name.starts_with("LspRequest("))
            {
                break;
            }
        }
    };
    let canceled = interaction
        .client
        .send_request::<CodeLensResolveRequest>(json!(lenses[1]));
    wait_for_dispatch(canceled.id().to_string());
    let live = interaction
        .client
        .send_request::<CodeLensResolveRequest>(json!(lenses[2]));
    wait_for_dispatch(live.id().to_string());
    let interactive = interaction.client.references("symbols.py", 4, 5, false);
    wait_for_dispatch(interactive.id().to_string());
    interaction.client.cancel_request(canceled.id());
    interaction.client.cancel_request(first.id());
    let deadline = Instant::now() + timeout;
    let mut cancellations = 0;
    while cancellations < 2 {
        let event = events.recv_deadline(deadline).unwrap();
        if matches!(&event.event.kind, TelemetryEventKind::LspEvent(name) if name == "CancelRequest")
        {
            cancellations += 1;
        }
    }
    // Interactive references must finish while the first lens is still blocked.
    assert_eq!(calls.recv_timeout(timeout).unwrap(), "symbols.run");
    interactive
        .expect_response_with(|locations| {
            locations
                .unwrap()
                .iter()
                .any(|location| location.uri == external_uri)
        })
        .unwrap();
    interaction
        .client
        .send_request::<RenameRequest>(json!({
            "textDocument": {
                "uri": Uri::from_file_path(root_path.join("symbols.py")).unwrap()
            },
            "position": {"line": 4, "character": 5},
            "newName": "renamed_run"
        }))
        .expect_response_with(|edit| {
            assert!(edit.is_some());
            true
        })
        .unwrap();
    assert_eq!(calls.recv_timeout(timeout).unwrap(), "symbols.run");
    release.send(()).unwrap();
    first
        .expect_response_error_code(ErrorCode::RequestCanceled as i32)
        .unwrap();
    canceled
        .expect_response_error_code(ErrorCode::RequestCanceled as i32)
        .unwrap();
    assert_eq!(calls.recv_timeout(timeout).unwrap(), "symbols.run");
    live.expect_response_with(|lens| {
        has_reference_lens(&lens, 4, "3 references", 3)
            && reference_locations(&lens)
                .iter()
                .any(|location| location.uri == external_uri)
    })
    .unwrap();
    assert!(calls.is_empty());
    interaction.shutdown().unwrap();
}

fn has_reference_lens(
    lens: &CodeLens,
    line: u32,
    expected_title: &str,
    expected_locations: usize,
) -> bool {
    let Some(command) = &lens.command else {
        return false;
    };
    lens.range.start.line == line
        && command.title == expected_title
        && command.command == "pyrefly.showReferences"
        && reference_locations(lens).len() == expected_locations
}

fn reference_locations(lens: &CodeLens) -> Vec<Location> {
    let arguments = lens
        .command
        .as_ref()
        .and_then(|command| command.arguments.as_ref())
        .expect("Resolved reference lens should have command arguments");
    serde_json::from_value(
        arguments
            .get(2)
            .expect("Reference locations should be the third argument")
            .clone(),
    )
    .expect("Reference locations should deserialize")
}
