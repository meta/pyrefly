/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Lazy reference-count CodeLens creation and resolution.

use std::collections::HashSet;

use lsp_types::CodeLens;
use lsp_types::Command;
use lsp_types::DocumentSymbol;
use lsp_types::Location;
use lsp_types::Range;
use lsp_types::SymbolKind;
use lsp_types::Uri;
use pyrefly_build::handle::Handle;
use pyrefly_util::telemetry::EmptyResponseReason;
use serde::Deserialize;
use serde::Serialize;

use super::CodeLensError;
use crate::state::state::Transaction;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ReferenceCodeLensData {
    pub(super) uri: Uri,
    pub(super) version: Option<i32>,
}

/// Collect unresolved lenses without performing definition lookup or global reference searches.
pub(super) fn unresolved_code_lenses(
    transaction: &Transaction<'_>,
    handle: &Handle,
    uri: &Uri,
    version: Option<i32>,
) -> Result<Vec<CodeLens>, EmptyResponseReason> {
    let symbols = transaction
        .symbols(handle, None)
        .ok_or(EmptyResponseReason::ModuleInfoNotFound)?;
    let mut lenses = Vec::new();
    collect_symbol_lenses(&symbols, uri, version, &mut lenses);
    Ok(lenses)
}

/// Decode the opaque data attached to an unresolved reference-count lens.
pub(super) fn resolve_data(lens: &CodeLens) -> Result<ReferenceCodeLensData, CodeLensError> {
    let data = lens.data.clone().ok_or(CodeLensError::MissingData)?;
    serde_json::from_value(data).map_err(CodeLensError::InvalidData)
}

/// Attach the reference command, excluding every location backed by a notebook cell.
pub(super) fn resolve_code_lens(
    mut lens: CodeLens,
    source_uri: &Uri,
    results: Vec<(Uri, Vec<Range>)>,
    notebook_cells: &HashSet<Uri>,
) -> CodeLens {
    let mut locations = Vec::new();
    for (uri, ranges) in results {
        if notebook_cells.contains(&uri) || uri.path().ends_with(".ipynb") {
            continue;
        }
        for range in ranges {
            locations.push(Location {
                uri: uri.clone(),
                range,
            });
        }
    }

    let reference_count = locations.len();
    let title = if reference_count == 1 {
        "1 reference".to_owned()
    } else {
        format!("{reference_count} references")
    };
    lens.command = Some(Command {
        title,
        command: "pyrefly.showReferences".to_owned(),
        arguments: Some(vec![
            serde_json::to_value(source_uri).expect("URI should serialize for code lens"),
            serde_json::to_value(lens.range.start)
                .expect("Position should serialize for code lens"),
            serde_json::to_value(&locations).expect("Locations should serialize for code lens"),
        ]),
        tooltip: None,
    });
    lens.data = None;
    lens
}

/// Traverse document symbols and create one unresolved lens per reference-countable definition.
fn collect_symbol_lenses(
    symbols: &[DocumentSymbol],
    uri: &Uri,
    version: Option<i32>,
    lenses: &mut Vec<CodeLens>,
) {
    for symbol in symbols {
        if matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Function | SymbolKind::Method
        ) {
            lenses.push(CodeLens {
                range: symbol.selection_range,
                command: None,
                data: Some(
                    serde_json::to_value(ReferenceCodeLensData {
                        uri: uri.clone(),
                        version,
                    })
                    .expect("Reference CodeLens data should serialize"),
                ),
            });
        }
        if let Some(children) = symbol.children.as_deref() {
            collect_symbol_lenses(children, uri, version, lenses);
        }
    }
}
