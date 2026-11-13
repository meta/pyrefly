/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Measure listing and resolving visible reference lenses in a 10,000-line file.

use std::cell::RefCell;
use std::fs;
use std::time::Duration;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use lsp_types::CodeLens;
use lsp_types::CodeLensRequest;
use lsp_types::CodeLensResolveRequest;
use lsp_types::Uri;
use pyrefly_lsp_test::IndexingMode;
use pyrefly_lsp_test::LspArgs;
use pyrefly_lsp_test::object_model::InitializeSettings;
use pyrefly_lsp_test::object_model::LspInteraction;
use pyrefly_lsp_test::object_model::LspInteractionArgs;
use pyrefly_util::telemetry::NoTelemetry;
use serde_json::json;
use tempfile::TempDir;

const DEFINITIONS: usize = 2500;
const VISIBLE_LENSES: usize = 20;
const IMPORTERS: usize = 10;

/// Measure warm LSP requests with fixture creation and initial analysis excluded.
fn code_lens(c: &mut Criterion) {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("pyrefly.toml"),
        "project-includes = [\"**/*.py\"]\n",
    )
    .unwrap();
    let mut source = String::new();
    for index in 0..DEFINITIONS {
        source.push_str(&format!(
            "def function_{index}() -> int:\n    return {index}\n\nfunction_{index}()\n"
        ));
    }
    assert_eq!(source.lines().count(), 10_000);
    fs::write(root.path().join("large.py"), source).unwrap();
    let mut usage = "import large\n".to_owned();
    for index in 0..VISIBLE_LENSES {
        usage.push_str(&format!("large.function_{index}()\n"));
    }
    for index in 0..IMPORTERS {
        fs::write(root.path().join(format!("usage_{index}.py")), &usage).unwrap();
    }

    let uri = Uri::from_file_path(root.path().join("large.py")).unwrap();
    let mut interaction = LspInteraction::new_with_args(LspInteractionArgs {
        args: LspArgs {
            indexing_mode: IndexingMode::LazyBlocking,
            ..LspInteractionArgs::default().args
        },
        telemetry: Box::new(NoTelemetry),
        ..Default::default()
    });
    interaction.client.set_message_logging(false);
    interaction.set_root(root.path().to_path_buf());
    interaction
        .initialize(InitializeSettings {
            workspace_folders: Some(vec![(
                "benchmark".to_owned(),
                Uri::from_file_path(root.path()).unwrap(),
            )]),
            initialization_options: Some(json!({"pyrefly": {
                "supportsReferencesCodeLens": true,
                "referencesCodeLens": true
            }})),
            ..Default::default()
        })
        .unwrap();
    interaction.client.did_open("large.py");
    let params = json!({"textDocument": {"uri": uri}});
    let lenses = RefCell::new(Vec::<CodeLens>::new());
    interaction
        .client
        .send_request::<CodeLensRequest>(params.clone())
        .expect_response_with(|response| {
            *lenses.borrow_mut() = response.unwrap();
            assert_eq!(lenses.borrow().len(), DEFINITIONS);
            true
        })
        .unwrap();
    let visible: Vec<_> = lenses
        .into_inner()
        .into_iter()
        .take(VISIBLE_LENSES)
        .map(|lens| json!(lens))
        .collect();
    let resolve_visible = || {
        for lens in &visible {
            interaction
                .client
                .send_request::<CodeLensResolveRequest>(lens.clone())
                .expect_response_with(|resolved| {
                    assert_eq!(resolved.command.unwrap().title, "11 references");
                    true
                })
                .unwrap();
        }
    };
    resolve_visible();

    let mut group = c.benchmark_group("code_lens/10000_lines");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));
    group.bench_function("list", |b| {
        b.iter(|| {
            interaction
                .client
                .send_request::<CodeLensRequest>(params.clone())
                .expect_response_with(|response| {
                    let lenses = response.unwrap();
                    assert_eq!(lenses.len(), DEFINITIONS);
                    assert!(lenses.iter().all(|lens| lens.command.is_none()));
                    true
                })
                .unwrap();
        });
    });
    group.bench_function("resolve_20_visible", |b| b.iter(resolve_visible));
    group.finish();
    interaction.shutdown().unwrap();
}

criterion_group!(benches, code_lens);
criterion_main!(benches);
