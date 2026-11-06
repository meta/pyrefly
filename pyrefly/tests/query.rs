/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;

use lsp_types::Uri;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

/// A project inside an unrelated configured directory.
struct Project {
    root: TempDir,
}

impl Project {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("pyrefly query ")
            .tempdir()
            .unwrap();
        fs::write(root.path().join("pyrefly.toml"), "project-includes = []\n").unwrap();
        let project = root.path().join("project");
        fs::create_dir(&project).unwrap();
        fs::write(
            project.join("pyrefly.toml"),
            "project-includes = [\"**/*.py\"]\nproject-excludes = []\nskip-interpreter-query = true\n",
        )
        .unwrap();
        fs::write(
            project.join("core.py"),
            "def leaf(x: int) -> int:\n    return x\n\ndef target(x: int) -> int:\n    return leaf(x)\n",
        )
        .unwrap();
        fs::write(
            project.join("caller.py"),
            "from core import target\n\ndef caller() -> int:\n    return target(1)\n\nbad: str = 1\n",
        )
        .unwrap();
        fs::write(
            project.join("unrelated.py"),
            "def target() -> str:\n    return 'unrelated'\n\nresult = target()\n",
        )
        .unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn query(
        &self,
        cwd: &Path,
        kind: &str,
        file: &str,
        line: u32,
        column: u32,
        extra: &[&str],
    ) -> Output {
        Command::new(env!("CARGO_BIN_EXE_pyrefly"))
            .current_dir(cwd)
            .args(["--threads", "1", "query", kind])
            .arg(self.path().join("project").join(file))
            .arg(line.to_string())
            .arg(column.to_string())
            .args(extra)
            .output()
            .unwrap()
    }
}

/// Parse the complete stdout so that any non-JSON output fails the test.
fn result(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    value
}

fn location(path: &Path, line: u32, start: u32, end: u32) -> Value {
    json!({
        "uri": Uri::from_file_path(path).unwrap(),
        "range": {
            "start": {"line": line, "character": start},
            "end": {"line": line, "character": end}
        }
    })
}

#[test]
fn references_select_the_target_project_and_have_exact_sorted_locations() {
    let project = Project::new();
    let source = project.path().join("project");
    let expected = json!([
        location(&source.join("caller.py"), 0, 17, 23),
        location(&source.join("caller.py"), 3, 11, 17),
        location(&source.join("core.py"), 3, 4, 10),
    ]);
    for cwd in [project.path(), source.as_path()] {
        let references = result(project.query(
            cwd,
            "find-references",
            "core.py",
            4,
            5,
            &["--include-declaration"],
        ));
        assert_eq!(references, expected);
    }
    assert_eq!(
        result(project.query(project.path(), "find-references", "core.py", 4, 5, &[])),
        json!([
            location(&source.join("caller.py"), 0, 17, 23),
            location(&source.join("caller.py"), 3, 11, 17),
        ])
    );
}

#[test]
fn nested_configuration_and_explicit_configuration_select_candidates() {
    let project = Project::new();
    let nested = project.path().join("project/nested");
    fs::create_dir(&nested).unwrap();
    fs::write(
        nested.join("pyrefly.toml"),
        "project-includes = [\"*.py\"]\nskip-interpreter-query = true\n",
    )
    .unwrap();
    fs::write(nested.join("inner.py"), "value: int = 1\nprint(value)\n").unwrap();
    assert_eq!(
        result(project.query(
            project.path(),
            "find-references",
            "nested/inner.py",
            1,
            1,
            &["--include-declaration"]
        )),
        json!([
            location(&nested.join("inner.py"), 0, 0, 5),
            location(&nested.join("inner.py"), 1, 6, 11),
        ])
    );

    let config = project.path().join("project/explicit.toml");
    fs::write(
        &config,
        "project-includes = [\"core.py\"]\nskip-interpreter-query = true\n",
    )
    .unwrap();
    assert_eq!(
        result(project.query(
            project.path(),
            "find-references",
            "core.py",
            4,
            5,
            &[
                "--include-declaration",
                "--config",
                config.to_str().unwrap()
            ]
        )),
        json!([location(&project.path().join("project/core.py"), 3, 4, 10)])
    );
}

#[test]
fn candidates_keep_their_own_configuration() {
    let project = Project::new();
    let nested = project.path().join("project/nested");
    fs::create_dir(&nested).unwrap();
    fs::write(
        nested.join("pyrefly.toml"),
        "replace-imports-with-any = [\"core\"]\nskip-interpreter-query = true\n",
    )
    .unwrap();
    fs::write(
        nested.join("other.py"),
        "from core import target\n\ntarget(2)\n",
    )
    .unwrap();
    let references = result(project.query(
        project.path(),
        "find-references",
        "core.py",
        4,
        5,
        &["--include-declaration"],
    ));
    assert_eq!(references.as_array().unwrap().len(), 3);
    assert!(
        references
            .as_array()
            .unwrap()
            .iter()
            .all(|location| !location["uri"].as_str().unwrap().contains("other.py"))
    );
}

#[test]
fn build_system_configuration_can_supply_imports() {
    let project = Project::new();
    let source = project.path().join("project");
    let manifest = json!({
        "root": source,
        "db": {"//:library": {
            "srcs": {"core": ["core.py"], "caller": ["caller.py"]},
            "python_version": "3.13",
            "python_platform": if cfg!(windows) { "win32" } else if cfg!(target_os = "macos") { "darwin" } else { "linux" }
        }}
    });
    fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
    // The build-system protocol permits extra arguments after the configured command.
    let interpreter = if cfg!(windows) { "python" } else { "python3" };
    let command = json!([
        interpreter,
        "-c",
        "import sys; from pathlib import Path; sys.stdout.buffer.write(Path('manifest.json').read_bytes())"
    ]);
    fs::write(source.join("pyrefly.toml"), format!(
        "project-includes = [\"core.py\", \"caller.py\"]\nskip-interpreter-query = true\n[build-system]\ntype = \"custom\"\ncommand = {command}\n"
    )).unwrap();
    assert_eq!(
        result(project.query(
            project.path(),
            "find-references",
            "core.py",
            4,
            5,
            &["--include-declaration"]
        )),
        json!([
            location(&source.join("caller.py"), 0, 17, 23),
            location(&source.join("caller.py"), 3, 11, 17),
            location(&source.join("core.py"), 3, 4, 10),
        ])
    );
    fs::write(source.join("manifest.json"), "invalid JSON").unwrap();
    let output = project.query(project.path(), "find-references", "core.py", 4, 5, &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to query sourcedb"));
}

#[test]
fn hover_and_call_hierarchy_resolve_exact_source_ranges() {
    let project = Project::new();
    let hover = result(project.query(project.path(), "hover", "core.py", 4, 5, &[]));
    assert!(
        hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("(x: int) -> int")
    );
    let hierarchy =
        result(project.query(project.path(), "call-hierarchy", "caller.py", 4, 12, &[]));
    assert_eq!(hierarchy["item"]["name"], "target");
    assert_eq!(
        hierarchy["item"]["selectionRange"],
        location(&project.path().join("project/core.py"), 3, 4, 10)["range"]
    );
    assert_eq!(hierarchy["incoming"].as_array().unwrap().len(), 1);
    assert_eq!(hierarchy["incoming"][0]["from"]["name"], "caller");
    assert_eq!(
        hierarchy["incoming"][0]["fromRanges"],
        json!([location(&project.path().join("project/caller.py"), 3, 11, 20)["range"]])
    );
    assert_eq!(hierarchy["outgoing"].as_array().unwrap().len(), 1);
    assert_eq!(hierarchy["outgoing"][0]["to"]["name"], "leaf");
    assert_eq!(
        hierarchy["outgoing"][0]["to"]["selectionRange"],
        location(&project.path().join("project/core.py"), 0, 4, 8)["range"]
    );
    assert_eq!(
        hierarchy["outgoing"][0]["fromRanges"],
        json!([location(&project.path().join("project/core.py"), 4, 11, 18)["range"]])
    );
}

#[test]
fn bundled_definitions_use_existing_stub_files() {
    let project = Project::new();
    fs::write(
        project.path().join("project/stdlib.py"),
        "import json\n\ndef encode() -> str:\n    return json.dumps(len([]))\n",
    )
    .unwrap();
    for column in [17, 23] {
        let references = result(project.query(
            project.path(),
            "find-references",
            "stdlib.py",
            4,
            column,
            &["--include-declaration"],
        ));
        let mut found_stub = false;
        for entry in references.as_array().unwrap() {
            let uri: Uri = serde_json::from_value(entry["uri"].clone()).unwrap();
            let path = uri.to_file_path().unwrap();
            assert!(path.is_file(), "{}", path.display());
            found_stub |= path.extension().is_some_and(|extension| extension == "pyi");
        }
        assert!(found_stub);
        let hierarchy = result(project.query(
            project.path(),
            "call-hierarchy",
            "stdlib.py",
            4,
            column,
            &[],
        ));
        let uri: Uri = serde_json::from_value(hierarchy["item"]["uri"].clone()).unwrap();
        assert!(uri.to_file_path().unwrap().is_file());
    }
    let hierarchy = result(project.query(project.path(), "call-hierarchy", "stdlib.py", 3, 5, &[]));
    assert!(
        hierarchy["outgoing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call["to"]["name"] == "dumps")
    );
    for call in hierarchy["outgoing"].as_array().unwrap() {
        let uri: Uri = serde_json::from_value(call["to"]["uri"].clone()).unwrap();
        assert_eq!(uri.to_file_path().unwrap().extension().unwrap(), "pyi");
    }
}

#[test]
fn invalid_targets_and_positions_fail_without_json_output() {
    let project = Project::new();
    fs::write(
        project.path().join("project/unicode.py"),
        "text = '\u{1f40d}'; value = 1\r\n    \r\n",
    )
    .unwrap();
    fs::write(project.path().join("project/broken.py"), [0xff]).unwrap();
    for (file, line, column, message) in [
        ("missing.py", 1, 1, "cannot access target"),
        (".", 1, 1, "is not a file"),
        ("broken.py", 1, 1, "cannot load target"),
        ("core.py", 0, 1, "line must be greater than zero"),
        ("core.py", 1, 0, "column must be greater than zero"),
        ("core.py", 99, 1, "is outside"),
        ("core.py", 1, 99, "not a valid UTF-16 position"),
        ("unicode.py", 1, 10, "not a valid UTF-16 position"),
    ] {
        let output = project.query(project.path(), "hover", file, line, column, &[]);
        assert!(!output.status.success(), "{file}:{line}:{column}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{output:?}"
        );
    }
    assert_eq!(
        result(project.query(project.path(), "hover", "unicode.py", 2, 1, &[])),
        Value::Null
    );
    let hover = result(project.query(project.path(), "hover", "unicode.py", 1, 14, &[]));
    assert!(!hover.is_null());
    fs::write(
        project.path().join("project/pyrefly.toml"),
        "invalid config",
    )
    .unwrap();
    let output = project.query(project.path(), "hover", "core.py", 4, 5, &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Fatal configuration error"));
}

#[test]
fn query_help_describes_the_experimental_coordinate_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_pyrefly"))
        .args(["query", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for text in [
        "Experimental",
        "one-based",
        "UTF-16",
        "zero-based",
        "file URIs",
    ] {
        assert!(help.contains(text), "{help}");
    }
}
