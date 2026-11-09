/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use lsp_server::ErrorCode;
use lsp_types::Uri;
use pyrefly_config::config::ConfigFile;
use pyrefly_util::events::CategorizedEvents;
use pyrefly_util::thread_pool::ThreadCount;
use serde_json::json;
use tempfile::TempDir;
use tsp_types::Declaration;
use tsp_types::GetTypeArg;
use tsp_types::GetTypeArgNode;
use tsp_types::GetTypeParams;
use tsp_types::Position;
use tsp_types::Range;
use tsp_types::Type;
use tsp_types::TypeKind;

use crate::test::util::init_test;
use crate::tsp::session::FilesystemSession;

fn session(root: &Path, main: &str) -> FilesystemSession {
    init_test();
    fs::write(root.join("main.py"), main).unwrap();
    let mut config = ConfigFile {
        search_path_from_args: vec![root.to_owned()],
        disable_search_path_heuristics: true,
        ..Default::default()
    };
    config.python_environment.set_empty_to_default();
    config.interpreters.skip_interpreter_query = true;
    let mut session = FilesystemSession::new(config, ThreadCount::Inline).unwrap();
    session.set_files(vec![root.join("main.py")]).unwrap();
    session
}

fn params(path: &Path, line: u32, start: u32, end: u32, snapshot: i32) -> GetTypeParams {
    GetTypeParams {
        arg: GetTypeArg::Node(GetTypeArgNode {
            uri: Uri::from_file_path(path).unwrap().to_string(),
            range: Range {
                start: Position {
                    line,
                    character: start,
                },
                end: Position {
                    line,
                    character: end,
                },
            },
        }),
        snapshot,
    }
}

fn assert_class(ty: Option<Type>, name: &str) {
    let Some(Type::Class(class)) = ty else {
        panic!("Expected {name} class, got {ty:?}");
    };
    assert_eq!(class.kind, TypeKind::Class);
    match class.declaration {
        Declaration::Regular(declaration) => assert_eq!(declaration.name.as_deref(), Some(name)),
        other => panic!("Expected a regular {name} declaration, got {other:?}"),
    }
}

#[test]
fn test_filesystem_session_queries_edits_and_stale_snapshots() {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    let foo = root.join("foo.py");
    let main = root.join("main.py");
    fs::write(&foo, "value: int = 1\n").unwrap();
    let mut session = session(root, "from foo import value\nvalue\n");
    assert_eq!(session.get_snapshot(), 0);
    let old = params(&main, 1, 0, 5, session.get_snapshot());
    for _ in 0..3 {
        assert_class(session.get_computed_type(&old).unwrap(), "int");
        assert_eq!(session.get_snapshot(), old.snapshot);
    }
    assert_class(
        session
            .get_computed_type(&params(&foo, 0, 0, 5, old.snapshot))
            .unwrap(),
        "int",
    );

    fs::write(&foo, "value: str = 'editor'\n").unwrap();
    let notification = session
        .files_changed(&CategorizedEvents {
            modified: vec![foo.clone()],
            ..Default::default()
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(notification).unwrap(),
        json!({
            "jsonrpc": "2.0",
            "method": "typeServer/snapshotChanged",
            "params": {"old": 0, "new": 1}
        })
    );
    let stale = session.get_computed_type(&old).unwrap_err();
    assert_eq!(stale.code, ErrorCode::ServerCancelled as i32);
    assert!(stale.message.contains("client sent 0, server is at 1"));
    let current = params(&main, 1, 0, 5, session.get_snapshot());
    assert_class(session.get_computed_type(&current).unwrap(), "str");
    assert_class(
        session
            .get_computed_type(&params(&foo, 0, 0, 5, current.snapshot))
            .unwrap(),
        "str",
    );
    assert_eq!(session.get_snapshot(), current.snapshot);
    assert!(
        session
            .files_changed(&CategorizedEvents::default())
            .unwrap()
            .is_none()
    );
    assert_eq!(session.get_snapshot(), current.snapshot);

    fs::write(&foo, "value: int = 1\n").unwrap();
    session
        .files_changed(&CategorizedEvents {
            modified: vec![foo],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(session.get_snapshot(), 2);
    assert_class(
        session
            .get_computed_type(&params(&main, 1, 0, 5, session.get_snapshot()))
            .unwrap(),
        "int",
    );
}

#[test]
fn test_filesystem_session_discovers_created_and_removed_imports() {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    let main = root.join("main.py");
    let created = root.join("created.py");
    let mut session = session(root, "from created import value\nvalue\n");
    let before = params(&main, 1, 0, 5, session.get_snapshot());
    assert!(!matches!(
        session.get_computed_type(&before).unwrap(),
        Some(Type::Class(_))
    ));

    fs::write(&created, "value: int = 3\n").unwrap();
    session
        .files_changed(&CategorizedEvents {
            created: vec![created.clone()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        session.get_computed_type(&before).unwrap_err().code,
        ErrorCode::ServerCancelled as i32
    );
    assert_class(
        session
            .get_computed_type(&params(&main, 1, 0, 5, session.get_snapshot()))
            .unwrap(),
        "int",
    );
    let before_remove = session.get_snapshot();
    fs::remove_file(&created).unwrap();
    session
        .files_changed(&CategorizedEvents {
            removed: vec![created],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(session.get_snapshot(), before_remove + 1);
    assert!(!matches!(
        session
            .get_computed_type(&params(&main, 1, 0, 5, session.get_snapshot()))
            .unwrap(),
        Some(Type::Class(_))
    ));
}

#[test]
fn test_filesystem_session_preserves_function_declarations_and_call_results() {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    let foo = root.join("foo.py");
    fs::write(&foo, "def build() -> str:\n    return 'value'\n").unwrap();
    let mut session = session(root, "from foo import build\nbuild\nbuild()\n");
    let snapshot = session.get_snapshot();
    let ty = session
        .get_computed_type(&params(&root.join("main.py"), 1, 0, 5, snapshot))
        .unwrap();
    let Some(Type::Function(function)) = ty else {
        panic!("Expected a declaration-preserving function, got {ty:?}");
    };
    let Declaration::Regular(declaration) = function.declaration else {
        panic!("Expected function source declaration");
    };
    assert_eq!(declaration.name.as_deref(), Some("build"));
    assert_eq!(
        declaration.node.uri,
        Uri::from_file_path(&foo).unwrap().to_string()
    );
    assert_eq!(
        declaration.node.range.start,
        Position {
            line: 0,
            character: 4
        }
    );
    assert_eq!(
        declaration.node.range.end,
        Position {
            line: 0,
            character: 9
        }
    );
    assert_class(
        session
            .get_computed_type(&params(&root.join("main.py"), 2, 0, 7, snapshot))
            .unwrap(),
        "str",
    );
    assert_eq!(session.get_snapshot(), snapshot);
}

#[test]
fn test_filesystem_session_rejects_invalid_inputs_without_moving_snapshot() {
    let directory = TempDir::new().unwrap();
    let mut session = session(directory.path(), "value: int = 1\nvalue\n");
    let snapshot = session.get_snapshot();
    let main = directory.path().join("main.py");
    for (line, start, end) in [(1, 5, 0), (99, 0, 0), (1, 99, 99)] {
        assert_eq!(
            session
                .get_computed_type(&params(&main, line, start, end, snapshot))
                .unwrap_err()
                .code,
            ErrorCode::InvalidParams as i32
        );
    }
    for uri in ["not a URI", "untitled:Untitled-1"] {
        let mut request = params(&main, 0, 0, 5, snapshot);
        request.arg = GetTypeArg::Node(GetTypeArgNode {
            uri: uri.to_owned(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            },
        });
        assert_eq!(
            session.get_computed_type(&request).unwrap_err().code,
            ErrorCode::InvalidParams as i32
        );
    }
    assert_eq!(
        session
            .set_files(vec![PathBuf::from("relative.py")])
            .unwrap_err()
            .code,
        ErrorCode::InvalidParams as i32
    );
    assert_eq!(
        session
            .files_changed(&CategorizedEvents {
                modified: vec![main.clone(), PathBuf::from("relative.py")],
                ..Default::default()
            })
            .unwrap_err()
            .code,
        ErrorCode::InvalidParams as i32
    );
    assert_eq!(session.get_snapshot(), snapshot);
    assert_class(
        session
            .get_computed_type(&params(&main, 1, 0, 5, snapshot))
            .unwrap(),
        "int",
    );
}

#[test]
fn test_filesystem_session_uses_utf16_ranges_and_rejects_split_surrogates() {
    let directory = TempDir::new().unwrap();
    let main = directory.path().join("main.py");
    let mut session = session(
        directory.path(),
        "value: int = 1\ntext = '\u{1f600}'; value\n",
    );
    let snapshot = session.get_snapshot();
    assert_class(
        session
            .get_computed_type(&params(&main, 1, 13, 18, snapshot))
            .unwrap(),
        "int",
    );
    assert_eq!(
        session
            .get_computed_type(&params(&main, 1, 9, 9, snapshot))
            .unwrap_err()
            .code,
        ErrorCode::InvalidParams as i32
    );
    assert_eq!(session.get_snapshot(), snapshot);
}

#[test]
fn test_filesystem_session_surfaces_configuration_errors() {
    let mut config = ConfigFile::parse_config("python-interpreter-path = 'not-executed'").unwrap();
    config.interpreters.skip_interpreter_query = true;
    let error = FilesystemSession::new(config, ThreadCount::Inline)
        .err()
        .expect("Conflicting interpreter selections must not initialize a session");
    assert_eq!(error.code, ErrorCode::InvalidParams as i32);
    assert!(
        error
            .message
            .contains("Only one interpreter selection option")
    );
}
