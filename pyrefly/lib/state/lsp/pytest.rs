/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashSet;
use std::iter::once;
use std::iter::once_with;

use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_name::ModuleNameWithKind;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::symbol_kind::SymbolKind;
use ruff_python_ast::AnyNodeRef;
use ruff_python_ast::Identifier;
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;
use vec1::Vec1;

use super::DefinitionMetadata;
use super::FindDefinitionItemWithDocstring;
use super::FindPreference;
use crate::config::config::ConfigSource;
use crate::state::pytest::find_pytest_fixture_definitions_for_parameter;
use crate::state::pytest::find_pytest_fixture_parameter_references;
use crate::state::pytest::is_directly_parametrized;
use crate::state::pytest::is_pytest_fixture_parameter_context;
use crate::state::state::Transaction;

pub(crate) fn pytest_conftest_handles(
    transaction: &Transaction<'_>,
    handle: &Handle,
    preference: FindPreference,
) -> Vec<Handle> {
    let module_path = handle.path();
    let Some(mut dir) = module_path.as_path().parent() else {
        return Vec::new();
    };
    let config = transaction.get_config(handle);
    let root = config
        .as_ref()
        .and_then(|config| match &config.source {
            ConfigSource::Synthetic(root) => root.as_deref(),
            source => source.root_from_file(),
        })
        .filter(|root| dir.starts_with(root));
    let filenames = if preference.prefer_pyi {
        ["conftest.pyi", "conftest.py"]
    } else {
        ["conftest.py", "conftest.pyi"]
    };
    let mut handles = Vec::new();
    loop {
        for filename in filenames {
            let path = dir.join(filename);
            let memory_path = ModulePath::memory(path.clone());
            let memory_config = transaction.config_finder().python_file(
                ModuleNameWithKind::guaranteed(ModuleName::unknown()),
                &memory_path,
            );
            let memory_handle = memory_config.handle_from_module_path(memory_path);
            if transaction.get_module_info(&memory_handle).is_some() {
                handles.push(memory_handle);
            } else if path.exists() {
                let filesystem_path = ModulePath::filesystem(path);
                let filesystem_config = transaction.config_finder().python_file(
                    ModuleNameWithKind::guaranteed(ModuleName::unknown()),
                    &filesystem_path,
                );
                handles.push(filesystem_config.handle_from_module_path(filesystem_path));
            }
        }
        if Some(dir) == root {
            break;
        }
        let Some(parent) = dir.parent() else {
            break;
        };
        dir = parent;
    }
    handles
}

impl<'a> Transaction<'a> {
    /// Resolve a pytest fixture parameter to the fixture functions that can provide it.
    ///
    /// This runs during definition lookup. The common non-pytest path is cheap because we first
    /// require either a pytest fixture function or a test-named function.
    pub(super) fn pytest_fixture_definitions_for_parameter(
        &self,
        handle: &Handle,
        identifier: &Identifier,
        covering_nodes: &[AnyNodeRef],
        preference: FindPreference,
    ) -> Option<Vec1<FindDefinitionItemWithDocstring>> {
        let mod_module = self.get_ast(handle)?;
        let answers = self.get_answers(handle)?;
        let bindings = answers.bindings();
        if !is_pytest_fixture_parameter_context(bindings, covering_nodes, identifier)
            || is_directly_parametrized(&mod_module, covering_nodes, identifier.id())
        {
            return None;
        }
        let enclosing_function_range = covering_nodes
            .iter()
            .find_map(|node| match node {
                AnyNodeRef::StmtFunctionDef(function) => Some(function.name.range),
                _ => None,
            })
            .expect("pytest parameter context requires an enclosing function");
        let matches = find_pytest_fixture_definitions_for_parameter(
            mod_module.as_ref(),
            bindings,
            identifier,
            covering_nodes,
        );
        let module_info = self.get_module_info(handle)?;
        let mut definitions: Vec<_> = matches
            .into_iter()
            .filter(|fixture| {
                !preference.follow_pytest_conftest || fixture.range != enclosing_function_range
            })
            .map(|fixture| FindDefinitionItemWithDocstring {
                metadata: DefinitionMetadata::Variable(Some(SymbolKind::Function)),
                definition_range: fixture.range,
                module: module_info.clone(),
                docstring_range: fixture.docstring_range,
                display_name: Some(fixture.name.as_str().to_owned()),
            })
            .collect();
        if definitions.is_empty() && preference.follow_pytest_conftest {
            let requesting_fixture_name = self
                .get_exports_data(handle)
                .pytest_fixtures()
                .iter()
                .find(|fixture| {
                    fixture.range == enclosing_function_range
                        && fixture.fixture_name.as_ref().unwrap_or(&fixture.name) == identifier.id()
                })
                .map(|fixture| fixture.name.clone());
            let current_path = handle.path().as_path();
            // Module fixtures take precedence; discover conftests only after a local miss.
            let candidates = once(handle.clone())
                .chain(once_with(|| pytest_conftest_handles(self, handle, preference)).flatten());
            'fixtures: for fixture_handle in candidates {
                let mut target_handle = fixture_handle;
                let mut target_name = identifier.id.clone();
                let mut imported = false;
                let mut seen = HashSet::new();
                while seen.insert((target_handle.clone(), target_name.clone())) {
                    let exports = self.get_exports_data(&target_handle);
                    if let Some(fixture) = exports.pytest_fixtures().iter().find(|fixture| {
                        let target_path = target_handle.path().as_path();
                        let is_same_definition = fixture.range == enclosing_function_range
                            && target_path == current_path;
                        // The implementation and its stub denote the same module-level provider.
                        let is_paired_definition = requesting_fixture_name.as_ref()
                            == Some(&fixture.name)
                            && target_handle.path().style() != handle.path().style()
                            && target_path.parent() == current_path.parent()
                            && target_path.file_stem() == current_path.file_stem();
                        if is_same_definition || is_paired_definition {
                            return false;
                        }
                        if imported {
                            fixture.name == target_name
                                && fixture
                                    .fixture_name
                                    .as_ref()
                                    .is_none_or(|name| name == identifier.id())
                        } else {
                            fixture.fixture_name.as_ref().unwrap_or(&fixture.name)
                                == identifier.id()
                        }
                    }) {
                        let module = self
                            .get_module_info(&target_handle)
                            .expect("fixture module must be loaded after demanding export data");
                        definitions.push(FindDefinitionItemWithDocstring {
                            metadata: DefinitionMetadata::Variable(Some(SymbolKind::Function)),
                            definition_range: fixture.range,
                            module,
                            docstring_range: fixture.docstring_range,
                            display_name: Some(fixture.name.as_str().to_owned()),
                        });
                        break 'fixtures;
                    }
                    let Some((module, name)) = exports.pytest_fixture_import(&target_name) else {
                        break;
                    };
                    let Some(import_handle) =
                        self.import_handle_with_preference(&target_handle, module, preference)
                    else {
                        break;
                    };
                    target_handle = import_handle;
                    target_name = name;
                    imported = true;
                }
            }
        }
        Vec1::try_from_vec(definitions).ok()
    }

    /// Find local pytest test/fixture parameters that reference a fixture definition.
    ///
    /// This is only used after the regular reference path has found a definition. Modules without
    /// pytest metadata return before walking the AST.
    pub(super) fn local_pytest_fixture_parameter_references(
        &self,
        handle: &Handle,
        definition_range: TextRange,
        expected_name: &Name,
    ) -> Option<Vec<TextRange>> {
        let mod_module = self.get_ast(handle)?;
        let answers = self.get_answers(handle)?;
        let bindings = answers.bindings();
        find_pytest_fixture_parameter_references(
            mod_module.as_ref(),
            bindings,
            definition_range,
            expected_name,
        )
    }
}
