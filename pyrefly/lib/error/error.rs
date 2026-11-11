/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::borrow::Cow;
use std::cmp;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::io;
use std::io::Write;
use std::num::NonZeroU32;
use std::path::Path;

use itertools::Itertools;
use lsp_types::CodeDescription;
use lsp_types::Diagnostic;
use lsp_types::DiagnosticSeverity;
use lsp_types::DiagnosticTag;
use lsp_types::Uri;
use pyrefly_python::ignore::SuppressionEdit;
use pyrefly_python::ignore::SuppressionEffect;
use pyrefly_python::ignore::Tool;
use pyrefly_python::ignore::TypeIgnoreUnknownTagBehavior;
use pyrefly_python::module::Module;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::display::number_thousands;
use pyrefly_util::lined_buffer::DisplayRange;
use pyrefly_util::lined_buffer::LineNumber;
use pyrefly_util::lined_buffer::LinedBuffer;
use pyrefly_util::thread_pool::in_thread_pool;
use rayon::prelude::*;
use ruff_annotate_snippets::Annotation;
use ruff_annotate_snippets::AnnotationKind;
use ruff_annotate_snippets::Group;
use ruff_annotate_snippets::Level;
use ruff_annotate_snippets::Renderer;
use ruff_annotate_snippets::Snippet;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use serde::Deserialize;
use serde::Serialize;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use yansi::Paint;

use crate::config::error_kind::ErrorKind;
use crate::config::error_kind::Severity;
use crate::config::error_kind::ShownSeverity;
use crate::error::legacy::LegacyError;

/// A secondary annotation that labels a span in the same file as the primary error.
/// Used to show additional context, e.g. the types of both operands in a binary operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecondaryAnnotation {
    pub range: TextRange,
    pub label: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorQuickFix {
    ReplaceWithEnumMember { replacement: String },
    AssertNotNone,
    RemoveUnusedSuppression(SuppressionEdit),
    ImportModule { module: ModuleName },
    ReplaceDeprecatedContextManagerReturn(ContextManagerKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextManagerKind {
    Sync,
    Async,
}

impl ContextManagerKind {
    /// The iterator, generator, and explicit defaults for a return annotation.
    pub fn return_annotation_replacement(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Sync => ("Iterator", "Generator", ", None, None"),
            Self::Async => ("AsyncIterator", "AsyncGenerator", ", None"),
        }
    }
}

impl From<SuppressionEdit> for ErrorQuickFix {
    fn from(edit: SuppressionEdit) -> Self {
        Self::RemoveUnusedSuppression(edit)
    }
}

/// Whether an error was compared with the configured baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaselineStatus {
    /// No baseline was configured for this check.
    NotConfigured,
    /// A baseline was configured but was not loaded for comparison.
    NotCompared,
    /// The error did not match the loaded baseline.
    Unmatched,
    /// The error matched the loaded baseline.
    Matched,
}

/// A diagnostic as it is reported: an error, with the severity and baseline status it is
/// reported with. These can differ from the error's own, so reporting can borrow an error
/// rather than copy it to change them. Reports are built in parallel, hence `Sync`.
pub trait ReportedError: Sync {
    fn error(&self) -> &Error;
    fn severity(&self) -> Severity;
    fn baseline_status(&self) -> BaselineStatus;
}

impl ReportedError for Error {
    fn error(&self) -> &Error {
        self
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn baseline_status(&self) -> BaselineStatus {
        self.baseline_status
    }
}

impl BaselineStatus {
    /// Value for the legacy JSON `baselined` field.
    /// `None` omits the field (no baseline configured), `Some(true/false)` emits it.
    pub fn legacy_baselined_flag(self) -> Option<bool> {
        match self {
            Self::NotConfigured => None,
            Self::Matched => Some(true),
            Self::NotCompared | Self::Unmatched => Some(false),
        }
    }

    /// Suffix appended in text renderers, e.g. `" [baselined]"`.
    pub fn display_suffix(self) -> &'static str {
        match self {
            Self::Matched => " [baselined]",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Error {
    module: Module,
    range: TextRange,
    display_range: DisplayRange,
    error_kind: ErrorKind,
    severity: Severity,
    baseline_status: BaselineStatus,
    /// First line of the error message
    msg_header: Box<str>,
    /// The rest of the error message after the first line.
    /// Note that this is formatted for pretty-printing, with two spaces at the beginning and after every newline.
    msg_details: Option<Box<str>>,
    /// Additional labeled spans in the same file for richer diagnostics.
    secondary_annotations: Vec<SecondaryAnnotation>,
    /// Structured fixes used by editor and command integrations.
    quick_fixes: Vec<ErrorQuickFix>,
    /// Whether to mark `range` with the LSP `DEPRECATED` tag, which editors render as a
    /// strikethrough. This is only correct when `range` is the reference to the deprecated
    /// symbol, so it is cleared for a deprecation reached through an implicit dunder call,
    /// which is reported at the whole enclosing expression.
    deprecated_tag: bool,
}

/// An error representation that preserves the data needed for supported CLI output formats.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SerializableError {
    legacy_error: LegacyError,
    origin: String,
    details: Option<String>,
    primary_snippet: SerializableSnippet,
    additional_snippets: Vec<SerializableSnippet>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SerializableSnippet {
    source: String,
    line_start: usize,
    primary: Option<SerializableSpan>,
    secondary: Vec<SerializableAnnotation>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SerializableSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SerializableAnnotation {
    span: SerializableSpan,
    label: String,
}

struct SnippetLayout<'a> {
    from_line: LineNumber,
    to_line: LineNumber,
    primary: Option<TextRange>,
    secondary: Vec<&'a SecondaryAnnotation>,
}

struct SnippetData<'a> {
    source: &'a str,
    line_start: usize,
    primary: Option<SerializableSpan>,
    secondary: Vec<(SerializableSpan, &'a str)>,
}

#[derive(Clone, Copy)]
struct RenderMetadata<'a> {
    severity: Severity,
    header: &'a str,
    name: &'a str,
    baselined: bool,
}

impl Ranged for Error {
    fn range(&self) -> TextRange {
        self.range
    }
}

/// Where an error is, as rendered: its path, and its cell for an error in a notebook.
struct Origin<'a> {
    path: &'a Path,
    cell: Option<NonZeroU32>,
}

impl Display for Origin<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.to_string_lossy())?;
        if let Some(cell) = self.cell {
            write!(f, "#{cell}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ErrorRenderer<W> {
    writer: W,
    mode: ErrorRenderMode,
    snippets: Renderer,
}

#[derive(Clone, Copy, Debug)]
enum ErrorRenderMode {
    Plain,
    Color,
}

impl<W: Write> ErrorRenderer<W> {
    pub fn new(writer: W, color_choice: anstream::ColorChoice) -> Self {
        match color_choice {
            anstream::ColorChoice::Never => Self::plain(writer),
            anstream::ColorChoice::Always
            | anstream::ColorChoice::AlwaysAnsi
            | anstream::ColorChoice::Auto => Self::styled(writer),
        }
    }

    pub fn plain(writer: W) -> Self {
        Self {
            writer,
            mode: ErrorRenderMode::Plain,
            snippets: Renderer::plain(),
        }
    }

    pub fn styled(writer: W) -> Self {
        Self {
            writer,
            mode: ErrorRenderMode::Color,
            snippets: Renderer::styled(),
        }
    }

    fn with_mode(writer: W, mode: ErrorRenderMode) -> Self {
        match mode {
            ErrorRenderMode::Plain => Self::plain(writer),
            ErrorRenderMode::Color => Self::styled(writer),
        }
    }

    pub fn write(
        &mut self,
        reported: &impl ReportedError,
        project_root: &Path,
        verbose: bool,
    ) -> io::Result<()> {
        let severity = reported.severity();
        if !severity.is_enabled() {
            return Ok(());
        }
        let error = reported.error();
        let metadata = RenderMetadata {
            severity,
            header: &error.msg_header,
            name: error.error_kind.to_name(),
            baselined: reported.baseline_status() == BaselineStatus::Matched,
        };
        if verbose {
            let origin = error.path_string_with_fragment(project_root);
            self.write_header(&metadata)?;
            self.write_snippet(error.render_snippets(&origin, severity))?;
            if let Some(details) = &error.msg_details {
                writeln!(self.writer, "{details}")?;
            }
        } else {
            self.write_concise(&metadata, error.origin(project_root), &error.display_range)?;
        }
        Ok(())
    }

    /// Write every error in `errors`, in order. On a thread pool, many errors are rendered in
    /// parallel, each chunk into its own buffer, and the buffers are written in order. The work
    /// is done in waves, so only a wave's buffers are held at once.
    pub fn write_all(
        &mut self,
        errors: &[impl ReportedError],
        project_root: &Path,
        verbose: bool,
    ) -> io::Result<()> {
        const CHUNK: usize = 256;
        if errors.len() <= CHUNK || !in_thread_pool() {
            for error in errors {
                self.write(error, project_root, verbose)?;
            }
            return Ok(());
        }
        let mode = self.mode;
        for wave in errors.chunks(CHUNK * rayon::current_num_threads() * 4) {
            let buffers: Vec<Vec<u8>> = wave
                .par_chunks(CHUNK)
                .map(|chunk| {
                    let mut renderer = ErrorRenderer::with_mode(Vec::new(), mode);
                    for error in chunk {
                        renderer
                            .write(error, project_root, verbose)
                            .expect("writing to a `Vec` cannot fail");
                    }
                    renderer.writer
                })
                .collect();
            for buffer in buffers {
                self.writer.write_all(&buffer)?;
            }
        }
        Ok(())
    }

    pub fn write_serializable(
        &mut self,
        error: &SerializableError,
        verbose: bool,
    ) -> io::Result<()> {
        let legacy_error = error.legacy_error();
        let severity = legacy_error.severity();
        if !severity.is_enabled() {
            return Ok(());
        }
        let metadata = RenderMetadata {
            severity,
            header: legacy_error.concise_description(),
            name: &legacy_error.name,
            baselined: legacy_error.is_baselined(),
        };
        if verbose {
            self.write_header(&metadata)?;
            self.write_serializable_snippets(error)?;
            if let Some(details) = &error.details {
                writeln!(self.writer, "{details}")?;
            }
        } else {
            self.write_concise(&metadata, &error.origin, legacy_error.display_range())?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_header(&mut self, metadata: &RenderMetadata) -> io::Result<()> {
        let RenderMetadata {
            severity,
            header,
            name,
            baselined,
        } = *metadata;
        match self.mode {
            ErrorRenderMode::Plain => writeln!(
                self.writer,
                "{} {} [{}]{}",
                severity.label(),
                header,
                name,
                if baselined { " [baselined]" } else { "" },
            ),
            ErrorRenderMode::Color => {
                write!(
                    self.writer,
                    "{} {} {}",
                    severity.painted(),
                    Paint::new(header),
                    Paint::dim(format!("[{name}]").as_str()),
                )?;
                if baselined {
                    write!(self.writer, " {}", Paint::dim("[baselined]"))?;
                }
                writeln!(self.writer)
            }
        }
    }

    fn write_concise(
        &mut self,
        metadata: &RenderMetadata,
        origin: impl Display,
        display_range: impl Display,
    ) -> io::Result<()> {
        let RenderMetadata {
            severity,
            header,
            name,
            baselined,
        } = *metadata;
        let header: Cow<str> = if header.contains('\n') {
            Cow::Owned(header.lines().map(str::trim).join(" "))
        } else {
            Cow::Borrowed(header.trim())
        };
        match self.mode {
            ErrorRenderMode::Plain => writeln!(
                self.writer,
                "{} {}:{}: {} [{}]{}",
                severity.label(),
                origin,
                display_range,
                header,
                name,
                if baselined { " [baselined]" } else { "" },
            ),
            ErrorRenderMode::Color => {
                write!(
                    self.writer,
                    "{} {}:{}: {} {}",
                    severity.painted(),
                    Paint::blue(&origin),
                    Paint::dim(&display_range),
                    Paint::new(&header),
                    Paint::dim(&format_args!("[{name}]")),
                )?;
                if baselined {
                    write!(self.writer, " {}", Paint::dim("[baselined]"))?;
                }
                writeln!(self.writer)
            }
        }
    }

    fn write_serializable_snippets(&mut self, error: &SerializableError) -> io::Result<()> {
        let severity = error.legacy_error.severity();
        let mut message = Group::with_level(severity_to_level(severity)).element(
            Self::render_serializable_snippet(&error.primary_snippet, &error.origin, severity),
        );
        for snippet in &error.additional_snippets {
            message = message.element(Self::render_serializable_snippet(
                snippet,
                &error.origin,
                severity,
            ));
        }
        self.write_snippet(message)
    }

    fn render_serializable_snippet<'a>(
        snippet: &'a SerializableSnippet,
        origin: &'a str,
        severity: Severity,
    ) -> Snippet<'a, Annotation<'a>> {
        let mut rendered = Snippet::source(&snippet.source)
            .line_start(snippet.line_start)
            .path(origin);
        if let Some(primary) = &snippet.primary {
            rendered = rendered
                .annotation(severity_to_annotation_kind(severity).span(primary.start..primary.end));
        }
        for annotation in &snippet.secondary {
            rendered = rendered.annotation(
                AnnotationKind::Context
                    .span(annotation.span.start..annotation.span.end)
                    .label(annotation.label.as_str()),
            );
        }
        rendered
    }

    fn write_snippet<'a>(&mut self, snippet: Group<'a>) -> io::Result<()> {
        writeln!(self.writer, "{}", self.snippets.render(&[snippet]))
    }
}

impl Error {
    /// Return the path with a cell fragment if the error is in a notebook cell.
    pub fn path_string_with_fragment(&self, project_root: &Path) -> String {
        self.origin(project_root).to_string()
    }

    /// The path relative to `project_root`, with a cell fragment if the error is in a
    /// notebook cell.
    fn origin(&self, project_root: &Path) -> Origin<'_> {
        let path = self.path().as_path();
        Origin {
            path: path.strip_prefix(project_root).unwrap_or(path),
            cell: self.display_range.start.cell(),
        }
    }

    fn snippet_layouts(&self) -> Vec<SnippetLayout<'_>> {
        // Maximum number of lines to show in a single snippet. Annotations further apart
        // than this are shown as separate snippets rather than dumping all lines in between.
        // The primary span is also capped to this many lines for very large multi-line spans.
        const MAX_LINES: u32 = 10;

        // Partition secondary annotations into nearby (shown inline with the primary span)
        // and distant (shown as separate snippets to avoid printing excessive context).
        let primary_start_line = self.display_range.start.line_within_file();
        let primary_end_line = self.display_range.end.line_within_file();
        let mut start_line = primary_start_line;
        // Cap the primary span to MAX_LINES to avoid dumping huge multi-line spans.
        let mut end_line = cmp::min(
            LineNumber::from_zero_indexed(primary_start_line.to_zero_indexed() + MAX_LINES),
            primary_end_line,
        );
        let mut nearby_annotations = Vec::new();
        let mut distant_annotations = Vec::new();
        for ann in &self.secondary_annotations {
            let ann_display = self.module.display_range(ann.range);
            let ann_start = ann_display.start.line_within_file();
            let ann_end = ann_display.end.line_within_file();
            let is_nearby = ann_start
                .to_zero_indexed()
                .abs_diff(primary_end_line.to_zero_indexed())
                <= MAX_LINES
                && ann_end
                    .to_zero_indexed()
                    .abs_diff(primary_start_line.to_zero_indexed())
                    <= MAX_LINES;
            if is_nearby {
                start_line = cmp::min(start_line, ann_start);
                end_line = cmp::max(end_line, ann_end);
                nearby_annotations.push(ann);
            } else {
                distant_annotations.push((ann, ann_display));
            }
        }

        let mut snippets = vec![SnippetLayout {
            from_line: start_line,
            to_line: end_line,
            primary: Some(self.range),
            secondary: nearby_annotations,
        }];
        for (ann, ann_display) in &distant_annotations {
            let ann_start_line = ann_display.start.line_within_file();
            let ann_end_line = ann_display.end.line_within_file();
            snippets.push(SnippetLayout {
                from_line: ann_start_line,
                to_line: ann_end_line,
                primary: None,
                secondary: vec![ann],
            });
        }
        snippets
    }

    /// Render the source snippets of this error as reported at `severity`, which can differ
    /// from the error's own severity, so that they are drawn like the reported header.
    fn render_snippets<'a>(&'a self, origin: &'a str, severity: Severity) -> Group<'a> {
        let mut layouts = self.snippet_layouts().into_iter();
        let mut message = Group::with_level(severity_to_level(severity)).element(
            self.make_snippet(
                origin,
                layouts
                    .next()
                    .expect("a diagnostic always has a primary snippet"),
                severity,
            ),
        );
        for layout in layouts {
            message = message.element(self.make_snippet(origin, layout, severity));
        }
        message
    }

    fn make_snippet<'a>(
        &'a self,
        origin: &'a str,
        layout: SnippetLayout<'a>,
        severity: Severity,
    ) -> Snippet<'a, Annotation<'a>> {
        let data = self.snippet_data(layout);
        let mut snippet = Snippet::source(data.source)
            .line_start(data.line_start)
            .path(origin);
        if let Some(primary) = data.primary {
            snippet = snippet
                .annotation(severity_to_annotation_kind(severity).span(primary.start..primary.end));
        }
        for (span, label) in data.secondary {
            snippet = snippet.annotation(
                AnnotationKind::Context
                    .span(span.start..span.end)
                    .label(label),
            );
        }
        snippet
    }

    fn snippet_data<'a>(&'a self, layout: SnippetLayout<'a>) -> SnippetData<'a> {
        // Warning: the source range is char indexed, while the snippet is byte indexed.
        let source = self
            .module
            .lined_buffer()
            .content_in_line_range(layout.from_line, layout.to_line);
        let line_start = self.module.lined_buffer().line_start(layout.from_line);
        let cell_line = self
            .module
            .display_range(TextRange::new(line_start, line_start))
            .start
            .line_within_cell()
            .get() as usize;
        let primary = layout.primary.map(|range| {
            let start = (range.start() - line_start).to_usize();
            let end = cmp::min(start + range.len().to_usize(), source.len());
            SerializableSpan { start, end }
        });
        let secondary = layout
            .secondary
            .into_iter()
            .filter_map(|annotation| {
                let start = annotation
                    .range
                    .start()
                    .to_usize()
                    .saturating_sub(line_start.to_usize());
                let end = cmp::min(start + annotation.range.len().to_usize(), source.len());
                (start <= end && end <= source.len())
                    .then_some((SerializableSpan { start, end }, annotation.label.as_ref()))
            })
            .collect();
        SnippetData {
            source,
            line_start: cell_line,
            primary,
            secondary,
        }
    }

    fn serializable_snippets(&self) -> (SerializableSnippet, Vec<SerializableSnippet>) {
        let mut layouts = self.snippet_layouts().into_iter();
        let primary = self.make_serializable_snippet(
            layouts
                .next()
                .expect("a diagnostic always has a primary snippet"),
        );
        let additional = layouts
            .map(|layout| self.make_serializable_snippet(layout))
            .collect();
        (primary, additional)
    }

    fn make_serializable_snippet(&self, layout: SnippetLayout<'_>) -> SerializableSnippet {
        let data = self.snippet_data(layout);
        SerializableSnippet {
            source: data.source.to_owned(),
            line_start: data.line_start,
            primary: data.primary,
            secondary: data
                .secondary
                .into_iter()
                .map(|(span, label)| SerializableAnnotation {
                    span,
                    label: label.to_owned(),
                })
                .collect(),
        }
    }

    pub fn with_severity(&self, severity: Severity) -> Self {
        let mut res = self.clone();
        res.severity = severity;
        res
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn with_baseline_status(mut self, baseline_status: BaselineStatus) -> Self {
        self.baseline_status = baseline_status;
        self
    }

    pub fn baseline_status(&self) -> BaselineStatus {
        self.baseline_status
    }

    /// The tag editors use to draw this diagnostic in their own way: struck
    /// through for a deprecated name, greyed out for inactive, unreachable, or unused code.
    pub fn editor_tag(&self) -> Option<DiagnosticTag> {
        if self.deprecated_tag {
            Some(DiagnosticTag::Deprecated)
        } else if matches!(
            self.error_kind(),
            ErrorKind::InactiveCode
                | ErrorKind::Unreachable
                | ErrorKind::UnusedImport
                | ErrorKind::UnusedParameter
                | ErrorKind::UnusedVariable
        ) {
            Some(DiagnosticTag::Unnecessary)
        } else {
            None
        }
    }

    /// Create a diagnostic suitable for use in LSP, or `None` for an ignored error.
    pub fn to_diagnostic(&self) -> Option<Diagnostic> {
        self.to_diagnostic_with_severity(self.severity)
    }

    /// Like `to_diagnostic`, but with `severity` in place of this error's own severity.
    pub(crate) fn to_diagnostic_with_severity(&self, severity: Severity) -> Option<Diagnostic> {
        let severity = severity.shown()?;
        let code = self.error_kind().to_name().to_owned();
        let code_description = Uri::parse(&self.error_kind().docs_url())
            .ok()
            .map(|href| CodeDescription { href });
        let tag = self.editor_tag();
        // TODO: Map secondary_annotations to DiagnosticRelatedInformation for LSP clients.
        // This requires constructing a Uri from the module path, which may not always succeed.
        Some(Diagnostic {
            range: self.module.to_lsp_range(self.range()),
            severity: Some(match severity {
                ShownSeverity::Error => DiagnosticSeverity::Error,
                ShownSeverity::Warn => DiagnosticSeverity::Warning,
                ShownSeverity::Hint if tag.is_some() => DiagnosticSeverity::Hint,
                // VS Code draws an untagged LSP hint as three small dots under its first
                // characters, with no underline, so a hint without a tag is shown like `info`.
                ShownSeverity::Info | ShownSeverity::Hint => DiagnosticSeverity::Information,
            }),
            source: Some("Pyrefly".to_owned()),
            message: self.msg().to_owned().into(),
            code: Some(lsp_types::Code::String(code)),
            code_description,
            tags: tag.map(|tag| vec![tag]),
            ..Default::default()
        })
    }

    pub fn get_notebook_cell(&self) -> Option<usize> {
        self.module.to_cell_for_lsp(self.range().start())
    }

    pub fn module(&self) -> &Module {
        &self.module
    }
}

#[cfg(test)]
pub fn print_errors(project_root: &Path, errors: &[Error]) {
    let mut buf = Vec::new();
    {
        let mut renderer = ErrorRenderer::new(&mut buf, anstream::stdout().current_choice());
        for err in errors {
            renderer.write(err, project_root, true).unwrap();
        }
        renderer.flush().unwrap();
    }
    // Use print! so Rust's test runner captures the output and shows it
    // on test failure. Direct writes to stdout (e.g. via ErrorRenderer +
    // stdout.lock()) bypass test capture and are invisible in test output.
    if !buf.is_empty() {
        print!("{}", String::from_utf8_lossy(&buf));
    }
}

fn count_error_kinds(errors: &[impl ReportedError]) -> Vec<(ErrorKind, usize)> {
    let mut map = SmallMap::new();
    for err in errors {
        let kind = err.error().error_kind();
        *map.entry(kind).or_default() += 1;
    }
    let mut res = map.into_iter().collect::<Vec<_>>();
    res.sort_by_key(|x| x.1);
    res
}

pub fn print_error_counts(errors: &[impl ReportedError], limit: usize) {
    let items = count_error_kinds(errors);
    let limit = if limit > 0 { limit } else { items.len() };
    for (error, count) in items.iter().rev().take(limit) {
        eprintln!(
            "{} instances of {}",
            number_thousands(*count),
            error.to_name()
        );
    }
}

impl Error {
    pub fn new(
        module: Module,
        range: TextRange,
        header: String,
        details: Vec<String>,
        error_kind: ErrorKind,
    ) -> Self {
        let display_range = module.display_range(range);
        let msg_header = header.into_boxed_str();
        let msg_details = if details.is_empty() {
            None
        } else {
            Some(
                details
                    .iter()
                    .map(|s| format!("  {s}"))
                    .join("\n")
                    .into_boxed_str(),
            )
        };
        Self {
            module,
            range,
            display_range,
            error_kind,
            severity: error_kind.default_severity(),
            baseline_status: BaselineStatus::NotConfigured,
            msg_header,
            msg_details,
            secondary_annotations: Vec::new(),
            quick_fixes: Vec::new(),
            deprecated_tag: error_kind == ErrorKind::Deprecated,
        }
    }

    /// Stop marking this error's range as deprecated in editors. Used when the range spans
    /// more than the deprecated symbol, so striking it through would obscure unrelated code.
    pub fn without_deprecated_tag(mut self) -> Self {
        self.deprecated_tag = false;
        self
    }

    /// Add a secondary labeled annotation to this error. These appear as additional
    /// underlined spans with labels in the source snippet.
    pub fn with_annotation(mut self, range: TextRange, label: String) -> Self {
        self.secondary_annotations.push(SecondaryAnnotation {
            range,
            label: label.into_boxed_str(),
        });
        self
    }

    pub fn with_quick_fix(mut self, quick_fix: ErrorQuickFix) -> Self {
        self.quick_fixes.push(quick_fix);
        self
    }

    /// Merge editor fixes when both values describe the same user-facing diagnostic.
    pub(crate) fn merge_if_same_diagnostic(&mut self, other: &Self) -> bool {
        if self.module != other.module
            || self.range != other.range
            || self.display_range != other.display_range
            || self.error_kind != other.error_kind
            || self.severity != other.severity
            || self.msg_header != other.msg_header
            || self.msg_details != other.msg_details
            || self.secondary_annotations != other.secondary_annotations
            || self.deprecated_tag != other.deprecated_tag
        {
            return false;
        }
        for fix in &other.quick_fixes {
            if !self.quick_fixes.contains(fix) {
                self.quick_fixes.push(fix.clone());
            }
        }
        true
    }

    pub fn display_range(&self) -> &DisplayRange {
        &self.display_range
    }

    pub fn lined_buffer(&self) -> &LinedBuffer {
        self.module.lined_buffer()
    }

    pub fn path(&self) -> &ModulePath {
        self.module.path()
    }

    pub fn msg_header(&self) -> &str {
        &self.msg_header
    }

    pub fn msg_details(&self) -> Option<&str> {
        self.msg_details.as_deref()
    }

    pub fn msg(&self) -> String {
        if let Some(details) = &self.msg_details {
            format!("{}\n{}", self.msg_header, details)
        } else {
            (*self.msg_header).to_owned()
        }
    }

    pub fn suppression_effect(
        &self,
        enabled_ignores: &SmallSet<Tool>,
        type_ignore_unknown_tag_behavior: TypeIgnoreUnknownTagBehavior,
    ) -> SuppressionEffect {
        // Suppressing an unused-ignore error would only create another one.
        if self.error_kind.is_unused_ignore() {
            return SuppressionEffect::None;
        }
        // Check both this kind's name and any parent kind's name, so that e.g.
        // `# pyrefly: ignore[bad-override]` also suppresses `bad-override-mutable-attribute`.
        self.error_kind
            .suppression_names()
            .map(|name| {
                self.module.suppression_effect(
                    &self.display_range,
                    name,
                    enabled_ignores,
                    type_ignore_unknown_tag_behavior,
                )
            })
            .max()
            .unwrap_or(SuppressionEffect::None)
    }

    pub fn error_kind(&self) -> ErrorKind {
        self.error_kind
    }

    /// Return the secondary annotations attached to this error.
    pub fn secondary_annotations(&self) -> &[SecondaryAnnotation] {
        &self.secondary_annotations
    }

    pub fn quick_fixes(&self) -> &[ErrorQuickFix] {
        &self.quick_fixes
    }
}

impl SerializableError {
    pub fn from_error(relative_to: &Path, error: &Error) -> Self {
        let (primary_snippet, additional_snippets) = error.serializable_snippets();
        Self {
            legacy_error: LegacyError::from_error(relative_to, error),
            origin: error.path_string_with_fragment(relative_to),
            details: error.msg_details().map(str::to_owned),
            primary_snippet,
            additional_snippets,
        }
    }

    pub fn legacy_error(&self) -> &LegacyError {
        &self.legacy_error
    }

    pub fn into_legacy_error(self) -> LegacyError {
        self.legacy_error
    }
}

fn severity_to_level(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Error => Level::ERROR,
        Severity::Warn => Level::WARNING,
        Severity::Info => Level::INFO,
        Severity::Hint => Level::NOTE.with_name("hint"),
        Severity::Ignore => Level::NOTE.no_name(),
    }
}

fn severity_to_annotation_kind(severity: Severity) -> AnnotationKind {
    match severity {
        Severity::Error | Severity::Warn | Severity::Ignore => AnnotationKind::Primary,
        Severity::Hint | Severity::Info => AnnotationKind::Context,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use dupe::Dupe;
    use pyrefly_python::module_name::ModuleName;
    use pyrefly_util::thread_pool::TEST_THREAD_COUNT;
    use pyrefly_util::thread_pool::ThreadPool;
    use ruff_text_size::TextSize;

    use super::*;
    use crate::error::collector::CollectedDiagnostic;
    use crate::test::util::TestEnv;

    fn render_error(error: &Error, root: &Path, verbose: bool) -> String {
        let mut output = Vec::new();
        {
            let mut renderer = ErrorRenderer::plain(&mut output);
            renderer.write(error, root, verbose).unwrap();
        }
        str::from_utf8(&output).unwrap().to_owned()
    }

    fn render_serializable_error(error: &Error, root: &Path, verbose: bool) -> String {
        let error = SerializableError::from_error(root, error);
        let mut output = Vec::new();
        {
            let mut renderer = ErrorRenderer::plain(&mut output);
            renderer.write_serializable(&error, verbose).unwrap();
        }
        str::from_utf8(&output).unwrap().to_owned()
    }

    /// Errors in two modules, enough of them that `ErrorRenderer::write_all` renders them in
    /// parallel, some with details and some with a header spanning lines.
    fn many_errors() -> Vec<Error> {
        let contents = Arc::new("x = 1\n".repeat(1000));
        ["foo", "bar"]
            .into_iter()
            .flat_map(|name| {
                let module = Module::new(
                    ModuleName::from_str(name),
                    ModulePath::filesystem(PathBuf::from(format!("/root/{name}.py"))),
                    contents.dupe(),
                );
                (0..400u32).map(move |i| {
                    let start = TextSize::new(i * 6);
                    Error::new(
                        module.dupe(),
                        TextRange::new(start, start + TextSize::new(1)),
                        format!("error {i} with \"quotes\"\n  over two lines"),
                        if i % 2 == 0 {
                            vec![format!("detail {i}")]
                        } else {
                            Vec::new()
                        },
                        ErrorKind::BadAssignment,
                    )
                })
            })
            .collect()
    }

    #[test]
    fn test_write_all_matches_writing_each_error() {
        let errors = many_errors();
        for verbose in [false, true] {
            let mut each = Vec::new();
            let mut renderer = ErrorRenderer::plain(&mut each);
            for error in &errors {
                renderer.write(error, Path::new("/root"), verbose).unwrap();
            }
            // Errors are rendered in parallel only on a thread pool.
            let mut all = Vec::new();
            ThreadPool::new(TEST_THREAD_COUNT).install(|| {
                ErrorRenderer::plain(&mut all)
                    .write_all(&errors, Path::new("/root"), verbose)
                    .unwrap()
            });
            assert!(
                all == each,
                "rendering in parallel should not change the output (verbose: {verbose})"
            );
        }
    }

    #[test]
    fn test_multiline_header_is_flattened_only_in_concise_output() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("x".to_owned()),
        );
        let error = Error::new(
            module_info,
            TextRange::new(TextSize::new(0), TextSize::new(1)),
            "revealed type: Overload[\n  (x: int) -> str\n]".to_owned(),
            Vec::new(),
            ErrorKind::RevealType,
        );

        let concise = render_error(&error, Path::new(""), false);
        assert_eq!(concise.lines().count(), 1);
        assert!(concise.contains("revealed type: Overload[ (x: int) -> str ]"));

        let verbose = render_error(&error, Path::new(""), true);
        assert!(verbose.contains("revealed type: Overload[\n  (x: int) -> str\n]"));
    }

    #[test]
    fn test_error_render() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("def f(x: int) -> str:\n    return x".to_owned()),
        );
        let error = Error::new(
            module_info,
            TextRange::new(TextSize::new(26), TextSize::new(34)),
            "bad return".to_owned(),
            Vec::new(),
            ErrorKind::BadReturn,
        );
        let root = PathBuf::new();
        let normal = render_error(&error, root.as_path(), false);
        let verbose = render_error(&error, root.as_path(), true);

        assert_eq!(normal, "ERROR test.py:2:5-13: bad return [bad-return]\n");
        assert_eq!(
            render_serializable_error(&error, root.as_path(), false),
            normal
        );
        assert_eq!(
            verbose,
            r#"ERROR bad return [bad-return]
 --> test.py:2:5
  |
2 |     return x
  |     ^^^^^^^^
"#,
        );
        assert_eq!(
            render_serializable_error(&error, root.as_path(), true),
            verbose
        );
    }

    #[test]
    fn test_snippet_drawn_at_reported_severity() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("return\nx = 1".to_owned()),
        );
        let error = Error::new(
            module_info,
            TextRange::new(TextSize::new(7), TextSize::new(12)),
            "This code is unreachable".to_owned(),
            Vec::new(),
            ErrorKind::Unreachable,
        )
        .with_severity(Severity::Hint);
        let reported = CollectedDiagnostic::new(&error, Severity::Warn);
        let mut output = Vec::new();
        ErrorRenderer::plain(&mut output)
            .write(&reported, Path::new(""), true)
            .unwrap();
        assert_eq!(
            str::from_utf8(&output).unwrap(),
            r#" WARN This code is unreachable [unreachable]
 --> test.py:2:1
  |
2 | x = 1
  | ^^^^^
"#,
        );
    }

    #[test]
    fn test_baselined_error_render() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("x: str = 1".to_owned()),
        );
        let error = Error::new(
            module_info,
            TextRange::new(TextSize::new(9), TextSize::new(10)),
            "bad assignment".to_owned(),
            Vec::new(),
            ErrorKind::BadAssignment,
        )
        .with_baseline_status(BaselineStatus::Matched);

        assert_eq!(
            render_error(&error, Path::new(""), false),
            "ERROR test.py:1:10-11: bad assignment [bad-assignment] [baselined]\n"
        );
        assert!(
            render_error(&error, Path::new(""), true)
                .starts_with("ERROR bad assignment [bad-assignment] [baselined]\n")
        );
    }

    #[test]
    fn test_error_too_long() {
        let contents = format!("Start\n{}\nEnd", "X\n".repeat(1000));
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new(contents.clone()),
        );
        let error = Error::new(
            module_info,
            TextRange::new(TextSize::new(0), TextSize::new(contents.len() as u32)),
            "oops".to_owned(),
            Vec::new(),
            ErrorKind::BadReturn,
        );
        let root = PathBuf::new();
        let output = render_error(&error, root.as_path(), true);

        assert_eq!(
            output,
            r#"ERROR oops [bad-return]
  --> test.py:1:1
   |
 1 | / Start
 2 | | X
 3 | | X
 4 | | X
...  |
10 | | X
11 | | X
   | |__^
"#,
        );
    }

    #[test]
    fn test_error_with_secondary_annotations() {
        // Source: "val * 2" where val is at bytes 0..3, * at 4, 2 at 6
        let source = "val * 2";
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new(source.to_owned()),
        );
        let error = Error::new(
            module_info,
            // Primary span covers the whole expression
            TextRange::new(TextSize::new(0), TextSize::new(7)),
            "`*` is not supported between `int | str` and `int`".to_owned(),
            Vec::new(),
            ErrorKind::UnsupportedOperation,
        )
        .with_annotation(
            TextRange::new(TextSize::new(0), TextSize::new(3)),
            "has type `int | str`".to_owned(),
        )
        .with_annotation(
            TextRange::new(TextSize::new(6), TextSize::new(7)),
            "has type `int`".to_owned(),
        );
        let root = PathBuf::new();
        let output = render_error(&error, root.as_path(), true);

        assert_eq!(
            output,
            r#"ERROR `*` is not supported between `int | str` and `int` [unsupported-operation]
 --> test.py:1:1
  |
1 | val * 2
  | ---^^^-
  | |     |
  | |     has type `int`
  | has type `int | str`
"#,
        );
        assert_eq!(
            render_serializable_error(&error, root.as_path(), true),
            output
        );
    }

    /// Integration test: verify that binary operator errors from the type checker
    /// produce secondary annotations labeling both operands with their types.
    #[test]
    fn test_binop_error_has_type_annotations() {
        let code = r#"
def f(x: None) -> None:
    y = x * 2  # E: `*` is not supported between `None` and `Literal[2]`
"#;
        let (state, handle) = TestEnv::one("main", code).to_state();
        let errors = state
            .transaction()
            .get_errors(&[handle("main")])
            .collect_errors()
            .ordinary;
        assert_eq!(errors.len(), 1);
        let err = &errors[0];
        let annotations = err.secondary_annotations();
        assert_eq!(annotations.len(), 2);
        assert_eq!(&*annotations[0].label, "has type `None`");
        assert_eq!(&*annotations[1].label, "has type `Literal[2]`");
    }

    // https://github.com/facebook/pyrefly/issues/5113
    #[test]
    fn test_hint_severity_in_lsp() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("x".to_owned()),
        );
        let diagnostic = |kind| {
            Error::new(
                module_info.clone(),
                TextRange::new(TextSize::new(0), TextSize::new(1)),
                "message".to_owned(),
                Vec::new(),
                kind,
            )
            .with_severity(Severity::Hint)
            .to_diagnostic()
            .expect("a severity other than ignore is shown")
        };
        let unused = diagnostic(ErrorKind::UnusedImport);
        assert_eq!(unused.severity, Some(DiagnosticSeverity::Hint));
        assert_eq!(unused.tags, Some(vec![DiagnosticTag::Unnecessary]));
        let assignment = diagnostic(ErrorKind::BadAssignment);
        assert_eq!(assignment.severity, Some(DiagnosticSeverity::Information));
        assert_eq!(assignment.tags, None);
    }

    #[test]
    fn test_ignored_error_has_no_lsp_diagnostic() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("x".to_owned()),
        );
        let error = Error::new(
            module_info,
            TextRange::new(TextSize::new(0), TextSize::new(1)),
            "message".to_owned(),
            Vec::new(),
            ErrorKind::BadAssignment,
        );
        assert!(error.to_diagnostic().is_some());
        assert!(
            error
                .with_severity(Severity::Ignore)
                .to_diagnostic()
                .is_none(),
            "an ignored error is never published"
        );
    }

    // https://github.com/facebook/pyrefly/issues/5113
    #[test]
    fn test_deprecated_and_unreachable_in_lsp() {
        let module_info = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(PathBuf::from("test.py")),
            Arc::new("x".to_owned()),
        );
        let diagnostic = |kind| {
            Error::new(
                module_info.clone(),
                TextRange::new(TextSize::new(0), TextSize::new(1)),
                "message".to_owned(),
                Vec::new(),
                kind,
            )
            .to_diagnostic()
            .expect("a severity other than ignore is shown")
        };
        let deprecated = diagnostic(ErrorKind::Deprecated);
        assert_eq!(deprecated.severity, Some(DiagnosticSeverity::Warning));
        assert_eq!(deprecated.tags, Some(vec![DiagnosticTag::Deprecated]));
        let unreachable = diagnostic(ErrorKind::Unreachable);
        assert_eq!(unreachable.severity, Some(DiagnosticSeverity::Hint));
        assert_eq!(unreachable.tags, Some(vec![DiagnosticTag::Unnecessary]));
    }
}
