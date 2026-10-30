/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Server orchestration for runnable and reference-count CodeLens providers.

mod reference_count;
mod runnable;

use std::collections::HashSet;

use lsp_server::ErrorCode;
use lsp_server::RequestId;
use lsp_types::CodeLens;
use lsp_types::CodeLensParams;
use lsp_types::CodeLensRefreshRequest;
use lsp_types::CodeLensRequest;
use lsp_types::Command;
use lsp_types::Request as _;
use lsp_types::Uri;
use pyrefly_util::telemetry::ActivityKey;
use pyrefly_util::telemetry::EmptyResponseReason;

use super::FindReferencesRequest;
use super::ReferenceTaskKind;
use super::Server;
use crate::commands::lsp::IndexingMode;
use crate::lsp::non_wasm::lsp::new_response;
use crate::lsp::non_wasm::protocol::Response;
use crate::state::lsp::FindPreference;
use crate::state::lsp::ImportBehavior;
use crate::state::lsp::ReferenceOptions;
use crate::state::state::Transaction;

/// Failures that prevent a reference lens from resolving.
pub(super) enum CodeLensError {
    MissingData,
    InvalidData(serde_json::Error),
    ContentModified,
    Unavailable(EmptyResponseReason),
}

impl From<EmptyResponseReason> for CodeLensError {
    fn from(reason: EmptyResponseReason) -> Self {
        Self::Unavailable(reason)
    }
}

impl CodeLensError {
    /// Convert the failure to one LSP error response.
    pub(super) fn into_response(self, id: RequestId, lens: CodeLens) -> Response {
        let (code, message) = match self {
            Self::MissingData => (
                ErrorCode::InvalidParams,
                "Reference CodeLens is missing resolve data".to_owned(),
            ),
            Self::InvalidData(error) => (
                ErrorCode::InvalidParams,
                format!("Invalid reference CodeLens data: {error}"),
            ),
            Self::ContentModified => (
                ErrorCode::ContentModified,
                "The document changed. Request new CodeLens items.".to_owned(),
            ),
            Self::Unavailable(
                EmptyResponseReason::NotAnIdentifier { .. }
                | EmptyResponseReason::DefinitionNotFound { .. },
            ) => {
                // An empty command displays a disabled label in the editor.
                return new_response(
                    id,
                    Ok(CodeLens {
                        command: Some(Command {
                            title: "References unavailable".to_owned(),
                            command: String::new(),
                            arguments: None,
                            tooltip: None,
                        }),
                        data: None,
                        ..lens
                    }),
                );
            }
            Self::Unavailable(_) => (
                ErrorCode::RequestFailed,
                "References are unavailable for this CodeLens.".to_owned(),
            ),
        };
        Response::new_err(id, code as i32, message)
    }
}

impl Server {
    /// Request new counts after validation or a configuration change.
    pub(super) fn refresh_reference_code_lenses(&self) {
        if self.supports_references_code_lens()
            && self
                .initialize_params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.code_lens.as_ref())
                .and_then(|code_lens| code_lens.refresh_support)
                == Some(true)
        {
            self.send_request::<CodeLensRefreshRequest>(());
        }
    }

    /// Return runnable lenses and unresolved reference lenses for supported clients.
    pub(super) fn code_lens(
        &self,
        transaction: &Transaction<'_>,
        params: CodeLensParams,
    ) -> Result<Option<Vec<CodeLens>>, EmptyResponseReason> {
        let uri = &params.text_document.uri;
        if self.open_notebook_cells.read().contains_key(uri) {
            return Ok(Some(Vec::new()));
        }
        let handle = self.make_handle_if_enabled(uri, Some(CodeLensRequest::METHOD.as_str()))?;
        let mut lenses = runnable::code_lenses(self, transaction, &handle, uri)?;
        if self.references_code_lens_enabled(uri) {
            let version = self.text_document_version(uri);
            lenses.extend(reference_count::unresolved_code_lenses(
                transaction,
                &handle,
                uri,
                version,
            )?);
        }
        Ok(Some(lenses))
    }

    /// Check both client support and the resource setting before offering reference commands.
    fn references_code_lens_enabled(&self, uri: &Uri) -> bool {
        self.supports_references_code_lens()
            && self.path_for_uri(uri).is_some_and(|path| {
                self.workspaces
                    .get_with(path, |(_, workspace)| workspace.references_code_lens)
            })
    }

    /// Check whether the client can execute reference lenses while indexing is enabled.
    pub(super) fn supports_references_code_lens(&self) -> bool {
        self.indexing_mode != IndexingMode::None
            && self
                .initialize_params
                .initialization_options
                .as_ref()
                .and_then(|options| options.get("pyrefly"))
                .and_then(|options| options.get("supportsReferencesCodeLens"))
                .and_then(serde_json::Value::as_bool)
                == Some(true)
    }

    /// Resolve one reference lens using the same search as Find References.
    pub(super) fn resolve_code_lens<'a>(
        &'a self,
        request_id: RequestId,
        transaction: &Transaction<'a>,
        lens: &CodeLens,
        activity_key: Option<ActivityKey>,
    ) -> Result<(), CodeLensError> {
        let data = reference_count::resolve_data(lens)?;
        if self.open_notebook_cells.read().contains_key(&data.uri) {
            return Err(EmptyResponseReason::NotebookNotSupported.into());
        }
        if !self.references_code_lens_enabled(&data.uri) {
            return Err(EmptyResponseReason::MethodDisabled.into());
        }
        if self.text_document_version(&data.uri) != data.version {
            return Err(CodeLensError::ContentModified);
        }
        let handle = self
            .make_handle_if_enabled(&data.uri, Some(CodeLensRequest::METHOD.as_str()))
            .map_err(EmptyResponseReason::from)?;
        let notebook_cells: HashSet<_> = self.open_notebook_cells.read().keys().cloned().collect();
        let lens = lens.clone();
        let source_uri = data.uri.clone();
        self.async_find_references_helper(
            transaction,
            FindReferencesRequest {
                request_id,
                handle,
                uri: data.uri,
                position: lens.range.start,
                find_preference: FindPreference {
                    import_behavior: ImportBehavior::StopAtRenamedImports,
                    ..Default::default()
                },
                options: ReferenceOptions::all(false),
                activity_key,
                kind: ReferenceTaskKind::CodeLens {
                    version: data.version,
                },
            },
            move |results| {
                reference_count::resolve_code_lens(lens, &source_uri, results, &notebook_cells)
            },
        )?;
        Ok(())
    }
}
