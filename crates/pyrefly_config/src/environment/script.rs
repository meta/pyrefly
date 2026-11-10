/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! PEP 723 scripts: their metadata, and the Python environments that a script command reports
//! for them in the format of `uv workspace metadata --script`.

use std::path::Path;
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;
use serde::Deserialize;

/// The subset of `uv workspace metadata --script` output that names a script's environment.
#[derive(Deserialize)]
struct CommandOutput {
    environment: Option<Environment>,
}

#[derive(Deserialize)]
struct Environment {
    python: Python,
}

#[derive(Deserialize)]
struct Python {
    path: PathBuf,
}

/// Returns the contents of the PEP 723 `script` metadata block in `source`, if it has one.
pub fn metadata(source: &str) -> Option<&str> {
    let mut start = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let text = line.trim_end_matches(['\n', '\r']);
        match start {
            None if text == "# /// script" => start = Some(offset + line.len()),
            Some(start) if text == "# ///" => return Some(&source[start..offset]),
            Some(_) if text != "#" && !text.starts_with("# ") => start = None,
            _ => {}
        }
        offset += line.len();
    }
    None
}

/// Returns the interpreter of the environment that `command` reports for `script`, or `None` if
/// that environment does not exist. Pyrefly runs `command` with the script's path appended, in the
/// script's directory, and reads its output in the JSON format of `uv workspace metadata --script`.
#[cfg(not(target_arch = "wasm32"))]
pub fn interpreter(command: &[String], script: &Path) -> anyhow::Result<Option<PathBuf>> {
    let Some((program, args)) = command.split_first() else {
        return Err(anyhow::anyhow!("`script-command` is empty"));
    };
    let invocation = format!("{} {}", command.join(" "), script.display());
    let mut process = Command::new(program);
    process.args(args).arg(script);
    if let Some(directory) = script.parent() {
        process.current_dir(directory);
    }
    let output = process
        .output()
        .with_context(|| format!("Failed to run `{invocation}`"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "`{invocation}` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let parsed: CommandOutput = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("Failed to parse the output of `{invocation}`"))?;
    Ok(parsed
        .environment
        .map(|environment| environment.python.path))
}

#[cfg(target_arch = "wasm32")]
pub fn interpreter(_command: &[String], _script: &Path) -> anyhow::Result<Option<PathBuf>> {
    Err(anyhow::anyhow!(
        "PEP 723 script environments are not supported on WebAssembly"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        assert_eq!(
            metadata(
                "#!/usr/bin/env python\n# /// script\n# dependencies = [\"six\"]\n#\n# ///\nimport six\n"
            ),
            Some("# dependencies = [\"six\"]\n#\n"),
        );
        assert_eq!(
            metadata("# /// script\r\n# requires-python = \">=3.12\"\r\n# ///\r\n"),
            Some("# requires-python = \">=3.12\"\r\n"),
        );
        assert_eq!(metadata("# /// script\nimport six\n# ///\n"), None);
        assert_eq!(metadata("# /// pyproject\n# ///\n"), None);
        assert_eq!(metadata("x = 1\n"), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_interpreter() {
        let command = |output: &str| {
            vec![
                "sh".to_owned(),
                "-c".to_owned(),
                format!("test \"$1\" = /tool.py && echo '{output}'"),
                "sh".to_owned(),
            ]
        };
        let script = Path::new("/tool.py");
        assert_eq!(
            interpreter(
                &command(r#"{"environment": {"python": {"path": "/env/bin/python"}}}"#),
                script
            )
            .unwrap(),
            Some(PathBuf::from("/env/bin/python")),
        );
        assert_eq!(
            interpreter(&command(r#"{"environment": null}"#), script).unwrap(),
            None,
        );
        assert!(interpreter(&["false".to_owned()], script).is_err());
        assert!(interpreter(&[], script).is_err());
    }
}
