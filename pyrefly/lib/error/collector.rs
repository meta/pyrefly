/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fmt::Debug;
use std::mem;
use std::sync::Arc;

use dupe::Dupe;
use pyrefly_config::error_kind::ErrorKind;
use pyrefly_python::ignore::Suppression;
use pyrefly_python::ignore::SuppressionEffect;
use pyrefly_python::ignore::Tool;
use pyrefly_util::lined_buffer::LineNumber;
use pyrefly_util::lock::Mutex;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;

use crate::config::error::ErrorConfig;
use crate::config::error_kind::Severity;
use crate::error::context::ErrorContext;
use crate::error::error::BaselineStatus;
use crate::error::error::Error;
use crate::error::error::ErrorQuickFix;
use crate::error::error::ReportedError;
use crate::error::style::ErrorStyle;
use crate::module::module_info::ModuleInfo;
use crate::state::errors::find_containing_range;

#[derive(Debug, Default)]
struct ModuleErrors {
    /// Set to `true` when we have no duplicates and are sorted.
    clean: bool,
    items: Vec<Error>,
}

/// Finalized diagnostics produced by one computation step.
#[derive(Clone, Dupe, Debug, Default)]
pub struct ModuleDiagnostics(Arc<[Error]>);

impl ModuleDiagnostics {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Error> {
        self.0.iter()
    }

    /// Replay finalized diagnostics into an inference attempt while retaining the snapshot.
    pub(crate) fn replay(&self, errors: &ErrorCollector) {
        if errors.is_active() {
            let replayed = ModuleErrors {
                clean: true,
                items: self.0.to_vec(),
            };
            errors.errors.lock().extend(replayed);
        }
    }
}

impl ModuleErrors {
    fn push(&mut self, err: Error) {
        self.clean = false;
        self.items.push(err);
    }

    fn extend(&mut self, errs: ModuleErrors) {
        self.clean = false;
        self.items.extend(errs.items);
    }

    fn cleanup(&mut self) {
        if self.clean {
            return;
        }
        self.clean = true;
        // We want to sort only by source-range, not by message.
        // When we get an overload error, we want that overload to remain before whatever the precise overload failure is.
        self.items
            .sort_by_key(|x| (x.range().start(), x.range().end()));

        // Within a single source range we want to dedupe, even if the error messages aren't adjacent
        let mut res = Vec::with_capacity(self.items.len());
        mem::swap(&mut res, &mut self.items);

        // The range and where that range started in self.items
        let mut previous_range = TextRange::default();
        let mut previous_start = 0;
        for x in res {
            if x.range() != previous_range {
                previous_range = x.range();
                previous_start = self.items.len();
                self.items.push(x);
            } else if !self.items[previous_start..]
                .iter_mut()
                .any(|existing| existing.merge_if_same_diagnostic(&x))
            {
                self.items.push(x);
            }
        }
    }

    fn is_empty(&self) -> bool {
        // No need to do cleanup if it's empty.
        self.items.is_empty()
    }

    fn len(&mut self) -> usize {
        self.cleanup();
        self.items.len()
    }

    fn len_hard(&mut self) -> usize {
        self.cleanup();
        self.items
            .iter()
            .filter(|err| !err.error_kind().is_soft())
            .count()
    }

    fn has_hard(&mut self) -> bool {
        self.cleanup();
        self.items.iter().any(|err| !err.error_kind().is_soft())
    }

    /// Iterates over all errors, including ignored ones.
    fn iter(&mut self) -> impl ExactSizeIterator<Item = &Error> {
        self.cleanup();
        self.items.iter()
    }
}

/// For each error, whether it is an `unknown-from-error` error that a counted error within its
/// range explains. Such an error reports a value whose type comes from another error, and an
/// error within its range is the cause or a more precise report of the same unknown value. At an
/// equal range, only an error of another kind counts. `items` must be sorted by range.
fn unknown_from_error_explained(items: &[&Error], counts: impl Fn(ErrorKind) -> bool) -> Vec<bool> {
    // The candidates for each error start at the first item that does not start before it.
    items
        .iter()
        .map(|x| {
            if x.error_kind() != ErrorKind::UnknownFromError {
                return false;
            }
            let range = x.range();
            let first = items.partition_point(|y| y.range().start() < range.start());
            // `x` itself has an equal range and the same kind, so the last condition skips it.
            items[first..]
                .iter()
                .take_while(|y| y.range().start() <= range.end())
                .any(|y| {
                    counts(y.error_kind())
                        && range.contains_range(y.range())
                        && (y.range() != range || y.error_kind() != ErrorKind::UnknownFromError)
                })
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct CollectedErrors {
    /// Ordinary diagnostics (errors, warnings, info) that passed severity and
    /// suppression filters. These participate in baseline exclusion,
    /// suppression, and min-severity filtering.
    pub ordinary: Vec<Error>,
    /// Diagnostics at `Severity::Hint` that passed suppression filters. Hints are
    /// meant for the IDE, so only the surfaces that can show them (the language
    /// server, the playground and `pyrefly check`) read them.
    pub hints: Vec<Error>,
    /// Directive diagnostics (e.g. `reveal_type`) that are always displayed to
    /// the user. Directives are never subject to baseline exclusion,
    /// suppression, or min-severity filtering.
    pub directives: Vec<Error>,
    /// Errors that are suppressed with inline ignore comments.
    pub suppressed: Vec<Error>,
    /// Errors that are disabled with configuration options.
    pub disabled: Vec<Error>,
    /// Errors that are suppressed by baseline file.
    pub baseline: Vec<Error>,
}

/// Concatenate the vectors that `field` picks out of `parts`, allocating the result once.
fn concat<P, T>(parts: &mut [P], field: impl Fn(&mut P) -> &mut Vec<T>) -> Vec<T> {
    let mut result = Vec::with_capacity(parts.iter_mut().map(|part| field(part).len()).sum());
    for part in parts {
        result.append(field(part));
    }
    result
}

impl CollectedErrors {
    /// Concatenate each category of `parts`.
    pub(crate) fn concat(mut parts: Vec<Self>) -> Self {
        Self {
            ordinary: concat(&mut parts, |part| &mut part.ordinary),
            hints: concat(&mut parts, |part| &mut part.hints),
            directives: concat(&mut parts, |part| &mut part.directives),
            suppressed: concat(&mut parts, |part| &mut part.suppressed),
            disabled: concat(&mut parts, |part| &mut part.disabled),
            baseline: concat(&mut parts, |part| &mut part.baseline),
        }
    }
}

/// A reported diagnostic that borrows its error, so that reporting can change the severity
/// and baseline status it is reported with without copying the error.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CollectedDiagnostic<'a> {
    error: &'a Error,
    severity: Severity,
    baseline_status: BaselineStatus,
    /// The match's `baseline-error-level` is unset. An open file shows it as an
    /// LSP hint, which `to_diagnostic_with_severity` does not produce for an
    /// untagged hint.
    unconfigured_baseline_hint: bool,
}

impl<'a> CollectedDiagnostic<'a> {
    pub(crate) fn new(error: &'a Error, severity: Severity) -> Self {
        Self {
            error,
            severity,
            baseline_status: error.baseline_status(),
            unconfigured_baseline_hint: false,
        }
    }

    pub(crate) fn error(&self) -> &'a Error {
        self.error
    }

    pub(crate) fn severity(&self) -> Severity {
        self.severity
    }

    pub(crate) fn with_severity(self, severity: Severity) -> Self {
        Self { severity, ..self }
    }

    pub(crate) fn with_baseline_status(self, baseline_status: BaselineStatus) -> Self {
        Self {
            baseline_status,
            ..self
        }
    }

    pub(crate) fn with_unconfigured_baseline_hint(self) -> Self {
        Self {
            unconfigured_baseline_hint: true,
            ..self
        }
    }

    pub(crate) fn unconfigured_baseline_hint(self) -> bool {
        self.unconfigured_baseline_hint
    }

    pub(crate) fn into_error(self) -> Error {
        self.error
            .with_severity(self.severity)
            .with_baseline_status(self.baseline_status)
    }
}

impl ReportedError for CollectedDiagnostic<'_> {
    fn error(&self) -> &Error {
        self.error
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn baseline_status(&self) -> BaselineStatus {
        self.baseline_status
    }
}

/// Like `CollectedErrors`, but borrowing each diagnostic from its module.
#[derive(Debug, Default)]
pub(crate) struct BorrowedCollectedErrors<'a> {
    pub(crate) ordinary: Vec<CollectedDiagnostic<'a>>,
    pub(crate) hints: Vec<CollectedDiagnostic<'a>>,
    pub(crate) directives: Vec<CollectedDiagnostic<'a>>,
    pub(crate) suppressed: Vec<CollectedDiagnostic<'a>>,
    pub(crate) disabled: Vec<CollectedDiagnostic<'a>>,
}

impl BorrowedCollectedErrors<'_> {
    /// Concatenate each category of `parts`.
    pub(crate) fn concat(mut parts: Vec<Self>) -> Self {
        Self {
            ordinary: concat(&mut parts, |part| &mut part.ordinary),
            hints: concat(&mut parts, |part| &mut part.hints),
            directives: concat(&mut parts, |part| &mut part.directives),
            suppressed: concat(&mut parts, |part| &mut part.suppressed),
            disabled: concat(&mut parts, |part| &mut part.disabled),
        }
    }

    pub(crate) fn into_owned(self, result: &mut CollectedErrors) {
        fn owned<'a>(
            errors: Vec<CollectedDiagnostic<'a>>,
        ) -> impl Iterator<Item = Error> + use<'a> {
            errors.into_iter().map(CollectedDiagnostic::into_error)
        }

        result.ordinary.extend(owned(self.ordinary));
        result.hints.extend(owned(self.hints));
        result.directives.extend(owned(self.directives));
        result.suppressed.extend(owned(self.suppressed));
        result.disabled.extend(owned(self.disabled));
    }
}

/// Collects the user errors (e.g. type errors) associated with a module.
///
/// IMPORTANT: to avoid accidentally duplicating or dropping errors, or coupling semantic decisions
/// to diagnostic collection, the error collector:
/// - Deliberately does not implement `Clone`.
/// - Avoids examining specific errors for the purpose of influencing downstream analysis. I.e., do
///   not add anything like a non-private `has_error_message` or `has_error_kind`.
/// - Only consumes other error collectors (i.e., takes ownership of them when taking errors). Do
///   not copy errors from borrowed collectors.
#[derive(Debug)]
pub struct ErrorCollector {
    module_info: ModuleInfo,
    style: ErrorStyle,
    errors: Mutex<ModuleErrors>,
}

impl ErrorCollector {
    pub fn new(module_info: ModuleInfo, style: ErrorStyle) -> Self {
        Self {
            module_info,
            style,
            errors: Mutex::new(Default::default()),
        }
    }

    pub fn is_active(&self) -> bool {
        self.style != ErrorStyle::Never
    }

    pub fn finish(self) -> ModuleDiagnostics {
        let mut errors = self.errors.into_inner();
        errors.cleanup();
        ModuleDiagnostics(errors.items.into())
    }

    /// Remove and finalize the collector's accumulated diagnostics.
    pub(crate) fn take(&self) -> ModuleDiagnostics {
        let mut errors = mem::take(&mut *self.errors.lock());
        errors.cleanup();
        ModuleDiagnostics(errors.items.into())
    }

    pub fn extend(&self, other: ErrorCollector) {
        if self.is_active() {
            self.errors.lock().extend(other.errors.into_inner());
        }
    }

    /// Run `commit` while draining is blocked, and append `other` only if it
    /// succeeds. If it fails, return `other` so the caller can retry. `commit`
    /// must not block, because other threads take this lock to publish.
    pub(crate) fn extend_if<T, E>(
        &self,
        mut other: ErrorCollector,
        commit: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, (E, ErrorCollector)> {
        // With nothing to append, `commit` has no diagnostics to make visible
        // with it, so it does not need to block collection.
        if !self.is_active() || other.errors.get_mut().is_empty() {
            return commit().map_err(|failure| (failure, other));
        }
        let mut errors = self.errors.lock();
        match commit() {
            Ok(result) => {
                errors.extend(other.errors.into_inner());
                Ok(result)
            }
            Err(failure) => Err((failure, other)),
        }
    }

    /// Add the errors from another collector that satisfy `keep`.
    pub(crate) fn extend_filtered(
        &self,
        other: ErrorCollector,
        mut keep: impl FnMut(&Error) -> bool,
    ) {
        if self.is_active() {
            let mut other = other.errors.into_inner();
            other.items.retain(|error| keep(error));
            self.errors.lock().extend(other);
        }
    }

    /// Start building an error. Returns a no-op builder if style is Never.
    pub fn error_builder(
        &self,
        range: TextRange,
        kind: ErrorKind,
        header: String,
    ) -> ErrorBuilder<'_> {
        ErrorBuilder {
            collector: self,
            active: self.is_active(),
            range,
            kind,
            header,
            details: Vec::new(),
            context: None,
            annotations: Vec::new(),
            quick_fixes: Vec::new(),
            deprecated_tag: true,
        }
    }

    pub fn internal_error(&self, range: TextRange, header: String) {
        self.error_builder(range, ErrorKind::InternalError, header)
            .with_detail(
                "Sorry, Pyrefly encountered an internal error, \
                 this is always a bug in Pyrefly itself"
                    .to_owned(),
            )
            .with_detail(
                if cfg!(fbcode_build) {
                    "Please report the bug at https://fb.workplace.com/groups/pyreqa"
                } else {
                    "Please report the bug at https://github.com/facebook/pyrefly/issues/new"
                }
                .to_owned(),
            )
            .emit();
    }

    pub fn module(&self) -> &ModuleInfo {
        &self.module_info
    }

    pub fn style(&self) -> ErrorStyle {
        self.style
    }

    /// Report each pyrefly `ignore-errors` directive found outside the preamble, where
    /// it is inert.
    pub(crate) fn add_misplaced_ignores(&self, misplaced: &[LineNumber]) {
        for line in misplaced {
            let buffer = self.module_info.lined_buffer();
            let line_start = buffer.line_start(*line);
            // Point the diagnostic at the directive itself, from the `#` to the end of the
            // comment, rather than at the leading whitespace, so editor underlines land on it.
            let line_text = buffer.content_in_line_range(*line, *line);
            let leading_ws = (line_text.len() - line_text.trim_start().len()) as u32;
            let content_len = line_text.trim_end().len() as u32;
            let range = TextRange::new(
                line_start + TextSize::new(leading_ws),
                line_start + TextSize::new(content_len),
            );
            self.error_builder(
                range,
                ErrorKind::MisplacedIgnore,
                MISPLACED_IGNORE_MESSAGE.to_owned(),
            )
            .emit();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.errors.lock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.lock().len()
    }

    /// Count of errors excluding soft diagnostics (which should not
    /// influence overload selection or type-inference decisions).
    pub fn len_hard(&self) -> usize {
        self.errors.lock().len_hard()
    }

    /// Whether any hard (non-soft) errors exist. Short-circuits on the first match.
    pub fn has_hard(&self) -> bool {
        self.errors.lock().has_hard()
    }

    /// Checks whether an error is suppressed, considering ignore-all directives,
    /// per-line suppressions, and (for errors inside multi-line f/t-strings)
    /// suppressions on the f-string's start or end lines.
    fn suppression_effect(
        err: &Error,
        fstring_ranges: &[(LineNumber, LineNumber)],
        ignore_all: &[Suppression],
        error_config: &ErrorConfig,
    ) -> SuppressionEffect {
        // Check whole-file ignore-all directives first.
        // Unused-ignore errors cannot be suppressed to prevent infinite loops.
        if !err.error_kind().is_unused_ignore()
            && err.error_kind().suppression_names().any(|kind| {
                ignore_all.iter().any(|supp| {
                    error_config.enabled_ignores.contains(&supp.tool())
                        && match supp.tool() {
                            Tool::Pyrefly => {
                                supp.error_codes().is_empty()
                                    || supp.error_codes().iter().any(|x| x == kind)
                            }
                            _ => true,
                        }
                })
            })
        {
            return SuppressionEffect::Suppress;
        }
        let mut effect = err.suppression_effect(
            &error_config.enabled_ignores,
            error_config.type_ignore_unknown_tag_behavior,
        );
        if effect == SuppressionEffect::Suppress {
            return effect;
        }
        // Check if the error is inside a multi-line f/t-string. If so, a
        // suppression that covers the f-string's start or end line should also apply.
        let line = err.display_range().start.line_within_file();
        if let Some((fs_start, fs_end)) = find_containing_range(fstring_ranges, line) {
            let ignore = err.module().ignore();
            let enabled = &error_config.enabled_ignores;
            // Check both this kind's name and any parent kind's name.
            for kind in err.error_kind().suppression_names() {
                if fs_start != line {
                    effect = effect.max(ignore.suppression_effect(
                        fs_start,
                        kind,
                        enabled,
                        error_config.type_ignore_unknown_tag_behavior,
                    ));
                }
                if fs_end != line {
                    effect = effect.max(ignore.suppression_effect(
                        fs_end,
                        kind,
                        enabled,
                        error_config.type_ignore_unknown_tag_behavior,
                    ));
                }
            }
        }
        effect
    }

    pub fn collect_into(
        &self,
        error_config: &ErrorConfig,
        fstring_ranges: &[(LineNumber, LineNumber)],
        ignore_all: &[Suppression],
        result: &mut CollectedErrors,
    ) {
        let mut errors = self.errors.lock();
        let errors: Vec<&Error> = errors.iter().collect();
        let mut borrowed = BorrowedCollectedErrors::default();
        Self::collect_sorted(
            &self.module_info,
            &errors,
            error_config,
            fstring_ranges,
            ignore_all,
            &mut borrowed,
        );
        borrowed.into_owned(result);
    }

    /// Classify the finalized `snapshots` of one module together, so that an error in
    /// one can explain an `unknown-from-error` error in another. Each category of
    /// `result` receives the module's diagnostics sorted by range.
    pub(crate) fn collect_snapshots_into<'a>(
        module_info: &ModuleInfo,
        snapshots: &[&'a ModuleDiagnostics],
        error_config: &ErrorConfig,
        fstring_ranges: &[(LineNumber, LineNumber)],
        ignore_all: &[Suppression],
        result: &mut BorrowedCollectedErrors<'a>,
    ) {
        let mut errors: Vec<&'a Error> = snapshots
            .iter()
            .flat_map(|snapshot| snapshot.iter())
            .collect();
        // Each snapshot is sorted by range and the sort is stable, so diagnostics at an
        // equal range keep the order of `snapshots`.
        errors.sort_by_key(|error| (error.range().start(), error.range().end()));
        Self::collect_sorted(
            module_info,
            &errors,
            error_config,
            fstring_ranges,
            ignore_all,
            result,
        );
    }

    /// Classify `errors`, which must be sorted by range.
    fn collect_sorted<'a>(
        module_info: &ModuleInfo,
        errors: &[&'a Error],
        error_config: &ErrorConfig,
        fstring_ranges: &[(LineNumber, LineNumber)],
        ignore_all: &[Suppression],
        result: &mut BorrowedCollectedErrors<'a>,
    ) {
        if module_info.is_generated() && error_config.ignore_errors_in_generated_code {
            return;
        }
        // Only errors that are shown or suppressed explain an `unknown-from-error` error, so an
        // error whose kind is disabled never hides one. A soft error never produces an error
        // `Any`, so only hard errors and nested `unknown-from-error` errors count.
        let shown = |kind| error_config.display_config.severity(kind) != Severity::Ignore;
        let explained = shown(ErrorKind::UnknownFromError).then(|| {
            unknown_from_error_explained(errors, |kind: ErrorKind| {
                shown(kind) && (!kind.is_soft() || kind == ErrorKind::UnknownFromError)
            })
        });
        for (i, &error) in errors.iter().enumerate() {
            if explained.as_ref().is_some_and(|explained| explained[i]) {
                continue;
            }
            let severity = error_config.display_config.severity(error.error_kind());
            let collected = |severity| CollectedDiagnostic::new(error, severity);
            if error.error_kind().is_directive() {
                // Directives bypass suppression, baseline, and
                // min-severity, but still respect explicit severity
                // overrides (e.g. --ignore reveal-type).
                if severity == Severity::Ignore {
                    result.disabled.push(collected(error.severity()));
                } else {
                    result.directives.push(collected(severity));
                }
                continue;
            }
            // A `misplaced-ignore` error is about a suppression comment, so the fix is to
            // move or remove the comment rather than to suppress the error.
            let effect = if error.error_kind() == ErrorKind::MisplacedIgnore {
                SuppressionEffect::None
            } else {
                Self::suppression_effect(error, fstring_ranges, ignore_all, error_config)
            };
            if effect == SuppressionEffect::Suppress {
                result.suppressed.push(collected(severity));
                continue;
            }
            let severity = if effect == SuppressionEffect::DowngradeToWarning {
                severity.min(Severity::Warn)
            } else {
                severity
            };
            match severity {
                Severity::Error | Severity::Warn | Severity::Info => {
                    result.ordinary.push(collected(severity))
                }
                Severity::Hint => result.hints.push(collected(severity)),
                Severity::Ignore => result.disabled.push(collected(error.severity())),
            }
        }
    }

    pub fn collect(&self, error_config: &ErrorConfig) -> CollectedErrors {
        let mut result = CollectedErrors::default();
        self.collect_into(error_config, &[], &[], &mut result);
        result
    }
}

/// Message for the `misplaced-ignore` diagnostic. Kept as a shared constant so
/// the wording stays consistent across every misplaced directive.
const MISPLACED_IGNORE_MESSAGE: &str = "`# pyrefly: ignore-errors` has no effect here — a file-level suppression must appear before any code. \
Move it to the top of the file, or use `# pyrefly: ignore[code]` to suppress a single line.";

/// A builder for constructing and emitting errors incrementally.
/// Chain decoration methods and call `.emit()` to push the error into the collector.
#[must_use = "errors are not emitted until .emit() is called"]
pub struct ErrorBuilder<'a> {
    collector: &'a ErrorCollector,
    active: bool,
    range: TextRange,
    kind: ErrorKind,
    header: String,
    details: Vec<String>,
    context: Option<ErrorContext>,
    annotations: Vec<(TextRange, String)>,
    quick_fixes: Vec<ErrorQuickFix>,
    deprecated_tag: bool,
}

impl ErrorBuilder<'_> {
    /// Append a detail line (shown indented below the header).
    pub fn with_detail(mut self, msg: String) -> Self {
        if self.active {
            self.details.push(msg);
        }
        self
    }

    /// Append a detail line that is only worth working out if the error will be
    /// kept. Modules loaded below `Require::Errors` collect with
    /// [`ErrorStyle::Never`], so for them this never runs at all.
    pub fn with_detail_from(mut self, msg: impl FnOnce() -> Option<String>) -> Self {
        if self.active
            && let Some(msg) = msg()
        {
            self.details.push(msg);
        }
        self
    }

    /// Convenience method to append multiple detail lines.
    pub fn with_details(mut self, details: Vec<String>) -> Self {
        if self.active {
            self.details.extend(details);
        }
        self
    }

    /// Adds the errors from `collector` as detail lines.
    pub fn with_errors_as_details(mut self, collector: ErrorCollector) -> Self {
        for error in collector.errors.into_inner().iter() {
            self.details.push(error.msg_header().to_owned());
        }
        self
    }

    /// Append a diagnostic's details and editor metadata without repeating its header.
    /// Duplicate details, annotations, and fixes are included once.
    pub(crate) fn with_error_details(mut self, error: &Error) -> Self {
        if self.active {
            if let Some(details) = error.msg_details() {
                let details = details
                    .strip_prefix("  ")
                    .expect("error details are indented")
                    .to_owned();
                if !self.details.contains(&details) {
                    self.details.push(details);
                }
            }
            for annotation in error.secondary_annotations() {
                let annotation = (annotation.range, annotation.label.to_string());
                if !self.annotations.contains(&annotation) {
                    self.annotations.push(annotation);
                }
            }
            for fix in error.quick_fixes() {
                if !self.quick_fixes.contains(fix) {
                    self.quick_fixes.push(fix.clone());
                }
            }
        }
        self
    }

    /// Add a secondary labeled span.
    pub fn with_annotation(mut self, range: TextRange, label: String) -> Self {
        if self.active {
            self.annotations.push((range, label));
        }
        self
    }

    /// Report the deprecation without marking the range as deprecated in editors. See
    /// [`Error::without_deprecated_tag`].
    pub fn without_deprecated_tag(mut self) -> Self {
        self.deprecated_tag = false;
        self
    }

    /// Add a structured quick fix.
    pub fn with_quick_fix(mut self, fix: ErrorQuickFix) -> Self {
        if self.active {
            self.quick_fixes.push(fix);
        }
        self
    }

    /// Set the ErrorContext. At emit time, the context's message becomes the header
    /// (demoting the original header to first detail), its annotations are prepended,
    /// and the ErrorKind is overridden. If called more than once, the last context wins.
    /// `with_context(None)` clears the context.
    pub fn with_context(mut self, ctx: Option<impl FnOnce() -> ErrorContext>) -> Self {
        if self.active {
            self.context = ctx.map(|ctx| ctx());
        }
        self
    }

    /// Emit the error into the collector.
    pub fn emit(self) {
        if !self.active {
            return;
        }
        let (mut kind, mut header, mut details, mut annotations) =
            (self.kind, self.header, self.details, self.annotations);
        if let Some(ctx) = self.context {
            kind = ctx.as_error_kind();
            details.insert(0, header);
            header = ctx.format();
            let mut ctx_annotations = ctx.annotations();
            ctx_annotations.extend(annotations);
            annotations = ctx_annotations;
        }
        let mut err = Error::new(
            self.collector.module_info.dupe(),
            self.range,
            header,
            details,
            kind,
        );
        for (range, label) in annotations {
            err = err.with_annotation(range, label);
        }
        for fix in self.quick_fixes {
            err = err.with_quick_fix(fix);
        }
        if !self.deprecated_tag {
            err = err.without_deprecated_tag();
        }
        self.collector.errors.lock().push(err);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use pyrefly_python::ignore::Tool;
    use pyrefly_python::ignore::TypeIgnoreUnknownTagBehavior;
    use pyrefly_python::module_name::ModuleName;
    use pyrefly_python::module_path::ModulePath;
    use pyrefly_util::prelude::SliceExt;
    use ruff_python_ast::name::Name;
    use ruff_text_size::TextSize;

    use super::*;
    use crate::config::error::ErrorDisplayConfig;
    use crate::config::error_kind::ErrorKind;
    use crate::config::error_kind::Severity;

    fn add(errors: &ErrorCollector, range: TextRange, kind: ErrorKind, msg: String) {
        errors.error_builder(range, kind, msg).emit();
    }

    #[test]
    fn test_error_builder_merges_details_and_metadata() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new("contents".to_owned()),
        );
        let range = TextRange::new(TextSize::new(1), TextSize::new(2));
        let error = Error::new(
            mi.dupe(),
            range,
            "Protocol failure".to_owned(),
            vec![
                "Enter failed\n    Explanation".to_owned(),
                "Exit failed".to_owned(),
            ],
            ErrorKind::BadContextManager,
        )
        .with_annotation(range, "Enter".to_owned())
        .with_quick_fix(ErrorQuickFix::AssertNotNone);
        let merged = ErrorCollector::new(mi, ErrorStyle::Delayed);
        merged
            .error_builder(
                range,
                ErrorKind::BadContextManager,
                "Protocol failure".to_owned(),
            )
            .with_error_details(&error)
            .with_error_details(&error)
            .emit();
        let merged = merged.finish();
        assert_eq!(merged.iter().collect::<Vec<_>>(), vec![&error]);
    }

    #[test]
    fn test_error_collector() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new("contents".to_owned()),
        );
        let errors = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::InternalError,
            "b".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::InternalError,
            "a".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::InternalError,
            "a".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(2), TextSize::new(3)),
            ErrorKind::InternalError,
            "a".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::InternalError,
            "b".to_owned(),
        );
        assert_eq!(
            errors
                .collect(&ErrorConfig::new(
                    Arc::new(ErrorDisplayConfig::default()),
                    false,
                    Tool::default_enabled(),
                    TypeIgnoreUnknownTagBehavior::NoEffect,
                ))
                .ordinary
                .map(|x| x.msg()),
            vec!["b", "a", "a"]
        );
    }

    #[test]
    fn test_error_collector_extend_if() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new("contents".to_owned()),
        );
        let errors = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);

        let rejected = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);
        add(
            &rejected,
            TextRange::new(TextSize::new(1), TextSize::new(2)),
            ErrorKind::InternalError,
            "rejected".to_owned(),
        );
        let Err(((), rejected)) = errors.extend_if(rejected, || Err::<(), _>(())) else {
            panic!("a failed commit should return its errors");
        };
        assert_eq!(errors.len(), 0);
        assert_eq!(rejected.len(), 1, "the returned errors should be intact");

        let accepted = ErrorCollector::new(mi, ErrorStyle::Delayed);
        add(
            &accepted,
            TextRange::new(TextSize::new(2), TextSize::new(3)),
            ErrorKind::InternalError,
            "accepted".to_owned(),
        );
        assert!(errors.extend_if(accepted, || Ok::<_, ()>(())).is_ok());
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_error_collector_take() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new("contents".to_owned()),
        );
        let errors = ErrorCollector::new(mi, ErrorStyle::Delayed);
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(2)),
            ErrorKind::InternalError,
            "before".to_owned(),
        );

        let taken = errors.take();
        assert_eq!(taken.iter().map(Error::msg).collect::<Vec<_>>(), ["before"]);

        add(
            &errors,
            TextRange::new(TextSize::new(2), TextSize::new(3)),
            ErrorKind::InternalError,
            "after".to_owned(),
        );
        assert_eq!(
            errors.take().iter().map(Error::msg).collect::<Vec<_>>(),
            ["after"]
        );
    }

    #[test]
    fn test_error_collector_with_disabled_errors() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new("contents".to_owned()),
        );
        let errors = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::InternalError,
            "a".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::NotAsync,
            "b".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::BadAssignment,
            "c".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(2), TextSize::new(3)),
            ErrorKind::BadMatch,
            "d".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::NotIterable,
            "e".to_owned(),
        );

        let display_config = ErrorDisplayConfig::new(HashMap::from([
            (ErrorKind::NotAsync, Severity::Error),
            (ErrorKind::BadAssignment, Severity::Ignore),
            (ErrorKind::NotIterable, Severity::Ignore),
        ]));
        let config = ErrorConfig::new(
            Arc::new(display_config),
            false,
            Tool::default_enabled(),
            TypeIgnoreUnknownTagBehavior::NoEffect,
        );

        assert_eq!(
            errors.collect(&config).ordinary.map(|x| x.msg()),
            vec!["a", "b", "d"]
        );
    }

    #[test]
    fn test_error_collector_keeps_hints_apart() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new("contents".to_owned()),
        );
        let errors = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::BadAssignment,
            "a".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(2), TextSize::new(3)),
            ErrorKind::BadMatch,
            "b".to_owned(),
        );

        let display_config =
            ErrorDisplayConfig::new(HashMap::from([(ErrorKind::BadAssignment, Severity::Hint)]));
        let config = ErrorConfig::new(
            Arc::new(display_config),
            false,
            Tool::default_enabled(),
            TypeIgnoreUnknownTagBehavior::NoEffect,
        );

        let collected = errors.collect(&config);
        assert_eq!(collected.ordinary.map(|x| x.msg()), vec!["b"]);
        assert_eq!(collected.hints.map(|x| x.msg()), vec!["a"]);
    }

    #[test]
    fn test_error_collector_generated_code() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(Path::new("main.py").to_owned()),
            Arc::new(format!("# {}{}\ncontents", "@", "generated")),
        );
        let errors = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::InternalError,
            "a".to_owned(),
        );

        let display_config = ErrorDisplayConfig::default();
        let config0 = ErrorConfig::new(
            Arc::new(display_config.clone()),
            false,
            Tool::default_enabled(),
            TypeIgnoreUnknownTagBehavior::NoEffect,
        );
        assert_eq!(
            errors.collect(&config0).ordinary.map(|x| x.msg()),
            vec!["a"]
        );

        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(3)),
            ErrorKind::MisplacedIgnore,
            "misplaced".to_owned(),
        );
        let config1 = ErrorConfig::new(
            Arc::new(display_config),
            true,
            Tool::default_enabled(),
            TypeIgnoreUnknownTagBehavior::NoEffect,
        );
        assert!(errors.collect(&config1).ordinary.is_empty());
    }

    #[test]
    fn test_errors_not_sorted() {
        let mi = ModuleInfo::new(
            ModuleName::from_name(&Name::new_static("main")),
            ModulePath::filesystem(PathBuf::from("main.py")),
            Arc::new("test".to_owned()),
        );
        let errors = ErrorCollector::new(mi.dupe(), ErrorStyle::Delayed);
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(1)),
            ErrorKind::InternalError,
            "Overload".to_owned(),
        );
        add(
            &errors,
            TextRange::new(TextSize::new(1), TextSize::new(1)),
            ErrorKind::InternalError,
            "A specific error".to_owned(),
        );
        assert_eq!(
            errors
                .collect(&ErrorConfig::new(
                    Arc::new(ErrorDisplayConfig::default()),
                    false,
                    Tool::default_enabled(),
                    TypeIgnoreUnknownTagBehavior::NoEffect,
                ))
                .ordinary
                .map(|x| x.msg()),
            vec!["Overload", "A specific error"]
        );
    }
}
