/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Render annotations as source text for a module, with the imports they need.

use std::sync::Arc;

use dupe::Dupe;
use pyrefly_build::handle::Handle;
use pyrefly_types::stdlib::Stdlib;
use pyrefly_types::type_output::AnnotationPart;
use pyrefly_types::types::Type;
use ruff_python_ast::ModModule;
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use starlark_map::small_map::SmallMap;

use crate::state::ide::import_module_name;
use crate::state::ide::import_regular_import_edit;
use crate::state::ide::insert_import_edit;
use crate::state::ide::prepare_import_insertion;
use crate::state::import_tracker::ImportTracker;
use crate::state::lsp::ImportFormat;
use crate::state::state::Transaction;

struct DirectImport {
    handle: Handle,
    heads: Vec<String>,
}

/// Source text for annotations, with the imports the text needs.
pub(crate) struct RenderedAnnotation {
    pub text: String,
    /// Names the text uses unqualified, which need `from <module> import <name>`.
    names: Vec<DirectImport>,
    /// Modules the text references by qualified name, which need `import <module>`.
    modules: Vec<Handle>,
}

/// Renders annotations as source text for one module, planning the imports they need.
pub(crate) struct AnnotationRenderer<'a, 'state> {
    transaction: &'a Transaction<'state>,
    handle: &'a Handle,
    tracker: ImportTracker,
    ast: Arc<ModModule>,
    stdlib: &'a Stdlib,
    import_format: ImportFormat,
}

impl<'a, 'state> AnnotationRenderer<'a, 'state> {
    pub(crate) fn new(
        transaction: &'a Transaction<'state>,
        handle: &'a Handle,
        ast: Arc<ModModule>,
        stdlib: &'a Stdlib,
        import_format: ImportFormat,
    ) -> Self {
        Self {
            transaction,
            handle,
            tracker: ImportTracker::from_ast(
                ast.as_ref(),
                handle.module(),
                handle.path().is_init(),
                *handle.sys_info(),
            ),
            ast,
            stdlib,
            import_format,
        }
    }

    pub(crate) fn render(&self, ty: &Type) -> Option<RenderedAnnotation> {
        self.render_parts(&ty.get_annotation_parts(Some(self.stdlib)))
    }

    /// Returns `None` when a reference cannot be rendered unambiguously.
    pub(crate) fn render_parts(&self, parts: &[AnnotationPart]) -> Option<RenderedAnnotation> {
        let mut direct = Vec::<DirectImport>::new();
        for (module, head) in self
            .tracker
            .direct_import_candidates(parts, self.handle.module())
        {
            let Some(module_handle) = self
                .transaction
                .import_handle(self.handle, module, None)
                .finding()
            else {
                continue;
            };
            let name = Name::new(&head);
            if !self
                .transaction
                .get_exports(&module_handle)
                .contains_key(&name)
                || self
                    .transaction
                    .builtin_module_for_name(self.handle, &name)
                    .is_some()
            {
                continue;
            }
            match direct
                .iter_mut()
                .find(|import| import.handle.module() == module)
            {
                Some(import) => import.heads.push(head),
                None => direct.push(DirectImport {
                    handle: module_handle,
                    heads: vec![head],
                }),
            }
        }
        direct.sort_by(|left, right| {
            left.handle
                .module()
                .as_str()
                .cmp(right.handle.module().as_str())
        });
        for import in &mut direct {
            import.heads.sort();
        }

        let direct_imports = direct
            .iter()
            .flat_map(|import| {
                import
                    .heads
                    .iter()
                    .map(|head| (import.handle.module(), head.as_str()))
            })
            .collect::<Vec<_>>();
        let (text, missing) =
            self.tracker
                .resolve_annotation(parts, self.handle.module(), &direct_imports)?;

        let mut missing = missing.into_iter().collect::<Vec<_>>();
        missing.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        let modules = missing
            .into_iter()
            .map(|module| {
                self.transaction
                    .import_handle(self.handle, module, None)
                    .finding()
            })
            .collect::<Option<Vec<_>>>()?;
        Some(RenderedAnnotation {
            text,
            names: direct,
            modules,
        })
    }

    /// The imports that `rendered` needs, as complete lines inserted together.
    pub(crate) fn import_lines(&self, rendered: &RenderedAnnotation) -> Option<(TextSize, String)> {
        let mut text = String::new();
        for handle in &rendered.modules {
            text.push_str(&format!("import {}\n", handle.module()));
        }
        for import in &rendered.names {
            let module = import_module_name(
                self.transaction.config_finder(),
                self.handle,
                &import.handle,
                self.import_format,
            );
            text.push_str(&format!(
                "from {module} import {}\n",
                import.heads.join(", ")
            ));
        }
        (!text.is_empty()).then(|| prepare_import_insertion(self.ast.as_ref(), text))
    }

    /// The imports that `rendered` needs, extending existing `from` imports where possible.
    /// Edits at the same range are combined, so their order is explicit.
    pub(crate) fn import_edits(&self, rendered: &RenderedAnnotation) -> Vec<(TextRange, String)> {
        let mut edits = SmallMap::<TextRange, String>::new();
        for import in &rendered.names {
            for head in &import.heads {
                let edit = insert_import_edit(
                    self.ast.as_ref(),
                    self.transaction.config_finder(),
                    self.handle.dupe(),
                    import.handle.dupe(),
                    head,
                    self.import_format,
                );
                edits
                    .entry(edit.range)
                    .or_default()
                    .push_str(&edit.insert_text);
            }
        }
        for handle in &rendered.modules {
            let (position, text, _) =
                import_regular_import_edit(self.ast.as_ref(), handle.dupe(), None);
            edits
                .entry(TextRange::empty(position))
                .or_default()
                .push_str(&text);
        }
        edits
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .collect()
    }
}
