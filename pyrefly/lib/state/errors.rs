/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use anstream::eprintln;
use anyhow::Context as _;
use dupe::Dupe;
use pyrefly_config::error_kind::ErrorKind;
use pyrefly_config::error_kind::Severity;
use pyrefly_python::ignore::Ignore;
use pyrefly_python::ignore::Suppression;
use pyrefly_python::ignore::SuppressionEffect;
use pyrefly_python::ignore::Tool;
use pyrefly_python::ignore::TypeIgnoreUnknownTagBehavior;
use pyrefly_python::ignore::misplaced_ignore_errors;
use pyrefly_python::ignore::parse_ignore_all;
use pyrefly_python::ignore::physical_lines;
use pyrefly_python::module::Module;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::lined_buffer::LineNumber;
use pyrefly_util::thread_pool::in_thread_pool;
use pyrefly_util::visit::Visit;
use rayon::prelude::*;
use ruff_python_ast::Expr;
use ruff_python_ast::ModModule;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_expr;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::config::config::BaselineMatchingMode;
use crate::config::config::ConfigFile;
use crate::error::baseline::BaselineProcessor;
use crate::error::baseline::StaleRowScope;
use crate::error::baseline::TrackedBaselineProcessor;
use crate::error::baseline::normalize_baseline_path;
use crate::error::collector::BorrowedCollectedErrors;
use crate::error::collector::CollectedDiagnostic;
use crate::error::collector::CollectedErrors;
use crate::error::collector::ErrorCollector;
use crate::error::collector::ModuleDiagnostics;
use crate::error::error::BaselineStatus;
use crate::error::error::Error;
use crate::error::error::ReportedError;
use crate::error::expectation::Expectation;
use crate::error::legacy::BaselineErrors;
use crate::error::style::ErrorStyle;
use crate::state::load::Load;

/// Extracts `(start_line, end_line)` ranges for all multi-line strings from
/// the AST, including plain strings, byte strings, f-strings, and t-strings.
/// Single-line strings (where start == end) are excluded. The returned list
/// is sorted by start_line.
pub fn sorted_multi_line_string_ranges(
    ast: &ModModule,
    module: &Module,
) -> Vec<(LineNumber, LineNumber)> {
    let mut ranges = Vec::new();
    ast.visit(&mut |expr: &Expr| {
        collect_string_ranges(expr, module, &mut ranges);
    });
    ranges.sort();
    ranges
}

/// Recursively collects multi-line string ranges from an expression and all
/// of its sub-expressions. The `Visit<Expr> for Stmt` implementation only
/// visits top-level expressions in statements, so nested strings (e.g., an
/// f-string inside a function call's arguments) would be missed without
/// explicit recursion here.
fn collect_string_ranges(expr: &Expr, module: &Module, ranges: &mut Vec<(LineNumber, LineNumber)>) {
    let text_range = match expr {
        Expr::FString(x) => Some(x.range),
        Expr::TString(x) => Some(x.range),
        Expr::StringLiteral(x) => Some(x.range),
        Expr::BytesLiteral(x) => Some(x.range),
        _ => None,
    };
    if let Some(range) = text_range {
        // Computing columns scans from the start of the line for non-ASCII source, but this pass
        // only needs line numbers.
        let line_index = module.lined_buffer().line_index();
        let start = LineNumber::from_one_indexed(line_index.line_index(range.start()));
        let end = LineNumber::from_one_indexed(line_index.line_index(range.end()));
        if start != end {
            // Multi-line string found. Record its range but skip recursing
            // into its children — for nested f-strings we want errors to
            // remap to the outermost string's start line, and the outer
            // range already covers the inner one.
            ranges.push((start, end));
            return;
        }
    }
    expr.recurse(&mut |child: &Expr| {
        collect_string_ranges(child, module, ranges);
    });
}

/// Finds contiguous backslash continuation blocks in the source lines.
/// A block starts at the first line ending with `\` and ends at the first
/// subsequent line that does NOT end with `\` (inclusive — that line is the
/// last line of the continued expression). Returns sorted, non-overlapping
/// `(start, end)` ranges using 1-indexed `LineNumber`.
///
/// Comments are stripped before checking for trailing backslashes, so
/// `x = 1  # comment \` is not treated as a continuation. Lines inside
/// multiline strings are also excluded, since `\` at end of line inside a
/// triple-quoted string is string content, not a line continuation.
pub fn sorted_backslash_continuation_ranges(
    lines: &[&str],
    multiline_string_ranges: &[(LineNumber, LineNumber)],
    ignore: &Ignore,
) -> Vec<(LineNumber, LineNumber)> {
    fn is_continuation(line: &str, line_number: LineNumber, ignore: &Ignore) -> bool {
        let code = match ignore.comment_start(line_number) {
            Some(pos) => &line[..pos],
            None => line,
        };
        code.trim_end().ends_with('\\')
    }

    let mut ranges = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line_num = LineNumber::from_zero_indexed(i as u32);
        if find_containing_range(multiline_string_ranges, line_num).is_some() {
            i += 1;
        } else if is_continuation(lines[i], line_num, ignore) {
            let start = i;
            while i < lines.len()
                && is_continuation(lines[i], LineNumber::from_zero_indexed(i as u32), ignore)
                && find_containing_range(
                    multiline_string_ranges,
                    LineNumber::from_zero_indexed(i as u32),
                )
                .is_none()
            {
                i += 1;
            }
            // Include the first line that does not end with a backslash: it is
            // the final line of the continued expression.
            let end = if i < lines.len() { i } else { i - 1 };
            ranges.push((
                LineNumber::from_zero_indexed(start as u32),
                LineNumber::from_zero_indexed(end as u32),
            ));
            i += 1;
        } else {
            i += 1;
        }
    }
    ranges
}

/// Binary search over sorted f-string ranges to find the range containing `line`.
pub fn find_containing_range(
    ranges: &[(LineNumber, LineNumber)],
    line: LineNumber,
) -> Option<(LineNumber, LineNumber)> {
    let idx = ranges.partition_point(|(start, _)| *start <= line);
    if idx == 0 {
        return None;
    }
    let (start, end) = ranges[idx - 1];
    if line >= start && line <= end {
        Some((start, end))
    } else {
        None
    }
}

/// Extracts `(start_line, end_line)` line ranges for multi-line expressions
/// where a suppression comment placed on the line *above* an interior line
/// would be relocated by a formatter (e.g. Black) out of the bracketed group,
/// so the checker would no longer associate it with the error. These are the
/// conditional and operator expressions — ternaries (`a if c else b`), boolean
/// operators (`a and b`), binary operators (`a + b`), and comparisons
/// (`a < b`) — split across physical lines via bracketed continuation. Errors
/// on a non-first line of such a range must be suppressed inline instead.
///
/// Collection literals (dicts, lists, sets, tuples, comprehensions) are
/// intentionally *not* recorded: each element sits on its own line, so a
/// comment on the line above an element is stable under formatting and should
/// be kept. We still recurse into them, so an operator expression used *as* an
/// element (e.g. a multi-line ternary dict value) is still captured.
///
/// The returned list is sorted by start line and non-overlapping: nested
/// expressions (e.g. a binary operator inside a ternary) are merged into a
/// single enclosing range, preserving the binary-search invariant of
/// `find_containing_range`.
pub fn sorted_bracketed_continuation_ranges(
    ast: &ModModule,
    module: &Module,
) -> Vec<(LineNumber, LineNumber)> {
    struct Collector<'a> {
        module: &'a Module,
        ranges: Vec<(LineNumber, LineNumber)>,
    }
    impl<'a> Visitor<'a> for Collector<'a> {
        fn visit_expr(&mut self, expr: &'a Expr) {
            // Conditional/operator expressions float an interior own-line
            // comment when split across lines; collection literals do not.
            if matches!(
                expr,
                Expr::If(_) | Expr::BoolOp(_) | Expr::BinOp(_) | Expr::Compare(_)
            ) {
                let display = self.module.display_range(expr.range());
                let start = display.start.line_within_file();
                let end = display.end.line_within_file();
                if start != end {
                    self.ranges.push((start, end));
                }
            }
            walk_expr(self, expr);
        }
    }
    let mut collector = Collector {
        module,
        ranges: Vec::new(),
    };
    collector.visit_body(&ast.body);
    collector.ranges.sort();

    // Merge overlapping/nested ranges so the result stays non-overlapping and
    // sorted, as `find_containing_range` requires.
    let mut merged: Vec<(LineNumber, LineNumber)> = Vec::new();
    for (start, end) in collector.ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => {
                if end > last.1 {
                    last.1 = end;
                }
            }
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Per-module multi-line ranges and ignore-all directives, computed after parsing.
#[derive(Debug, Clone)]
pub struct ModuleRanges {
    /// Multi-line string and backslash-continuation ranges.
    pub multi_line: Vec<(LineNumber, LineNumber)>,
    /// Top-level ignore-all directives (e.g. `# pyrefly: ignore-errors`).
    pub ignore_all: Vec<Suppression>,
    /// Lines of pyrefly `ignore-errors` directives placed after the preamble,
    /// where they are inert. Surfaced as `misplaced-ignore` warnings.
    pub misplaced_ignore_all: Vec<LineNumber>,
}

impl ModuleRanges {
    /// Compute multi-line ranges and ignore-all directives from the AST and module source.
    pub fn compute(ast: &ModModule, module_info: &Module) -> Self {
        let mut multi_line = sorted_multi_line_string_ranges(ast, module_info);
        let lines = physical_lines(module_info.contents());
        multi_line.extend(sorted_backslash_continuation_ranges(
            &lines,
            &multi_line,
            module_info.ignore(),
        ));
        multi_line.sort();
        let ignore_all = parse_ignore_all(module_info.contents(), &multi_line);
        let misplaced_ignore_all = misplaced_ignore_errors(module_info.contents(), &multi_line);
        Self {
            multi_line,
            ignore_all,
            misplaced_ignore_all,
        }
    }
}

/// A module's load, its parse, binding and solve diagnostics, its ranges, and its config.
type LoadErrors = (
    Arc<Load>,
    Option<ModuleDiagnostics>,
    Option<ModuleDiagnostics>,
    Option<ModuleDiagnostics>,
    Option<Arc<ModuleRanges>>,
    ArcId<ConfigFile>,
);

/// Below this many modules, `Errors` collates on the calling thread, where the work is
/// too small to pay for moving it to a thread pool.
const PARALLEL_COLLATION_MIN_MODULES: usize = 64;

/// The errors from a collection of modules.
#[derive(Debug)]
pub struct Errors {
    // Sorted by module path so concatenating per-module diagnostics is deterministic.
    loads: Vec<LoadErrors>,
}

/// Outcome of applying a baseline file.
#[derive(Debug)]
pub enum BaselineApplyResult {
    /// No baseline path was configured.
    NotConfigured,
    /// A path was configured but no file exists on disk. Tolerated as
    /// `NotCompared` in ordinary `check`; `--prune`/`--error-stale-baseline`
    /// require an existing file elsewhere.
    NotFound,
    /// File exists but could not be read or parsed. Hard error unless
    /// `--update-baseline` tolerates it.
    FailedToRead(anyhow::Error),
    /// File was loaded and used to split `ordinary` vs `baseline`.
    /// When `stale_rows` is `None`, `unused=0` and `retained=[]`.
    Applied {
        unused_entry_count: usize,
        retained: BaselineErrors,
        /// The file's contents as they were read, which `retained` was derived from. A
        /// caller rewriting the baseline passes this back so the write can be abandoned
        /// if someone else replaced the file in the meantime.
        source: String,
    },
}

impl BaselineApplyResult {
    /// Resolve into the data needed by `check`, handling the
    /// `--update-baseline` toleration for unreadable baselines.
    /// Returns the `BaselineStatus` to assign to ordinary errors, plus
    /// pruning data (0/empty when not applicable).
    /// `tolerate_read_error` should be `true` when `--update-baseline` is set.
    pub fn resolve(
        self,
        tolerate_read_error: bool,
    ) -> anyhow::Result<(BaselineStatus, usize, BaselineErrors)> {
        match self {
            Self::NotConfigured => {
                Ok((BaselineStatus::NotConfigured, 0, BaselineErrors::default()))
            }
            Self::NotFound => Ok((BaselineStatus::NotCompared, 0, BaselineErrors::default())),
            Self::Applied {
                unused_entry_count,
                retained,
                source: _,
            } => Ok((BaselineStatus::Unmatched, unused_entry_count, retained)),
            Self::FailedToRead(e) if tolerate_read_error => {
                // When regenerating the baseline, a corrupt/unreadable existing
                // file is tolerated and treated as missing.
                eprintln!(
                    "Ignoring unreadable baseline while regenerating it with `--update-baseline`: {e:#}"
                );
                Ok((BaselineStatus::NotCompared, 0, BaselineErrors::default()))
            }
            Self::FailedToRead(e) => Err(e),
        }
    }
}

impl Errors {
    pub fn new(mut loads: Vec<LoadErrors>) -> Self {
        loads.sort_by_key(|x| x.0.module_info.path().dupe());
        Self { loads }
    }

    pub(crate) fn merge_display_errors<E: ReportedError>(
        mut ordinary: Vec<E>,
        directives: Vec<E>,
    ) -> Vec<E> {
        ordinary.extend(directives);
        ordinary.sort_by_cached_key(|e| {
            let e = e.error();
            (
                e.module().name(),
                e.path().dupe(),
                e.range().start(),
                e.range().end(),
            )
        });
        ordinary
    }

    /// Apply `f` to each module, in `loads` order. Every diagnostic is stored on the
    /// module that contains its primary location, so each module's diagnostics are
    /// collated on their own, and the results never need deduplicating across modules.
    /// That lets large collections run in parallel, on the thread pool of the calling thread,
    /// such as the state's pool once a CLI check has finished with it. Called from outside a
    /// pool, as the language server does because a recheck may be occupying the state's pool,
    /// the work stays on the calling thread.
    fn map_modules<'a, R: Send>(&'a self, f: impl Fn(&'a LoadErrors) -> R + Sync + Send) -> Vec<R> {
        if self.loads.len() < PARALLEL_COLLATION_MIN_MODULES || !in_thread_pool() {
            self.loads.iter().map(f).collect()
        } else {
            self.loads.par_iter().map(f).collect()
        }
    }

    pub fn collect_errors(&self) -> CollectedErrors {
        // Copy each module's diagnostics on the thread that collated them.
        let modules = self.map_modules(|module| {
            let mut owned = CollectedErrors::default();
            Self::collect_module_error_refs(module)?.into_owned(&mut owned);
            Some(owned)
        });
        CollectedErrors::concat(modules.into_iter().flatten().collect())
    }

    /// Collate every module's diagnostics, borrowing them, along with the unused-suppression
    /// diagnostics, which each module works out from its own diagnostics as it collates them.
    pub(crate) fn collect_error_refs_with_unused_ignores(
        &self,
    ) -> (BorrowedCollectedErrors<'_>, Vec<Error>) {
        let (errors, unused): (Vec<_>, Vec<_>) = self
            .map_modules(|module| {
                let (load, _, _, _, module_ranges, config) = module;
                let module_errors = Self::collect_module_error_refs(module).unwrap_or_default();
                let path = load.module_info.path();
                let module_unused = Self::collect_unused_ignore_errors_for_module(
                    &load.module_info,
                    module_ranges.as_deref(),
                    &config.enabled_ignores(path),
                    config.type_ignore_unknown_tag_behavior(path),
                    module_errors
                        .suppressed
                        .iter()
                        .chain(&module_errors.ordinary)
                        .chain(&module_errors.hints)
                        .map(CollectedDiagnostic::error),
                );
                (module_errors, module_unused)
            })
            .into_iter()
            .unzip();
        (
            BorrowedCollectedErrors::concat(errors),
            unused.into_iter().flatten().collect(),
        )
    }

    /// Collate one module's diagnostics, or `None` if the module does not report any.
    /// Each category of the result is sorted by range.
    fn collect_module_error_refs(
        (load, parse_errors, binding_errors, solve_errors, module_ranges, config): &LoadErrors,
    ) -> Option<BorrowedCollectedErrors<'_>> {
        if load.error_style == ErrorStyle::Never {
            return None;
        }
        let error_config = config.get_error_config(load.module_info.path());
        let ranges = module_ranges
            .as_ref()
            .expect("module_ranges must be present when error style is not Never");
        let parse_errors = parse_errors
            .as_ref()
            .expect("a module with module_ranges must have parse diagnostics");
        // Collect in the order the phases run, so diagnostics at an equal range
        // stay in phase order.
        let snapshots: Vec<&ModuleDiagnostics> = [&load.load_errors, parse_errors]
            .into_iter()
            .chain(binding_errors)
            .chain(solve_errors)
            .collect();
        let mut errors = BorrowedCollectedErrors::default();
        ErrorCollector::collect_snapshots_into(
            &load.module_info,
            &snapshots,
            &error_config,
            &ranges.multi_line,
            &ranges.ignore_all,
            &mut errors,
        );
        Some(errors)
    }

    /// Apply baseline filtering to already-collected errors in place.
    /// `relative_to` resolves relative paths stored in the baseline file.
    /// A missing file is mapped to `NotFound` (benign), not `FailedToRead`,
    /// so there is no TOCTOU race between `exists()` and reading the file.
    pub fn apply_baseline<E: ReportedError>(
        &self,
        ordinary: &mut Vec<E>,
        baseline: &mut Vec<E>,
        baseline_path: Option<&Path>,
        relative_to: &Path,
        matching_mode: BaselineMatchingMode,
        stale_rows: Option<StaleRowScope>,
    ) -> BaselineApplyResult {
        let Some(baseline_path) = baseline_path else {
            return BaselineApplyResult::NotConfigured;
        };

        let fail_ctx = || format!("failed to read baseline file `{}`", baseline_path.display());

        // Read via `std::fs` rather than `fs_anyhow` so that a missing baseline is
        // identified by the `ErrorKind` of this exact call, rather than by searching
        // an `anyhow` chain where an unrelated `NotFound` could be mistaken for it.
        let content = match fs::read_to_string(baseline_path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return BaselineApplyResult::NotFound;
            }
            Err(e) => {
                return BaselineApplyResult::FailedToRead(
                    anyhow::Error::new(e).context(fail_ctx()),
                );
            }
        };

        if let Some(scope) = stale_rows {
            let processor =
                match TrackedBaselineProcessor::from_json(&content, relative_to, matching_mode)
                    .with_context(fail_ctx)
                {
                    Ok(p) => p,
                    Err(e) => return BaselineApplyResult::FailedToRead(e),
                };
            let checked_paths: HashSet<_> = self
                .loads
                .iter()
                .filter(|(load, _, _, _, _, _)| load.error_style != ErrorStyle::Never)
                .map(|(load, _, _, _, _, _)| {
                    normalize_baseline_path(load.module_info.path().as_path(), relative_to)
                })
                .collect();
            let result = processor.process_errors(ordinary, baseline, &checked_paths, scope);
            BaselineApplyResult::Applied {
                unused_entry_count: result.unused_entry_count,
                retained: result.retained,
                source: content,
            }
        } else {
            let processor = match BaselineProcessor::from_json(&content, relative_to, matching_mode)
                .with_context(fail_ctx)
            {
                Ok(p) => p,
                Err(e) => return BaselineApplyResult::FailedToRead(e),
            };
            processor.process_errors(ordinary, baseline);
            BaselineApplyResult::Applied {
                unused_entry_count: 0,
                retained: BaselineErrors::default(),
                source: content,
            }
        }
    }

    /// Collect display errors for the language server, partitioned by whether or not they
    /// appear in a baseline file. Returns `(normal, baselined)`.
    ///
    /// Paths recorded in a baseline resolve relative to its config's source root, falling
    /// back to the baseline file's own directory.
    pub fn collect_lsp_errors_with_baselines(&self) -> (Vec<Error>, Vec<Error>) {
        let mut unused = Vec::new();
        let (normal, baselined) = self.collect_lsp_error_refs_with_baselines(&mut unused);
        let owned = |errors: Vec<CollectedDiagnostic<'_>>| {
            errors
                .into_iter()
                .map(CollectedDiagnostic::into_error)
                .collect()
        };
        (owned(normal), owned(baselined))
    }

    /// Like `collect_lsp_errors_with_baselines`, but borrowing every diagnostic. The
    /// unused-suppression diagnostics, which collection creates rather than finds, are
    /// stored in `unused`.
    pub(crate) fn collect_lsp_error_refs_with_baselines<'a>(
        &'a self,
        unused: &'a mut Vec<Error>,
    ) -> (Vec<CollectedDiagnostic<'a>>, Vec<CollectedDiagnostic<'a>>) {
        let (collected, unused_ignores) = self.collect_error_refs_with_unused_ignores();
        *unused = unused_ignores;
        let unused: &'a [Error] = unused;
        let config_by_path = self.config_by_path();
        let mut normal = collected.directives;
        let mut baselined = Vec::new();

        // Group by config before matching, so that each baseline is parsed once and sees
        // every diagnostic it governs in a single call.
        let mut by_config: SmallMap<ArcId<ConfigFile>, (&Path, Vec<CollectedDiagnostic>)> =
            SmallMap::new();
        for error in collected
            .ordinary
            .into_iter()
            .chain(collected.hints)
            .chain(self.shown_unused_ignore_errors(unused))
        {
            match config_by_path.get(&error.error().path()) {
                Some(config) if let Some(baseline) = &config.baseline => by_config
                    .entry((*config).dupe())
                    .or_insert_with(|| (baseline, Vec::new()))
                    .1
                    .push(error),
                // The module has no config, or its config has no baseline.
                _ => normal.push(error),
            }
        }

        for (config, (baseline_path, mut group)) in by_config {
            let relative_to = config
                .source
                .root_from_file()
                .or_else(|| baseline_path.parent())
                .unwrap_or_else(|| Path::new(""));
            let processor = fs::read_to_string(baseline_path).ok().and_then(|content| {
                BaselineProcessor::from_json(&content, relative_to, config.baseline_matching_mode)
                    .ok()
            });
            // An unreadable or invalid baseline suppresses nothing; the language server
            // surfaces every diagnostic rather than failing the request.
            if let Some(processor) = processor {
                let start = baselined.len();
                processor.process_errors(&mut group, &mut baselined);
                // `baseline-error-level` caps a match here, where this config is already
                // in hand. An unset level leaves the severity alone and asks the IDE to
                // draw the open-file match as a hint.
                for error in &mut baselined[start..] {
                    *error = match config.baseline_error_level {
                        Some(level) => error.with_severity(error.severity().min(level)),
                        None => error.with_unconfigured_baseline_hint(),
                    };
                }
            }
            normal.extend(group);
        }

        (
            Self::merge_display_errors(normal, Vec::new()),
            Self::merge_display_errors(baselined, Vec::new()),
        )
    }

    fn config_by_path(&self) -> SmallMap<&ModulePath, &ArcId<ConfigFile>> {
        self.loads
            .iter()
            .map(|(load, _, _, _, _, config)| (load.module_info.path(), config))
            .collect()
    }

    /// The diagnostics in `unused` that their module's config shows, at the severity it
    /// gives them.
    pub(crate) fn shown_unused_ignore_errors<'a>(
        &self,
        unused: &'a [Error],
    ) -> Vec<CollectedDiagnostic<'a>> {
        if unused.is_empty() {
            return Vec::new();
        }
        let config_by_path = self.config_by_path();
        let self_suppressed = self.self_suppressed_unused_ignore_ranges();
        unused
            .iter()
            .filter_map(|error| {
                if error.error_kind() == ErrorKind::UnusedIgnore
                    && self_suppressed.contains(&(error.path(), error.range()))
                {
                    return None;
                }
                let config = config_by_path.get(&error.path())?;
                let severity = config
                    .get_error_config(error.path())
                    .display_config
                    .severity(error.error_kind());
                (severity != Severity::Ignore).then(|| CollectedDiagnostic::new(error, severity))
            })
            .collect()
    }

    /// The exact comment ranges that suppress their own unused-ignore diagnostics.
    fn self_suppressed_unused_ignore_ranges(&self) -> SmallSet<(&ModulePath, TextRange)> {
        // Match exact comment ranges so one directive cannot hide another directive's diagnostic.
        self.loads
            .iter()
            .flat_map(|(load, ..)| {
                let module = &load.module_info;
                module
                    .ignore()
                    .iter()
                    .flat_map(|(_, suppressions)| suppressions)
                    .filter(|supp| {
                        supp.tool() == Tool::Pyrefly
                            && supp
                                .error_codes()
                                .iter()
                                .any(|code| code == ErrorKind::UnusedIgnore.to_name())
                    })
                    .map(|supp| {
                        (
                            module.path(),
                            supp.removal_edit(module.lined_buffer(), None).range,
                        )
                    })
            })
            .collect()
    }

    pub fn collect_display_errors(&self) -> Vec<Error> {
        let errors = self.collect_errors();
        Self::merge_display_errors(errors.ordinary, errors.directives)
    }

    pub fn collect_display_errors_with_unused_ignores(&self) -> Vec<Error> {
        let collected = self.collect_errors();
        let unused = self.collect_unused_ignore_errors_for_display(&collected);
        let mut ordinary = collected.ordinary;
        ordinary.extend(unused.ordinary);
        Self::merge_display_errors(ordinary, collected.directives)
    }

    pub fn collect_ignores(&self) -> SmallMap<&ModulePath, &Ignore> {
        let mut ignore_collection: SmallMap<&ModulePath, &Ignore> = SmallMap::new();
        for (load, _, _, _, _, _) in &self.loads {
            let module_path = load.module_info.path();
            let ignores = load.module_info.ignore();
            ignore_collection.insert(module_path, ignores);
        }
        ignore_collection
    }

    /// Collects errors for unused ignore comments.
    /// Returns a vector of errors with ErrorKind::UnusedIgnore for each
    /// suppression comment that doesn't suppress any actual error.
    /// Accepts pre-collected errors to avoid redundant error collection.
    pub fn collect_unused_ignore_errors(&self, collected: &CollectedErrors) -> Vec<Error> {
        self.collect_unused_ignore_errors_from(
            collected
                .suppressed
                .iter()
                .chain(&collected.ordinary)
                .chain(&collected.hints),
        )
    }

    fn collect_unused_ignore_errors_from<'a>(
        &self,
        affected: impl Iterator<Item = &'a Error>,
    ) -> Vec<Error> {
        let mut affected_by_suppression_by_module: SmallMap<&ModulePath, Vec<&Error>> =
            SmallMap::new();
        for error in affected {
            affected_by_suppression_by_module
                .entry(error.path())
                .or_default()
                .push(error);
        }
        self.map_modules(|(load, _, _, _, module_ranges, config)| {
            let path = load.module_info.path();
            let affected = affected_by_suppression_by_module
                .get(path)
                .into_iter()
                .flat_map(|errors| errors.iter().copied());
            Self::collect_unused_ignore_errors_for_module(
                &load.module_info,
                module_ranges.as_deref(),
                &config.enabled_ignores(path),
                config.type_ignore_unknown_tag_behavior(path),
                affected,
            )
        })
        .into_iter()
        .flatten()
        .collect()
    }

    /// Collects errors for the unused ignore comments of one module, given the module's
    /// diagnostics that a suppression affects.
    pub(crate) fn collect_unused_ignore_errors_for_module<'a>(
        module: &Module,
        ranges: Option<&ModuleRanges>,
        enabled_ignores: &SmallSet<Tool>,
        type_ignore_unknown_tag_behavior: TypeIgnoreUnknownTagBehavior,
        affected: impl Iterator<Item = &'a Error>,
    ) -> Vec<Error> {
        let multi_line = ranges.map_or(&[][..], |ranges| ranges.multi_line.as_slice());
        // Map from line number to the set of error codes suppressed on it.
        let mut suppressed_codes: SmallMap<LineNumber, SmallSet<String>> = SmallMap::new();
        for error in affected {
            assert_eq!(
                error.path(),
                module.path(),
                "a module can only classify its own suppressed diagnostics"
            );
            let start_line = error.display_range().start.line_within_file();
            let end_line = error.display_range().end.line_within_file();
            let containing_range = find_containing_range(multi_line, start_line);
            let is_affected = error
                .suppression_effect(enabled_ignores, type_ignore_unknown_tag_behavior)
                != SuppressionEffect::None
                || containing_range.is_some_and(|(fs_start, fs_end)| {
                    let ignore = error.module().ignore();
                    error.error_kind().suppression_names().any(|kind| {
                        (fs_start != start_line
                            && ignore.suppression_effect(
                                fs_start,
                                kind,
                                enabled_ignores,
                                type_ignore_unknown_tag_behavior,
                            ) != SuppressionEffect::None)
                            || (fs_end != start_line
                                && ignore.suppression_effect(
                                    fs_end,
                                    kind,
                                    enabled_ignores,
                                    type_ignore_unknown_tag_behavior,
                                ) != SuppressionEffect::None)
                    })
                });
            if is_affected {
                // Track both this kind's name and any parent kind's name, so that
                // e.g. `# pyrefly: ignore[bad-override]` is not reported as unused
                // when it suppresses a `bad-override-mutable-attribute` error.
                let error_codes: Vec<String> = error
                    .error_kind()
                    .suppression_names()
                    .map(|s| s.to_owned())
                    .collect();

                // Track the error codes for all lines the error spans.
                for line_idx in start_line.to_zero_indexed()..=end_line.to_zero_indexed() {
                    let line_codes = suppressed_codes
                        .entry(LineNumber::from_zero_indexed(line_idx))
                        .or_default();
                    for code in &error_codes {
                        line_codes.insert(code.clone());
                    }
                }

                // If the error is inside a multi-line f/t-string, also track
                // the code at the f-string's start and end lines so that a
                // suppression comment placed there is recognized as "used".
                if let Some((fs_start, fs_end)) = containing_range {
                    for code in &error_codes {
                        suppressed_codes
                            .entry(fs_start)
                            .or_default()
                            .insert(code.clone());
                        suppressed_codes
                            .entry(fs_end)
                            .or_default()
                            .insert(code.clone());
                    }
                }
            }
        }

        let mut unused_errors = Vec::new();
        for (applies_to_line, suppressions) in module.ignore().iter() {
            for supp in suppressions {
                if ranges.is_some_and(|ranges| ranges.ignore_all.contains(supp)) {
                    continue;
                }
                let tool = supp.tool();
                // Only check tools that are enabled and that we support
                // reporting unused ignores for (Pyrefly and Pyre).
                if !enabled_ignores.contains(&tool) {
                    continue;
                }
                match tool {
                    Tool::Pyrefly | Tool::Pyre | Tool::Type => {}
                    _ => continue,
                }

                // Get the error codes actually suppressed on this line
                let used_codes: SmallSet<String> = suppressed_codes
                    .get(applies_to_line)
                    .cloned()
                    .unwrap_or_default();
                // For Tool::Pyre, error code filtering is not enforced
                // (any Pyre suppression suppresses all errors on the line),
                // so we only report it as unused when no errors at all were
                // suppressed on its line.
                if tool == Tool::Pyre {
                    if !used_codes.is_empty() {
                        continue; // Pyre suppression is used
                    }
                    let edit = supp.removal_edit(module.lined_buffer(), None);
                    unused_errors.push(
                        Error::new(
                            module.dupe(),
                            edit.range,
                            "Unused pyre-fixme comment".to_owned(),
                            Vec::new(),
                            ErrorKind::UnusedIgnore,
                        )
                        .with_quick_fix(edit.into()),
                    );
                    continue;
                }

                // For `# type: ignore`, line-wide bookkeeping considers it unused
                // only if no suppression effect applies to any diagnostic on this line.
                if tool == Tool::Type {
                    if !used_codes.is_empty() {
                        continue; // type: ignore is used
                    }
                    let edit = supp.removal_edit(module.lined_buffer(), None);
                    unused_errors.push(
                        Error::new(
                            module.dupe(),
                            edit.range,
                            "Unused `# type: ignore` comment".to_owned(),
                            Vec::new(),
                            ErrorKind::UnusedTypeIgnore,
                        )
                        .with_quick_fix(edit.into()),
                    );
                    continue;
                }

                // Tool::Pyrefly: check individual error codes
                let declared_codes: SmallSet<String> = supp.error_codes().iter().cloned().collect();

                // Determine if the suppression is unused
                let unused_codes: SmallSet<String> = if declared_codes.is_empty() {
                    // Blanket ignore - unused if no errors were suppressed
                    if used_codes.is_empty() {
                        SmallSet::new() // Mark as unused (empty set signals blanket unused)
                    } else {
                        continue; // Used, skip
                    }
                } else {
                    // Specific codes - find which are unused
                    let unused: SmallSet<String> = declared_codes
                        .iter()
                        .filter(|code| !used_codes.contains(*code))
                        .cloned()
                        .collect();
                    if unused.is_empty() {
                        continue; // All codes used, skip
                    }
                    unused
                };

                let msg = if declared_codes.is_empty() {
                    "Unused `# pyrefly: ignore` comment".to_owned()
                } else if unused_codes.len() == declared_codes.len() {
                    format!(
                        "Unused `# pyrefly: ignore` comment for code(s): {}",
                        unused_codes.iter().cloned().collect::<Vec<_>>().join(", ")
                    )
                } else {
                    format!(
                        "Unused error code(s) in `# pyrefly: ignore`: {}",
                        unused_codes.iter().cloned().collect::<Vec<_>>().join(", ")
                    )
                };

                let partially_unused =
                    !declared_codes.is_empty() && unused_codes.len() != declared_codes.len();
                let edit = supp.removal_edit(
                    module.lined_buffer(),
                    partially_unused.then_some(&unused_codes),
                );
                unused_errors.push(
                    Error::new(
                        module.dupe(),
                        edit.range,
                        msg,
                        Vec::new(),
                        ErrorKind::UnusedIgnore,
                    )
                    .with_quick_fix(edit.into()),
                );
            }
        }

        unused_errors
    }

    /// Collects unused ignore errors for display, respecting severity configuration.
    /// Unlike `collect_unused_ignore_errors()`, this applies severity filtering so
    /// errors with `Severity::Ignore` are not included in the ordinary results.
    /// Accepts pre-collected errors to avoid redundant error collection.
    pub fn collect_unused_ignore_errors_for_display(
        &self,
        collected: &CollectedErrors,
    ) -> CollectedErrors {
        let unused_errors = self.collect_unused_ignore_errors(collected);
        if unused_errors.is_empty() {
            return CollectedErrors::default();
        }
        let mut result = CollectedErrors::default();
        let config_by_path = self.config_by_path();
        let self_suppressed = self.self_suppressed_unused_ignore_ranges();

        for error in unused_errors {
            if error.error_kind() == ErrorKind::UnusedIgnore
                && self_suppressed.contains(&(error.path(), error.range()))
            {
                continue;
            }
            if let Some(config) = config_by_path.get(&error.path()) {
                let error_config = config.get_error_config(error.path());
                let severity = error_config.display_config.severity(error.error_kind());
                let error = error.with_severity(severity);
                match severity {
                    Severity::Ignore => result.disabled.push(error),
                    Severity::Hint => result.hints.push(error),
                    Severity::Info | Severity::Warn | Severity::Error => {
                        result.ordinary.push(error)
                    }
                }
            }
        }

        result
    }

    pub fn check_against_expectations(&self) -> anyhow::Result<()> {
        for module in &self.loads {
            let Some(errors) = Self::collect_module_error_refs(module) else {
                continue;
            };
            let mut result = CollectedErrors::default();
            errors.into_owned(&mut result);
            let module_info = &module.0.module_info;
            let output_errors = Self::merge_display_errors(result.ordinary, result.directives);
            Expectation::parse(module_info.dupe(), module_info.contents()).check(&output_errors)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use dupe::Dupe;
    use pyrefly_build::handle::Handle;
    use pyrefly_config::error_kind::ErrorKind;
    use pyrefly_config::error_kind::Severity;
    use pyrefly_python::ast::Ast;
    use pyrefly_python::ignore::Ignore;
    use pyrefly_python::module::Module;
    use pyrefly_python::module_name::ModuleName;
    use pyrefly_python::module_path::ModulePath;
    use pyrefly_python::sys_info::SysInfo;
    use pyrefly_util::arc_id::ArcId;
    use pyrefly_util::fs_anyhow;
    use pyrefly_util::thread_pool::TEST_THREAD_COUNT;
    use regex::Regex;
    use ruff_text_size::Ranged;
    use tempfile::TempDir;

    use crate::config::config::ConfigFile;
    use crate::config::finder::ConfigFinder;
    use crate::error::error::ErrorRenderer;
    use crate::error::suppress::SerializedError;
    use crate::error::suppress::UnusedIgnoreKind;
    use crate::error::suppress::remove_unused_ignores_from_serialized;
    use crate::state::errors::Errors;
    use crate::state::errors::PARALLEL_COLLATION_MIN_MODULES;
    use crate::state::load::FileContents;
    use crate::state::require::Require;
    use crate::state::state::State;
    use crate::test::util::TestEnv;

    impl Errors {
        pub fn check_var_leak(&self) -> anyhow::Result<()> {
            // Match variable ids such as `@12`, but not source locations such as
            // `@128:7-21` that qualified names in messages can carry.
            let regex = Regex::new(r"@\d+(?:$|[^0-9:])").unwrap();
            let errors = self.collect_errors();
            for error in errors
                .ordinary
                .iter()
                .chain(&errors.hints)
                .chain(&errors.directives)
                .chain(&errors.suppressed)
                .chain(&errors.disabled)
            {
                let msg = error.msg();
                if regex.is_match(&msg) {
                    return Err(anyhow::anyhow!(
                        "{}:{}: variable ids leaked into error message: {}",
                        error.path(),
                        error.display_range(),
                        msg,
                    ));
                }
            }
            Ok(())
        }
    }

    fn get_path(tdir: &TempDir) -> PathBuf {
        tdir.path().join("test.py")
    }

    fn get_errors(contents: &str) -> (Errors, TempDir) {
        get_errors_with_config(contents, |_| {})
    }

    fn get_errors_with_config(
        contents: &str,
        configure: impl FnOnce(&mut ConfigFile),
    ) -> (Errors, TempDir) {
        let tdir = tempfile::tempdir().unwrap();

        let mut config = ConfigFile::default();
        config.python_environment.set_empty_to_default();
        configure(&mut config);
        let name = "test";
        fs_anyhow::write(&get_path(&tdir), contents).unwrap();
        config.configure();

        let config = ArcId::new(config);
        let sys_info = SysInfo::default();
        let state = State::new(ConfigFinder::new_constant(config), TEST_THREAD_COUNT);
        let handle = Handle::new(
            ModuleName::from_str(name),
            ModulePath::filesystem(get_path(&tdir)),
            sys_info.dupe(),
        );
        let mut transaction = state.new_transaction(Require::Exports, None);
        transaction.set_memory(vec![(
            get_path(&tdir),
            Some(Arc::new(FileContents::from_source(contents.to_owned()))),
        )]);
        transaction.run(&[handle.dupe()], Require::Everything, None);
        (transaction.get_errors([handle.clone()].iter()), tdir)
    }

    #[test]
    fn test_many_strings_on_one_non_ascii_line() {
        let mut contents = String::from("x = {'non_ascii_ä': 0,");
        for i in 0..100_000 {
            write!(contents, "'key_{i}': {i},").unwrap();
        }
        contents.push('}');
        let module = Module::new(
            ModuleName::from_str("test"),
            ModulePath::filesystem(Path::new("test.py").to_owned()),
            Arc::new(contents),
        );
        let ast = Ast::parse(module.contents(), module.source_type()).0;

        assert!(super::sorted_multi_line_string_ranges(&ast, &module).is_empty());
    }

    #[test]
    fn test_unused_blanket_ignore() {
        // A blanket ignore comment with no errors to suppress
        let contents = r#"
def f() -> int:
    # pyrefly: ignore
    return 1
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 1);
        assert!(unused[0].msg().contains("Unused"));
    }

    #[test]
    fn test_unused_ignore_disabled_by_severity_keeps_its_severity() {
        // `unused-ignore` defaults to `Severity::Ignore`, so it is not a result
        // to display. Callers that want it anyway, such as `pyrefly buck-check`
        // feeding `--remove-unused-ignores`, read `disabled` and rely on the
        // severity to say it should not be reported.
        let contents = r#"
def f() -> int:
    # pyrefly: ignore
    return 1
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors_for_display(&collected);
        assert!(unused.ordinary.is_empty());
        assert_eq!(unused.disabled.len(), 1);
        assert_eq!(unused.disabled[0].severity(), Severity::Ignore);
    }

    #[test]
    fn test_collect_errors_from_many_modules_in_path_order() {
        let count = PARALLEL_COLLATION_MIN_MODULES + 6;
        let names = (0..count).map(|i| format!("m{i:03}")).collect::<Vec<_>>();
        let mut env = TestEnv::new();
        for name in &names {
            env.add(name, "x: int = ''\ny: int = 1  # pyrefly: ignore\n");
        }
        let (state, handle) = env.to_state();
        let handles = names.iter().map(|name| handle(name)).collect::<Vec<_>>();
        let transaction = state.transaction();
        let errors = transaction.get_errors(&handles);
        // Collation runs in parallel only on a thread pool.
        let collected = state.install(|| errors.collect_errors());
        let modules = collected
            .ordinary
            .iter()
            .map(|error| error.module().name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(modules, names, "each module's error, in path order");
        assert!(
            collected
                .ordinary
                .iter()
                .all(|error| error.error_kind() == ErrorKind::BadAssignment)
        );
        let unused = state.install(|| errors.collect_unused_ignore_errors(&collected));
        let modules = unused
            .iter()
            .map(|error| error.module().name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(modules, names, "each module's unused ignore, in path order");
    }

    #[test]
    fn test_unused_ignores_are_isolated_by_module() {
        let mut env = TestEnv::new();
        env.add(
            "used",
            r#"
def f() -> int:
    # pyrefly: ignore [bad-return]
    return "x"
"#,
        );
        env.add(
            "unused",
            r#"
def f() -> int:
    # pyrefly: ignore [bad-return]
    return 1
"#,
        );
        let (state, handle) = env.to_state();
        let handles = [handle("used"), handle("unused")];
        let transaction = state.transaction();
        let errors = transaction.get_errors(&handles);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);

        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].module().name(), ModuleName::from_str("unused"));
        assert!(unused[0].msg().contains("bad-return"));
    }

    #[test]
    fn test_unused_specific_code_ignore() {
        // An ignore comment with a specific code that doesn't match any error
        let contents = r#"
def f() -> int:
    # pyrefly: ignore [bad-override]
    return 1
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 1);
        assert!(unused[0].msg().contains("bad-override"));
    }

    #[test]
    fn test_unused_ignore_self_suppression_for_display() {
        for (contents, raw_count, visible_count, type_error_count) in [
            ("# pyrefly: ignore[unused-ignore]\nx = 1\n", 1, 0, 0),
            ("x = 1  # pyrefly: ignore[unused-ignore]\n", 1, 0, 0),
            (
                "# pyrefly: ignore[bad-assignment, unused-ignore]\nx = 1\n",
                1,
                0,
                0,
            ),
            (
                "# pyrefly: ignore[bad-assignment, unused-ignore]\nx: int = \"\"\n",
                1,
                0,
                0,
            ),
            ("x: int = \"\"  # pyrefly: ignore[unused-ignore]\n", 1, 0, 1),
            ("# pyrefly: ignore[bad-assignment]\nx = 1\n", 1, 1, 0),
            ("# pyrefly: ignore\nx = 1\n", 1, 1, 0),
            (
                "# pyrefly: ignore[unused-ignore]\n# pyrefly: ignore[bad-assignment]\nx = 1\n",
                2,
                1,
                0,
            ),
            (
                "x = 1  # pyrefly: ignore[unused-ignore]  # pyrefly: ignore[bad-assignment]\n",
                2,
                1,
                0,
            ),
        ] {
            let (errors, _tdir) = get_errors_with_config(contents, |config| {
                config
                    .root
                    .errors
                    .get_or_insert_with(Default::default)
                    .set_error_severity(ErrorKind::UnusedIgnore, Severity::Error);
            });
            let collected = errors.collect_errors();
            assert_eq!(collected.ordinary.len(), type_error_count, "{contents}");
            assert_eq!(
                errors.collect_unused_ignore_errors(&collected).len(),
                raw_count,
                "{contents}",
            );
            let display = errors.collect_unused_ignore_errors_for_display(&collected);
            assert_eq!(display.ordinary.len(), visible_count, "{contents}");
            assert!(display.disabled.is_empty(), "{contents}");
            assert!(display.hints.is_empty(), "{contents}");
            assert_eq!(
                errors.collect_lsp_errors_with_baselines().0.len(),
                visible_count + type_error_count,
                "{contents}",
            );
        }
    }

    #[test]
    fn test_unused_ignore_self_suppression_preserves_cleanup() {
        for (contents, expected) in [
            (
                "x = 1  # pyrefly: ignore[bad-assignment, unused-ignore]\n",
                "x = 1\n",
            ),
            (
                "x: int = \"\"  # pyrefly: ignore[bad-assignment, unused-ignore]\n",
                "x: int = \"\"  # pyrefly: ignore[bad-assignment]\n",
            ),
        ] {
            let (errors, tdir) = get_errors(contents);
            let collected = errors.collect_errors();
            let unused = errors.collect_unused_ignore_errors(&collected);
            assert_eq!(unused.len(), 1);
            let serialized = unused
                .iter()
                .filter_map(SerializedError::from_error)
                .collect();
            assert_eq!(
                remove_unused_ignores_from_serialized(serialized, UnusedIgnoreKind::Pyrefly)
                    .unwrap(),
                1,
            );
            assert_eq!(
                fs_anyhow::read_to_string(&get_path(&tdir)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn test_used_ignore_no_errors() {
        // An ignore comment that is actually used should not be reported
        let contents = r#"
def f() -> int:
    # pyrefly: ignore [bad-return]
    return "hello"
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert!(unused.is_empty());
    }

    #[test]
    fn test_partially_used_ignore() {
        // An ignore with multiple codes where only some are used
        let contents = r#"
def f() -> int:
    # pyrefly: ignore [bad-return, bad-override]
    return "hello"
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 1);
        assert!(unused[0].msg().contains("bad-override"));
        assert!(!unused[0].msg().contains("bad-return"));
    }

    #[test]
    fn test_ignore_comments_suppress_hint() {
        let contents = r#"
def f(x: int) -> int:  # pyrefly: ignore
    return 1

def g(x: int) -> int:  # pyrefly: ignore[unused-parameter]
    return 1
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        assert_eq!(
            collected
                .suppressed
                .iter()
                .map(|e| e.error_kind())
                .collect::<Vec<_>>(),
            vec![ErrorKind::UnusedParameter, ErrorKind::UnusedParameter]
        );
        assert!(errors.collect_unused_ignore_errors(&collected).is_empty());
    }

    #[test]
    fn test_no_ignores_no_errors() {
        // Code with no ignores should produce no unused ignore errors
        let contents = r#"
def f() -> int:
    return 1
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert!(unused.is_empty());
    }

    #[test]
    fn test_multiple_unused_ignores() {
        // Multiple unused ignores in the same file
        let contents = r#"
def f() -> int:
    # pyrefly: ignore [bad-override]
    return 1

def g() -> str:
    # pyrefly: ignore
    return "hello"
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 2);
    }

    #[test]
    fn test_unused_ignore_ranges_cover_suppression_comments() {
        let contents = "\
# type: ignore
x = 1
y: int = \"bad\"  # pyrefly: ignore [bad-assignment, unknown-name]
z = 2  # pyrefly: ignore
";
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        let mut ranges = unused
            .iter()
            .map(|error| error.module().code_at(error.range()))
            .collect::<Vec<_>>();
        ranges.sort_unstable();
        assert_eq!(
            ranges,
            vec![
                "# pyrefly: ignore",
                "# pyrefly: ignore [bad-assignment, unknown-name]",
                "# type: ignore",
            ]
        );
    }

    #[test]
    fn test_unused_type_ignore_no_error() {
        let contents = r#"
def f() -> int:
    return 1  # type: ignore
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 1);
        assert!(unused[0].msg().contains("type: ignore"));
    }

    #[test]
    fn test_used_type_ignore_suppresses_error() {
        let contents = r#"
def f() -> int:
    return "hello"  # type: ignore
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert!(unused.is_empty());
    }

    #[test]
    fn test_unused_type_ignore_with_codes() {
        // mypy-style codes are not pyrefly codes; treat as blanket ignore
        let contents = r#"
def f() -> int:
    return 1  # type: ignore[assignment]
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 1);
        assert!(unused[0].msg().contains("type: ignore"));
    }

    #[test]
    fn test_unused_type_ignore_after_multibyte_character_renders() {
        let contents = "あ = '' # type: ignore\n";
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert_eq!(unused.len(), 1);
        assert_eq!(
            unused[0].lined_buffer().code_at(unused[0].range()),
            "# type: ignore"
        );

        let mut renderer = ErrorRenderer::plain(Vec::new());
        renderer.write(&unused[0], Path::new(""), true).unwrap();
    }

    #[test]
    fn test_collect_errors_keeps_parse_errors_in_source_order() {
        let contents = r#"
1 +
x: int = ""
"#;
        let (errors, _tdir) = get_errors(contents);
        let kinds = errors
            .collect_errors()
            .ordinary
            .iter()
            .map(|error| error.error_kind())
            .collect::<Vec<_>>();
        assert_eq!(kinds, vec![ErrorKind::ParseError, ErrorKind::BadAssignment]);
    }

    #[test]
    fn test_collect_errors_puts_parse_errors_first_at_the_same_range() {
        let (errors, _tdir) = get_errors("*e: int = (42,)\n");
        let errors = errors.collect_errors().ordinary;
        assert_eq!(errors.len(), 2);
        assert_eq!(
            errors[0].range(),
            errors[1].range(),
            "both diagnostics should report the starred target"
        );
        assert_eq!(errors[0].error_kind(), ErrorKind::ParseError);
    }

    #[test]
    fn test_used_ignore_multiline_fstring() {
        let contents = r#"
def f(some_condition: bool):
    if some_condition:
        foo = 1
    # pyrefly: ignore[unbound-name]
    bar = f"""The result is:
{foo}"""
"#;
        let (errors, _tdir) = get_errors(contents);
        let collected = errors.collect_errors();
        let unused = errors.collect_unused_ignore_errors(&collected);
        assert!(unused.is_empty());
    }

    #[test]
    fn test_backslash_continuation_ranges_ignores_comment_backslash() {
        use pyrefly_util::lined_buffer::LineNumber;

        use super::sorted_backslash_continuation_ranges;

        let no_strings = vec![];

        // A trailing backslash inside a comment should NOT trigger continuation.
        let lines = vec!["x = 1  # comment \\", "y = 2"];
        let ranges = sorted_backslash_continuation_ranges(
            &lines,
            &no_strings,
            &Ignore::new(&lines.join("\n")),
        );
        assert!(
            ranges.is_empty(),
            "comment backslash should not be a continuation"
        );

        // A real continuation should still be detected.
        let lines = vec!["x = 1 + \\", "    2"];
        let ranges = sorted_backslash_continuation_ranges(
            &lines,
            &no_strings,
            &Ignore::new(&lines.join("\n")),
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].0, LineNumber::from_zero_indexed(0));
        assert_eq!(ranges[0].1, LineNumber::from_zero_indexed(1));
    }

    #[test]
    fn test_backslash_continuation_ranges_ignores_multiline_strings() {
        use pyrefly_util::lined_buffer::LineNumber;

        use super::sorted_backslash_continuation_ranges;

        // A backslash at end of line inside a triple-quoted string should
        // NOT be detected as a continuation.
        let lines = vec![
            "x = \"\"\"\\", // line 0: start of triple-quoted string with \
            "hello\\",      // line 1: inside string with \
            "\"\"\"",       // line 2: end of string
        ];
        let string_ranges = vec![(
            LineNumber::from_zero_indexed(0),
            LineNumber::from_zero_indexed(2),
        )];
        let ranges = sorted_backslash_continuation_ranges(
            &lines,
            &string_ranges,
            &Ignore::new(&lines.join("\n")),
        );
        assert!(
            ranges.is_empty(),
            "backslash inside multiline string should not be a continuation"
        );
    }
}
