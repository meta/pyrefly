/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context as _;
use anyhow::ensure;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use dupe::Dupe;
use lsp_types::CallHierarchyIncomingCall;
use lsp_types::CallHierarchyItem;
use lsp_types::CallHierarchyOutgoingCall;
use lsp_types::Location;
use lsp_types::Position;
use lsp_types::Range;
use lsp_types::Uri;
use pyrefly_config::args::ConfigOverrideArgs;
use pyrefly_config::config::ConfigScope;
use pyrefly_python::module::TextRangeWithModule;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_name::ModuleNameWithKind;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::absolutize::Absolutize as _;
use pyrefly_util::includes::Includes;
use pyrefly_util::thread_pool::ThreadCount;
use serde::Serialize;
use serde_json::Value;

use crate::commands::check::Handles;
use crate::commands::config_finder::ConfigConfigurerWrapper;
use crate::commands::config_finder::default_config_finder_with_overrides;
use crate::commands::files::add_config_errors;
use crate::commands::files::get_explicit_config;
use crate::commands::util::CommandExitStatus;
use crate::config::finder::ConfigFinder;
use crate::lsp::non_wasm::call_hierarchy::find_function_at_position_in_ast;
use crate::lsp::non_wasm::call_hierarchy::prepare_call_hierarchy_item;
use crate::lsp::non_wasm::call_hierarchy::transform_incoming_calls;
use crate::lsp::non_wasm::call_hierarchy::transform_outgoing_calls;
use crate::lsp::non_wasm::module_helpers::handle_from_module_path;
use crate::lsp::non_wasm::module_helpers::module_info_to_uri;
use crate::lsp::wasm::hover::get_hover;
use crate::state::lsp::FindPreference;
use crate::state::lsp::ImportBehavior;
use crate::state::lsp::ReferenceOptions;
use crate::state::require::Require;
use crate::state::state::State;

/// Query semantic information about a source position.
#[derive(Clone, Debug, Parser)]
#[command(
    mut_args = |arg| arg.global(true),
    after_help = "Experimental interface. Inputs use one-based lines and UTF-16 columns. JSON results use file URIs and zero-based LSP ranges. To query a returned location, convert its URI to a path and add 1 to its line and character."
)]
pub struct QueryArgs {
    /// Semantic query to run.
    #[command(subcommand)]
    query: Query,

    /// Explicitly set the Pyrefly configuration to use.
    #[arg(long, short, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Configuration override options.
    #[command(flatten, next_help_heading = "Config Overrides")]
    config_override: ConfigOverrideArgs,
}

/// Semantic queries supported by the command-line interface.
#[derive(Clone, Debug, Subcommand)]
enum Query {
    /// Show hover information at a source position.
    Hover(QueryPosition),

    /// Find references to the symbol at a source position.
    FindReferences {
        /// Source position to query.
        #[command(flatten)]
        position: QueryPosition,

        /// Include the symbol's declaration in the results.
        #[arg(long)]
        include_declaration: bool,
    },

    /// Show incoming and outgoing calls for the callable at a source position.
    CallHierarchy(QueryPosition),
}

/// A one-based source position.
#[derive(Args, Clone, Debug)]
struct QueryPosition {
    /// Python file containing the position.
    file: PathBuf,

    /// One-based line number. JSON results use zero-based lines.
    line: u32,

    /// One-based UTF-16 column number. JSON results use zero-based characters.
    column: u32,
}

impl QueryPosition {
    /// Convert the user-facing one-based position to an LSP position.
    fn to_lsp_position(&self) -> anyhow::Result<Position> {
        let line = self
            .line
            .checked_sub(1)
            .context("line must be greater than zero")?;
        let character = self
            .column
            .checked_sub(1)
            .context("column must be greater than zero")?;
        Ok(Position::new(line, character))
    }
}

/// JSON representation of a complete call hierarchy query.
#[derive(Serialize)]
struct CallHierarchyResult {
    /// The callable resolved from the requested source position.
    item: CallHierarchyItem,
    /// Functions that call the resolved callable.
    incoming: Vec<CallHierarchyIncomingCall>,
    /// Functions called by the resolved callable.
    outgoing: Vec<CallHierarchyOutgoingCall>,
}

/// Order locations by URI, range start, and range end.
fn location_key<'a>(uri: &'a Uri, range: &Range) -> (&'a Uri, u32, u32, u32, u32) {
    (
        uri,
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
    )
}

impl QueryArgs {
    /// Run the query and write one JSON value to stdout.
    pub fn run(
        self,
        wrapper: Option<ConfigConfigurerWrapper>,
        thread_count: ThreadCount,
    ) -> anyhow::Result<CommandExitStatus> {
        let result = self.query(wrapper, thread_count)?;
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();
        serde_json::to_writer(&mut stdout, &result)?;
        writeln!(stdout)?;
        Ok(CommandExitStatus::Success)
    }

    /// Load the project and execute a semantic query.
    fn query(
        self,
        wrapper: Option<ConfigConfigurerWrapper>,
        thread_count: ThreadCount,
    ) -> anyhow::Result<Value> {
        self.config_override.validate()?;
        let position = match &self.query {
            Query::Hover(position) | Query::CallHierarchy(position) => position,
            Query::FindReferences { position, .. } => position,
        };
        let target_path = position.file.absolutize();
        let lsp_position = position.to_lsp_position()?;
        let metadata = fs::metadata(&target_path)
            .with_context(|| format!("cannot access target `{}`", target_path.display()))?;
        ensure!(
            metadata.is_file(),
            "target `{}` is not a file",
            target_path.display()
        );

        let config_finder = match self.config {
            Some(path) => {
                let (config, errors) = get_explicit_config(&path, self.config_override);
                let finder = ConfigFinder::new_constant(config);
                add_config_errors(&finder, errors)?;
                finder
            }
            None => default_config_finder_with_overrides(self.config_override, false, wrapper),
        };
        let config = config_finder.python_file(
            ModuleNameWithKind::guaranteed(ModuleName::unknown()),
            &ModulePath::filesystem(target_path.clone()),
        );
        add_config_errors(&config_finder, config_finder.errors())?;
        let mut files = Vec::new();
        if !matches!(self.query, Query::Hover(_)) {
            let mut includes = config.get_filtered_globs(None, ConfigScope::Default);
            files.extend(config_finder.checkpoint(includes.files_iter())?);
            if let Some(error) = includes.errors().into_iter().next() {
                return Err(error.context("cannot list query project files"));
            }
        }
        files.push(target_path.clone());
        let handles = Handles::new(files);
        let (handles, _, sourcedb_errors) = handles.all(&config_finder);
        add_config_errors(&config_finder, sourcedb_errors)?;
        add_config_errors(&config_finder, config_finder.errors())?;
        config_finder.print_errors();
        let target = handles
            .iter()
            .find(|handle| handle.path().as_path() == target_path)
            .cloned()
            .with_context(|| format!("could not resolve `{}`", target_path.display()))?;

        let state = State::new(config_finder, thread_count);
        let mut transaction = state.new_committable_transaction(Require::Exports, None);
        if !matches!(self.query, Query::Hover(_)) {
            transaction.as_mut().run(&handles, Require::Indexing, None);
        }
        transaction
            .as_mut()
            .run(&[target.dupe()], Require::Everything, None);
        state.commit_transaction(transaction, None);

        let mut transaction = state.cancellable_transaction();
        let load = transaction
            .as_ref()
            .get_load(&target)
            .with_context(|| format!("could not load `{}`", target_path.display()))?;
        if let Some(error) = load.load_errors.iter().next() {
            return Err(anyhow::anyhow!("cannot load target: {}", error.msg()));
        }
        let module = &load.module_info;
        ensure!(
            module.notebook().is_none(),
            "notebook query targets are not supported"
        );
        let position = module
            .lined_buffer()
            .try_from_lsp_position(lsp_position)
            .with_context(|| format!("cannot query `{}`", target_path.display()))?;

        match self.query {
            Query::Hover(_) => Ok(serde_json::to_value(get_hover(
                transaction.as_ref(),
                &target,
                position,
                false,
            ))?),
            Query::FindReferences {
                include_declaration,
                ..
            } => {
                let definition = transaction
                    .as_ref()
                    .find_definition(
                        &target,
                        position,
                        FindPreference {
                            import_behavior: ImportBehavior::StopAtRenamedImports,
                            ..Default::default()
                        },
                    )
                    .map_err(|reason| {
                        anyhow::anyhow!("no definition found at the requested position: {reason:?}")
                    })?
                    .first()
                    .clone();
                let references = transaction
                    .as_mut()
                    .find_global_references_from_definition(
                        *target.sys_info(),
                        definition.metadata,
                        TextRangeWithModule::new(definition.module, definition.definition_range),
                        ReferenceOptions::all(include_declaration),
                    )
                    .map_err(|_| anyhow::anyhow!("reference query was cancelled"))?;
                let mut locations = Vec::new();
                for (module, ranges) in references {
                    let Some(uri) = module_info_to_uri(&module, None) else {
                        continue;
                    };
                    locations.extend(
                        ranges
                            .into_iter()
                            .map(|range| Location::new(uri.clone(), module.to_lsp_range(range))),
                    );
                }
                locations.sort_by(|left, right| {
                    location_key(&left.uri, &left.range)
                        .cmp(&location_key(&right.uri, &right.range))
                });
                Ok(serde_json::to_value(locations)?)
            }
            Query::CallHierarchy(_) => {
                let definition = transaction
                    .as_ref()
                    .find_definition(&target, position, FindPreference::default())
                    .map_err(|reason| {
                        anyhow::anyhow!("no definition found at the requested position: {reason:?}")
                    })?
                    .first()
                    .clone();
                let definition_handle =
                    handle_from_module_path(&state, definition.module.path().dupe());
                transaction
                    .run(&[definition_handle.dupe()], Require::Everything, None)
                    .map_err(|_| anyhow::anyhow!("definition query was cancelled"))?;
                let ast = transaction
                    .as_ref()
                    .get_ast(&definition_handle)
                    .context("definition AST is unavailable")?;
                let function =
                    find_function_at_position_in_ast(&ast, definition.definition_range.start())
                        .context("the requested symbol is not a function or method")?;
                let uri = module_info_to_uri(&definition.module, None)
                    .context("cannot materialize the definition")?;
                let item = prepare_call_hierarchy_item(function, &definition.module, uri);
                let incoming = transaction
                    .find_global_incoming_calls_from_function_definition(
                        *definition_handle.sys_info(),
                        definition.metadata,
                        &TextRangeWithModule::new(
                            definition.module.dupe(),
                            definition.definition_range,
                        ),
                    )
                    .map_err(|_| anyhow::anyhow!("incoming call query was cancelled"))?;
                let outgoing = transaction
                    .find_global_outgoing_calls_from_function_definition(
                        &definition_handle,
                        definition.definition_range.start(),
                    )
                    .map_err(|_| anyhow::anyhow!("outgoing call query was cancelled"))?;
                let mut incoming = transform_incoming_calls(incoming, None);
                let mut outgoing = transform_outgoing_calls(outgoing, &definition.module, None);
                incoming.sort_by(|left, right| {
                    location_key(&left.from.uri, &left.from.range)
                        .cmp(&location_key(&right.from.uri, &right.from.range))
                });
                outgoing.sort_by(|left, right| {
                    location_key(&left.to.uri, &left.to.range)
                        .cmp(&location_key(&right.to.uri, &right.to.range))
                });
                Ok(serde_json::to_value(CallHierarchyResult {
                    item,
                    incoming,
                    outgoing,
                })?)
            }
        }
    }
}
