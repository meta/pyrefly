/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use dupe::Dupe;
use pyrefly_build::handle::Handle;
use pyrefly_python::module::Module;
use ruff_text_size::TextRange;

use crate::state::lsp::AllOffPartial;
use crate::state::lsp::ImportFormat;
use crate::state::lsp::InlayHintConfig;
use crate::state::state::Transaction;

/// Offer the insertable inlay hints that overlap `selection` as quick fixes.
///
/// Each action applies that hint's existing edits: the annotation at the hint
/// position, plus any imports the annotation needs. Call-argument name hints
/// are not type annotations, so they are not requested.
pub(crate) fn insert_inferred_annotation_code_actions(
    transaction: &Transaction<'_>,
    handle: &Handle,
    selection: TextRange,
    import_format: ImportFormat,
    mut inlay_hint_config: InlayHintConfig,
) -> Option<Vec<(String, Vec<(Module, TextRange, String)>)>> {
    if !inlay_hint_config.variable_types && !inlay_hint_config.function_return_types {
        return None;
    }
    inlay_hint_config.call_argument_names = AllOffPartial::Off;
    let hints =
        transaction.inlay_hints_in_range(handle, inlay_hint_config, import_format, selection)?;
    let module = transaction.get_module_info(handle)?;
    let mut actions = Vec::new();
    for hint in hints {
        let Some(edits) = hint.edits else {
            continue;
        };
        let title = format!("Insert inferred type `{}`", edits.annotation.trim());
        let mut action_edits = Vec::with_capacity(1 + edits.imports.len());
        action_edits.push((
            module.dupe(),
            TextRange::empty(hint.position),
            edits.annotation,
        ));
        for (offset, import_text) in edits.imports {
            action_edits.push((module.dupe(), TextRange::empty(offset), import_text));
        }
        actions.push((hint.position, title, action_edits));
    }
    if actions.is_empty() {
        return None;
    }
    actions.sort_by_key(|(position, _, _)| *position);
    Some(
        actions
            .into_iter()
            .map(|(_, title, edits)| (title, edits))
            .collect(),
    )
}
