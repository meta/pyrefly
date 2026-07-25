/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use lsp_types::Location;
use lsp_types::Range;
use lsp_types::Uri;
use lsp_types::VersionedTextDocumentIdentifier;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;


/// A document batch for override lenses. Ranges use the specified editor version.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideCodeLensParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub ranges: Vec<Range>,
    /// The client handles `pyrefly/overrideCodeLens/refresh` notifications.
    #[serde(default)]
    pub refresh_support: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OverrideCodeLensTarget {
    #[serde(flatten)]
    pub location: Location,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OverrideCodeLensTargets {
    pub parents: Vec<OverrideCodeLensTarget>,
    pub implementations: Vec<OverrideCodeLensTarget>,
}

/// Invalidates override lenses after the server updates its analysis.
pub enum OverrideCodeLensRefreshNotification {}

impl lsp_types::Notification for OverrideCodeLensRefreshNotification {
    type Params = Value;
    const METHOD: lsp_types::LspNotificationMethod<'static> =
        lsp_types::LspNotificationMethod::Custom("pyrefly/overrideCodeLens/refresh");
    const MESSAGE_DIRECTION: lsp_types::MessageDirection =
        lsp_types::MessageDirection::ServerToClient;
}

pub enum OverrideCodeLensRequest {}

impl lsp_types::Request for OverrideCodeLensRequest {
    type Params = OverrideCodeLensParams;
    type Result = Vec<OverrideCodeLensTargets>;
    const METHOD: lsp_types::LspRequestMethod<'static> =
        lsp_types::LspRequestMethod::Custom("pyrefly/textDocument/overrideCodeLens");
    const MESSAGE_DIRECTION: lsp_types::MessageDirection =
        lsp_types::MessageDirection::ClientToServer;
}
