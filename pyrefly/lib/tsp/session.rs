/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A serialized filesystem-backed session for native and WASI hosts.
//!
//! The host owns the filesystem and presents editor contents at their ordinary
//! paths. It must serialize filesystem writes, change events and queries.
//! This session owns no editor buffers, transport, worker or background watcher.

use std::path::Path;
use std::path::PathBuf;

use dupe::Dupe;
use lsp_server::ResponseError;
use lsp_types::Position;
use pyrefly_build::handle::Handle;
use pyrefly_config::config::ConfigFile;
use pyrefly_config::finder::ConfigFinder;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::events::CategorizedEvents;
use pyrefly_util::thread_pool::ThreadCount;
use ruff_text_size::TextRange;
use serde_json::json;
use tsp_types::GetTypeParams;
use tsp_types::SnapshotChangedNotification;
use tsp_types::TSPNotificationMethods;
use tsp_types::Type;

use crate::state::require::Require;
use crate::state::state::State;
use crate::tsp::type_conversion::convert_type_in_transaction;
use crate::tsp::validation::invalid_params_error;
use crate::tsp::validation::parse_uri;
use crate::tsp::validation::snapshot_outdated_error;

pub struct FilesystemSession {
    state: State,
    config: ArcId<ConfigFile>,
    handles: Vec<Handle>,
}

impl FilesystemSession {
    /// Configure a session. WASI hosts should disable interpreter queries and
    /// provide explicit search roots and the intended Python environment.
    pub fn new(mut config: ConfigFile, threads: ThreadCount) -> Result<Self, ResponseError> {
        let errors = config.configure();
        if !errors.is_empty() {
            let messages = errors
                .iter()
                .map(|error| error.get_message())
                .collect::<Vec<_>>()
                .join("\n");
            return Err(invalid_params_error(&format!(
                "Configuration errors: {messages}"
            )));
        }
        let config = ArcId::new(config);
        Ok(Self {
            state: State::new(ConfigFinder::new_constant(config.dupe()), threads),
            config,
            handles: Vec::new(),
        })
    }

    /// Set the filesystem files to analyze, without altering their contents.
    pub fn set_files(&mut self, files: Vec<PathBuf>) -> Result<(), ResponseError> {
        for path in &files {
            Self::validate_path(path)?;
        }
        self.handles = files
            .into_iter()
            .map(|path| {
                self.config
                    .handle_from_module_path(ModulePath::filesystem(path))
            })
            .collect();
        let transaction = self
            .state
            .new_committable_transaction(Require::Exports, None);
        self.state.run_with_committing_transaction(
            transaction,
            &self.handles,
            Require::Everything,
            None,
            None,
        );
        Ok(())
    }

    /// Invalidate after the host has changed its filesystem. Empty batches
    /// are no-ops. Hosts should also avoid reporting duplicate content writes.
    /// Return the protocol notification the host should forward to its client.
    pub fn files_changed(
        &mut self,
        events: &CategorizedEvents,
    ) -> Result<Option<SnapshotChangedNotification>, ResponseError> {
        for path in events.iter() {
            Self::validate_path(path)?;
        }
        if events.iter().next().is_none() {
            return Ok(None);
        }
        let old = self.get_snapshot();
        let mut transaction = self
            .state
            .new_committable_transaction(Require::Exports, None);
        transaction.as_mut().invalidate_events(events);
        self.state.run_with_committing_transaction(
            transaction,
            &self.handles,
            Require::Everything,
            None,
            None,
        );
        let new = self.get_snapshot();
        Ok((old != new).then(|| SnapshotChangedNotification {
            jsonrpc: "2.0".to_owned(),
            method: TSPNotificationMethods::TypeServerSnapshotChanged,
            params: Some(json!({ "old": old, "new": new })),
        }))
    }

    /// A session-local snapshot of committed filesystem input changes. All
    /// editor mutations use files_changed, so there is no second edit counter.
    pub fn get_snapshot(&self) -> i32 {
        self.state.generation() as i32
    }

    /// Answer the existing TSP request shape using a matching warm transaction.
    /// File URI queries and UTF-16 ranges are supported; notebook cell URI
    /// projection belongs to a future host adapter.
    pub fn get_computed_type(
        &mut self,
        params: &GetTypeParams,
    ) -> Result<Option<Type>, ResponseError> {
        self.validate_snapshot(params.snapshot)?;
        let uri = parse_uri(params.uri())?;
        if uri.scheme() != "file" {
            return Err(invalid_params_error("This session requires a file URI"));
        }
        let path = uri
            .to_file_path()
            .map_err(|_| invalid_params_error("URI does not identify a filesystem path"))?;
        Self::validate_path(&path)?;
        let start = params.position();
        let end = params.end_position();
        if (start.line, start.character) > (end.line, end.character) {
            return Err(invalid_params_error("Range start must not follow its end"));
        }
        let handle = self
            .config
            .handle_from_module_path(ModulePath::filesystem(path));
        let mut transaction = self
            .state
            .new_committable_transaction(Require::Exports, None);
        transaction
            .as_mut()
            .run(std::slice::from_ref(&handle), Require::Everything, None);
        let transaction =
            self.state
                .commit_transaction_downgrade(transaction, None, Require::Exports);
        let result = (|| {
            let module = transaction
                .get_module_info(&handle)
                .ok_or_else(|| invalid_params_error("Module is not available for a type query"))?;
            if module.is_notebook() {
                return Err(invalid_params_error(
                    "Notebook queries require cell projection",
                ));
            }
            let start = Position {
                line: start.line,
                character: start.character,
            };
            let end = Position {
                line: end.line,
                character: end.character,
            };
            let start_offset = module.from_lsp_position(start, None);
            let end_offset = module.from_lsp_position(end, None);
            if module.to_lsp_position(start_offset) != start
                || module.to_lsp_position(end_offset) != end
            {
                return Err(invalid_params_error(
                    "Range is outside the module or splits a UTF-16 character",
                ));
            }
            Ok(transaction
                .get_computed_type_at_range(&handle, TextRange::new(start_offset, end_offset))
                .map(|ty| convert_type_in_transaction(&transaction, &handle, &ty)))
        })();
        drop(transaction);
        self.validate_snapshot(params.snapshot)?;
        result
    }

    fn validate_snapshot(&self, snapshot: i32) -> Result<(), ResponseError> {
        let current = self.get_snapshot();
        if snapshot != current {
            return Err(snapshot_outdated_error(snapshot, current));
        }
        Ok(())
    }

    fn validate_path(path: &Path) -> Result<(), ResponseError> {
        if !path.is_absolute() {
            return Err(invalid_params_error("Filesystem paths must be absolute"));
        }
        Ok(())
    }
}
