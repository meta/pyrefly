/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::io;
use std::io::Write;
use std::path::Path;

use pyrefly_config::error_kind::Severity;
use pyrefly_util::absolutize::Absolutize;
use pyrefly_util::thread_pool::in_thread_pool;
use rayon::prelude::*;
use serde::Deserialize;
use serde::Serialize;

use crate::config::config::BaselineFormat;
use crate::config::config::BaselineMatchingMode;
use crate::error::error::Error;
use crate::error::error::ReportedError;

pub(crate) fn severity_to_str(severity: Severity) -> String {
    match severity {
        Severity::Ignore => "ignore".to_owned(),
        Severity::Hint => "hint".to_owned(),
        Severity::Info => "info".to_owned(),
        Severity::Warn => "warn".to_owned(),
        Severity::Error => "error".to_owned(),
    }
}

/// The path of `error` relative to `relative_to`, with `/` separators so that files written
/// from it are the same on every platform.
fn relative_path(relative_to: &Path, error: &Error) -> String {
    error
        .path()
        .as_path()
        .relativize_from(relative_to)
        .to_string_lossy()
        .replace('\\', "/")
}

fn default_legacy_severity() -> Severity {
    Severity::Error
}

/// Legacy error structure in Pyre1. Needs to be consistent with the following file:
/// <https://www.internalfb.com/code/fbsource/fbcode/tools/pyre/facebook/arc/lib/error.rs>
///
/// Used to serialize errors in a Pyre1-compatible format.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct LegacyError {
    line: usize,
    pub column: usize,
    stop_line: usize,
    stop_column: usize,
    pub path: String,
    /// This field is no longer used in Pyrefly. It is kept here for Pyre1 backward compatibility.
    code: i32,
    /// The kebab-case name of the error kind.
    pub name: String,
    description: String,
    concise_description: String,
    /// This field is not part of Pyre1 error format. But it's useful for Pyrefly clients
    #[serde(default = "default_legacy_severity")]
    severity: Severity,
    /// Whether the error matched a configured baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baselined: Option<bool>,
    /// Whether an ignore comment suppresses the error. Suppressed errors are only
    /// written by `pyrefly buck-check --include-suppressed`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    suppressed: bool,
    /// Optional notebook cell number for errors in notebook files
    #[serde(skip_serializing_if = "Option::is_none")]
    cell: Option<usize>,
}

impl LegacyError {
    pub fn from_error(relative_to: &Path, reported: &impl ReportedError) -> Self {
        Self::with_path(relative_path(relative_to, reported.error()), reported)
    }

    /// `path` is the error's path as `relative_path` renders it.
    fn with_path(path: String, reported: &impl ReportedError) -> Self {
        let error = reported.error();
        let error_range = error.display_range();
        Self {
            line: error_range.start.line_within_cell().get() as usize,
            column: error_range.start.column().get() as usize,
            stop_line: error_range.end.line_within_cell().get() as usize,
            stop_column: error_range.end.column().get() as usize,
            cell: error_range.start.cell().map(|cell| cell.get() as usize),
            path,
            // -2 is chosen because it's an unused error code in Pyre1
            code: -2, // TODO: replace this dummy value
            name: error.error_kind().to_name().to_owned(),
            description: error.msg(),
            concise_description: error.msg_header().to_owned(),
            severity: reported.severity(),
            baselined: reported.baseline_status().legacy_baselined_flag(),
            suppressed: false,
        }
    }

    /// Mark the error as suppressed by an ignore comment.
    pub fn into_suppressed(self) -> Self {
        Self {
            suppressed: true,
            ..self
        }
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub(crate) fn display_range(&self) -> String {
        if self.line == self.stop_line {
            if self.column == self.stop_column {
                format!("{}:{}", self.line, self.column)
            } else {
                format!("{}:{}-{}", self.line, self.column, self.stop_column)
            }
        } else {
            format!(
                "{}:{}-{}:{}",
                self.line, self.column, self.stop_line, self.stop_column
            )
        }
    }

    pub(crate) fn concise_description(&self) -> &str {
        &self.concise_description
    }

    pub(crate) fn is_baselined(&self) -> bool {
        self.baselined == Some(true)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct LegacyErrors {
    pub errors: Vec<LegacyError>,
}

/// Below this many errors, `LegacyErrors::from_errors` converts on the calling thread, where
/// the work is too small to pay for moving it to a thread pool.
const PARALLEL_CONVERSION_MIN_ERRORS: usize = 4096;

impl LegacyErrors {
    /// Convert `errors`, in parallel when there are many. Errors from one module are usually
    /// adjacent, so each run of errors with the same path relativizes that path once.
    pub fn from_errors<E: ReportedError>(relative_to: &Path, errors: &[E]) -> Self {
        fn convert<'a, E: ReportedError>(
            relative_to: &Path,
            run: &'a [E],
        ) -> impl Iterator<Item = LegacyError> + 'a {
            let path = relative_path(relative_to, run[0].error());
            run.iter()
                .map(move |reported| LegacyError::with_path(path.clone(), reported))
        }

        let same_path = |a: &E, b: &E| a.error().path() == b.error().path();
        let errors = if errors.len() < PARALLEL_CONVERSION_MIN_ERRORS {
            errors
                .chunk_by(same_path)
                .flat_map(|run| convert(relative_to, run))
                .collect()
        } else {
            errors
                .par_chunk_by(same_path)
                .flat_map_iter(|run| convert(relative_to, run))
                .collect()
        };
        Self { errors }
    }

    /// Write the JSON that `serde_json::to_writer_pretty` writes for the `LegacyErrors` built
    /// from `errors`, without building it. On a thread pool, many errors are converted and
    /// serialized in parallel, each chunk into its own buffer, and the buffers are written in
    /// order. The work is done in waves, so only a wave's buffers are held at once.
    pub fn write_json<E: ReportedError>(
        writer: &mut impl Write,
        relative_to: &Path,
        errors: &[E],
    ) -> io::Result<()> {
        const CHUNK: usize = 1024;
        if errors.is_empty() {
            return writer.write_all(b"{\n  \"errors\": []\n}");
        }
        let serialize = |chunk: &[E]| {
            let mut buffer = Vec::new();
            for run in chunk.chunk_by(|a, b| a.error().path() == b.error().path()) {
                let path = relative_path(relative_to, run[0].error());
                for reported in run {
                    if !buffer.is_empty() {
                        buffer.extend_from_slice(b",\n");
                    }
                    // Each error is an element of the array that is the value of `errors`,
                    // two levels into the document, so every line is indented four spaces.
                    buffer.extend_from_slice(b"    ");
                    serde_json::to_writer_pretty(
                        Indented(&mut buffer),
                        &LegacyError::with_path(path.clone(), reported),
                    )
                    .expect("serializing to a `Vec` cannot fail");
                }
            }
            buffer
        };
        writer.write_all(b"{\n  \"errors\": [\n")?;
        let mut first = true;
        // Off a thread pool, each wave is a single chunk, serialized on the calling thread.
        let wave_len = if in_thread_pool() {
            CHUNK * rayon::current_num_threads() * 4
        } else {
            CHUNK
        };
        for wave in errors.chunks(wave_len) {
            let buffers: Vec<Vec<u8>> = if wave.len() <= CHUNK {
                vec![serialize(wave)]
            } else {
                wave.par_chunks(CHUNK).map(serialize).collect()
            };
            for buffer in buffers {
                if !first {
                    writer.write_all(b",\n")?;
                }
                first = false;
                writer.write_all(&buffer)?;
            }
        }
        writer.write_all(b"\n  ]\n}")
    }
}

/// Writes to the inner writer, indenting each line after the first by four spaces. JSON
/// escapes newlines inside strings, so every newline in pretty-printed JSON is between
/// tokens, and this nests the JSON two levels deeper.
struct Indented<W>(W);

impl<W: Write> Write for Indented<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut lines = buf.split(|byte| *byte == b'\n');
        if let Some(first) = lines.next() {
            self.0.write_all(first)?;
        }
        for line in lines {
            self.0.write_all(b"\n    ")?;
            self.0.write_all(line)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct BaselineError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    pub path: String,
    /// The kebab-case name of the error kind.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concise_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    severity: Option<Severity>,
    /// Optional notebook cell number for errors in notebook files
    #[serde(skip_serializing_if = "Option::is_none")]
    cell: Option<usize>,
}

impl BaselineError {
    fn from_error(relative_to: &Path, reported: &impl ReportedError) -> Self {
        let error = reported.error();
        let error_range = error.display_range();
        Self {
            column: Some(error_range.start.column().get() as usize),
            cell: error_range.start.cell().map(|cell| cell.get() as usize),
            path: relative_path(relative_to, error),
            name: error.error_kind().to_name().to_owned(),
            concise_description: Some(error.msg_header().to_owned()),
            severity: Some(reported.severity()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct BaselineErrors {
    /// The severity threshold in effect when the baseline was written. A diagnostic below
    /// it was never recorded, so matching must not let one take part. Recording the
    /// threshold rather than reading the current run's `--min-severity` keeps that
    /// decision stable when the flag differs between runs. Absent in baselines written
    /// before it was tracked, where nothing is held back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_severity: Option<Severity>,
    pub errors: Vec<BaselineError>,
}

impl BaselineErrors {
    pub fn from_errors(
        relative_to: &Path,
        min_severity: Severity,
        errors: &[impl ReportedError],
    ) -> Self {
        Self {
            min_severity: Some(min_severity),
            errors: errors
                .iter()
                .map(|e| BaselineError::from_error(relative_to, e))
                .collect(),
        }
    }

    /// Remove fields that are not needed by a minimal baseline.
    pub fn with_format(
        mut self,
        matching_mode: BaselineMatchingMode,
        format: BaselineFormat,
    ) -> Self {
        // Only `column-ordered` consults the threshold, so it is not part of the minimum
        // the other modes need in order to match.
        if !matching_mode.is_ordered() {
            self.min_severity = None;
        }
        if format == BaselineFormat::Minimal {
            for error in &mut self.errors {
                if !matches!(
                    matching_mode,
                    BaselineMatchingMode::Column | BaselineMatchingMode::ColumnOrdered
                ) {
                    error.column = None;
                }
                if matching_mode != BaselineMatchingMode::ConciseDescription {
                    error.concise_description = None;
                }
                error.severity = None;
                error.cell = None;
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use dupe::Dupe;
    use pyrefly_python::module::Module;
    use pyrefly_python::module_name::ModuleName;
    use pyrefly_python::module_path::ModulePath;
    use pyrefly_util::thread_pool::TEST_THREAD_COUNT;
    use pyrefly_util::thread_pool::ThreadPool;
    use ruff_text_size::TextRange;
    use ruff_text_size::TextSize;

    use super::*;
    use crate::config::error_kind::ErrorKind;
    use crate::error::error::BaselineStatus;

    #[test]
    fn test_write_json_matches_serializing_legacy_errors() {
        let contents = Arc::new("x = 1\n".repeat(2000));
        let errors: Vec<Error> = ["foo", "bar"]
            .into_iter()
            .flat_map(|name| {
                let module = Module::new(
                    ModuleName::from_str(name),
                    ModulePath::filesystem(PathBuf::from(format!("/repo/{name}.py"))),
                    contents.dupe(),
                );
                (0..1500u32).map(move |i| {
                    let start = TextSize::new(i * 6);
                    Error::new(
                        module.dupe(),
                        TextRange::new(start, start + TextSize::new(1)),
                        format!("error {i} with \"quotes\""),
                        vec!["detail\n  over two lines".to_owned()],
                        ErrorKind::BadAssignment,
                    )
                    .with_baseline_status(BaselineStatus::Unmatched)
                })
            })
            .collect();
        // Enough errors to be written in parallel, a few, and none.
        for errors in [&errors[..], &errors[..3], &[]] {
            let mut expected = Vec::new();
            serde_json::to_writer_pretty(
                &mut expected,
                &LegacyErrors::from_errors(Path::new("/repo"), errors),
            )
            .unwrap();
            // Errors are serialized in parallel only on a thread pool.
            let mut written = Vec::new();
            ThreadPool::new(TEST_THREAD_COUNT).install(|| {
                LegacyErrors::write_json(&mut written, Path::new("/repo"), errors).unwrap()
            });
            assert_eq!(
                String::from_utf8(written).unwrap(),
                String::from_utf8(expected).unwrap()
            );
        }
    }

    #[test]
    fn test_relativize_when_error_is_not_under_relative_to() {
        let module = Module::new(
            ModuleName::from_str("foo"),
            ModulePath::filesystem(PathBuf::from("/repo/libs/foo.py")),
            Arc::new("x = 1\n".to_owned()),
        );
        let error = Error::new(
            module,
            TextRange::new(TextSize::new(0), TextSize::new(1)),
            "err".to_owned(),
            Vec::new(),
            ErrorKind::BadAssignment,
        );
        let legacy = LegacyError::from_error(Path::new("/repo/src"), &error);
        assert_eq!(legacy.path, "../libs/foo.py");
    }

    #[test]
    fn test_baseline_provenance_is_optional() {
        let module = Module::new(
            ModuleName::from_str("foo"),
            ModulePath::filesystem(PathBuf::from("/repo/foo.py")),
            Arc::new("x = 1\n".to_owned()),
        );
        let error = Error::new(
            module,
            TextRange::new(TextSize::new(0), TextSize::new(1)),
            "err".to_owned(),
            Vec::new(),
            ErrorKind::BadAssignment,
        );

        let without_baseline =
            serde_json::to_value(LegacyError::from_error(Path::new("/repo"), &error)).unwrap();
        assert!(without_baseline.get("baselined").is_none());

        let matched = error.clone().with_baseline_status(BaselineStatus::Matched);
        assert_eq!(
            serde_json::to_value(LegacyError::from_error(Path::new("/repo"), &matched)).unwrap()["baselined"],
            true
        );

        let not_compared = error.with_baseline_status(BaselineStatus::NotCompared);
        assert_eq!(
            serde_json::to_value(LegacyError::from_error(Path::new("/repo"), &not_compared))
                .unwrap()["baselined"],
            false
        );
    }

    #[test]
    fn test_baseline_formats() {
        let module = Module::new(
            ModuleName::from_str("foo"),
            ModulePath::filesystem(PathBuf::from("/repo/foo.py")),
            Arc::new("x = 1\n".to_owned()),
        );
        let error = Error::new(
            module,
            TextRange::new(TextSize::new(0), TextSize::new(1)),
            "err".to_owned(),
            Vec::new(),
            ErrorKind::BadAssignment,
        );

        let full = BaselineErrors::from_errors(
            Path::new("/repo"),
            Severity::Error,
            std::slice::from_ref(&error),
        )
        .with_format(BaselineMatchingMode::Column, BaselineFormat::Full);
        assert_eq!(
            serde_json::to_value(full).unwrap(),
            serde_json::json!({
                "errors": [{
                    "column": 1,
                    "path": "foo.py",
                    "name": "bad-assignment",
                    "concise_description": "err",
                    "severity": "error"
                }]
            })
        );

        let minimal_column = BaselineErrors::from_errors(
            Path::new("/repo"),
            Severity::Error,
            std::slice::from_ref(&error),
        )
        .with_format(BaselineMatchingMode::Column, BaselineFormat::Minimal);
        assert_eq!(
            serde_json::to_value(minimal_column).unwrap(),
            serde_json::json!({
                "errors": [{
                    "column": 1,
                    "path": "foo.py",
                    "name": "bad-assignment"
                }]
            })
        );

        let minimal_description = BaselineErrors::from_errors(
            Path::new("/repo"),
            Severity::Error,
            std::slice::from_ref(&error),
        )
        .with_format(
            BaselineMatchingMode::ConciseDescription,
            BaselineFormat::Minimal,
        );
        assert_eq!(
            serde_json::to_value(minimal_description).unwrap(),
            serde_json::json!({
                "errors": [{
                    "path": "foo.py",
                    "name": "bad-assignment",
                    "concise_description": "err"
                }]
            })
        );

        // `column-ordered` matches on the column, so a minimal baseline must keep it, and
        // holds back diagnostics below the threshold, so it must record that too.
        let minimal_ordered = BaselineErrors::from_errors(
            Path::new("/repo"),
            Severity::Warn,
            std::slice::from_ref(&error),
        )
        .with_format(BaselineMatchingMode::ColumnOrdered, BaselineFormat::Minimal);
        assert_eq!(
            serde_json::to_value(minimal_ordered).unwrap(),
            serde_json::json!({
                "min_severity": "warn",
                "errors": [{
                    "column": 1,
                    "path": "foo.py",
                    "name": "bad-assignment"
                }]
            })
        );

        let without_severity = serde_json::json!({
            "errors": [{
                "column": 1,
                "path": "foo.py",
                "name": "bad-assignment"
            }]
        });
        let parsed: BaselineErrors = serde_json::from_value(without_severity.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), without_severity);
    }
}
