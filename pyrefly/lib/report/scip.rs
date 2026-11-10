/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;
use std::io::Write;
use std::iter;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use dupe::Dupe;
use lsp_types::Contents;
use lsp_types::Hover;
use lsp_types::Uri;
use protobuf::Message as _;
use pyrefly_build::handle::Handle;
use pyrefly_python::ast::Ast;
use pyrefly_python::docstring::Docstring;
use pyrefly_python::module::Module;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePathDetails;
use pyrefly_python::symbol_kind::SymbolKind;
use pyrefly_python::sys_info::SysInfo;
use pyrefly_util::absolutize::Absolutize;
use pyrefly_util::fs_anyhow;
use pyrefly_util::lined_buffer::DisplayPos;
use pyrefly_util::lined_buffer::DisplayRange;
use pyrefly_util::lock::Mutex;
use pyrefly_util::unix_path::path_to_unix_string;
use pyrefly_util::visit::Visit;
use rayon::prelude::*;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Identifier;
use ruff_python_ast::ModModule;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_python_ast::name::Name;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use scip::symbol::format_symbol;
use scip::types::Descriptor;
use scip::types::Document;
use scip::types::Index;
use scip::types::Metadata;
use scip::types::Occurrence;
use scip::types::PositionEncoding;
use scip::types::Relationship;
use scip::types::Symbol;
use scip::types::SymbolInformation;
use scip::types::SymbolRole;
use scip::types::TextEncoding;
use scip::types::ToolInfo;
use scip::types::descriptor::Suffix;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use tracing::error;

use crate::config::config::ConfigFile;
use crate::lsp::wasm::hover::HoverOptions;
use crate::lsp::wasm::hover::HoverResult;
use crate::lsp::wasm::hover::get_hover_with_verbosity;
use crate::report::scip::symbols::ScipSymbol;
use crate::report::scip::symbols::definitions;
use crate::report::scip::symbols::descriptor;
use crate::state::lsp::CalleeKind;
use crate::state::lsp::FindDefinitionItemWithDocstring;
use crate::state::lsp::FindPreference;
use crate::state::lsp::callee_kind_from_call;
use crate::state::require::Require;
use crate::state::state::Transaction;
use crate::types::globals::ImplicitGlobal;

#[cfg(test)]
mod snapshot;
mod symbols;

const SCHEME: &str = "scip-python";
const MANAGER: &str = "python";

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
}

impl Package {
    /// The package of the project's own symbols. Explicit values come first, then `[project]`
    /// and `[tool.poetry]` in the project root's `pyproject.toml`.
    pub fn project(
        project_root: &Path,
        name: Option<String>,
        version: Option<String>,
    ) -> anyhow::Result<Self> {
        let pyproject = project_root.join(ConfigFile::PYPROJECT_FILE_NAME);
        let pyproject: toml::Table = if (name.is_none() || version.is_none()) && pyproject.exists()
        {
            fs_anyhow::read_to_string(&pyproject)?.parse()?
        } else {
            toml::Table::new()
        };
        let lookup = |key: &str| {
            let poetry = pyproject.get("tool").and_then(|tool| tool.get("poetry"));
            [pyproject.get("project"), poetry]
                .into_iter()
                .flatten()
                .find_map(|table| Some(table.get(key)?.as_str()?.to_owned()))
        };
        // An empty name or version is written as `.` in symbols.
        Ok(Self {
            name: name.or_else(|| lookup("name")).unwrap_or_default(),
            version: version.or_else(|| lookup("version")).unwrap_or_default(),
        })
    }
}

/// Writes a SCIP index as a stream of `Index` messages, which a reader merges into one: the
/// metadata, then each document as soon as it is built, in no particular order, then the
/// external symbols. Documents can be written while the files are being checked, from a
/// solutions hook.
pub struct ScipWriter<W> {
    writer: Mutex<W>,
    handles: SmallSet<Handle>,
    project_root: PathBuf,
    project: Package,
    definitions: Mutex<HashMap<Handle, Arc<SmallMap<TextRange, ScipSymbol>>>>,
    asts: Mutex<HashMap<Handle, Arc<ModModule>>>,
    distributions: Mutex<HashMap<PathBuf, Arc<HashMap<Name, Package>>>>,
    external_symbols: Mutex<HashMap<String, ExternalUse>>,
}

impl<W: Write + Send> ScipWriter<W> {
    pub fn new(
        mut writer: W,
        handles: &[Handle],
        project_root: &Path,
        project: Package,
        version: &str,
    ) -> anyhow::Result<Self> {
        let project_root = project_root.absolutize();
        Index {
            metadata: Some(Metadata {
                project_root: Uri::from_directory_path(&project_root)
                    .map_err(|_| {
                        anyhow!(
                            "cannot represent SCIP project root `{}` as a file URI",
                            project_root.display()
                        )
                    })?
                    .to_string(),
                tool_info: Some(ToolInfo {
                    name: "scip-pyrefly".to_owned(),
                    version: version.to_owned(),
                    ..Default::default()
                })
                .into(),
                text_document_encoding: TextEncoding::UTF8.into(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        }
        .write_to_writer(&mut writer)?;
        Ok(Self {
            writer: Mutex::new(writer),
            handles: handles.iter().cloned().collect(),
            project_root,
            project,
            definitions: Mutex::new(HashMap::new()),
            asts: Mutex::new(HashMap::new()),
            distributions: Mutex::new(HashMap::new()),
            external_symbols: Mutex::new(HashMap::new()),
        })
    }

    /// Writes documents from the solutions hook and releases their answers afterwards.
    pub fn start<'a>(self, transaction: &mut Transaction<'a>) -> Arc<Self>
    where
        W: 'a,
    {
        let scip = Arc::new(self);
        let hook = scip.dupe();
        transaction.set_solutions_hook(Some(Box::new(move |handle, transaction| {
            if let Err(e) = hook.write_document(transaction, handle) {
                error!(
                    "while writing SCIP document for `{}`: {e:#}",
                    handle.module()
                );
            }
        })));
        transaction.set_evict_answers_after_scip_hook(true);
        scip
    }

    fn indexer<'a>(&'a self, transaction: &'a Transaction<'a>) -> Indexer<'a> {
        Indexer {
            transaction,
            project_root: &self.project_root,
            project: &self.project,
            definitions: &self.definitions,
            asts: &self.asts,
            distributions: &self.distributions,
            external_symbols: &self.external_symbols,
        }
    }

    /// Writes the document of `handle`, if it is one of the indexed files.
    pub fn write_document(&self, transaction: &Transaction, handle: &Handle) -> anyhow::Result<()> {
        if !self.handles.contains(handle) {
            return Ok(());
        }
        let Some(document) = self.indexer(transaction).document(handle) else {
            return Ok(());
        };
        let bytes = Index {
            documents: vec![document],
            ..Default::default()
        }
        .write_to_bytes()?;
        self.writer.lock().write_all(&bytes)?;
        Ok(())
    }

    /// Detaches the solutions hook and writes external symbols after every document is written.
    pub fn finish(self: Arc<Self>, transaction: &mut Transaction) -> anyhow::Result<W> {
        transaction.set_solutions_hook(None);
        transaction.set_evict_answers_after_scip_hook(false);
        let scip = Arc::into_inner(self).expect("the solutions hook holds the only other copy");
        let Self {
            writer,
            external_symbols,
            definitions,
            asts,
            distributions,
            ..
        } = scip;
        drop((definitions, asts, distributions));
        let mut writer = writer.into_inner();
        let mut first_uses: Vec<(String, ExternalUse)> =
            external_symbols.into_inner().into_iter().collect();
        // Most external symbols are documented at their first use. When that hover has no
        // documentation, retain the defining module's answers to document the symbol there.
        let dependencies: Vec<Handle> = first_uses
            .iter()
            .filter(|(_, first_use)| first_use.documentation.is_empty())
            .filter_map(|(_, first_use)| {
                first_use
                    .definition
                    .as_ref()
                    .map(|(handle, _)| handle.dupe())
            })
            .collect::<SmallSet<_>>()
            .into_iter()
            .collect();
        transaction.run(&dependencies, Require::Everything, None);
        let transaction = &*transaction;
        transaction.install(|| {
            first_uses.par_iter_mut().for_each(|(_, first_use)| {
                if first_use.documentation.is_empty()
                    && let Some((handle, position)) = &first_use.definition
                {
                    first_use.documentation = documentation(transaction, handle, *position);
                }
            })
        });
        let mut external_symbols: Vec<SymbolInformation> = first_uses
            .into_iter()
            .filter(|(_, first_use)| !first_use.documentation.is_empty())
            .map(|(symbol, first_use)| SymbolInformation {
                symbol,
                documentation: first_use.documentation,
                ..Default::default()
            })
            .collect();
        external_symbols.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        Index {
            external_symbols,
            ..Default::default()
        }
        .write_to_writer(&mut writer)?;
        Ok(writer)
    }
}

/// Top-level names whose kind in the bundled stubs differs from how other indexers name them:
/// `Any` was a variable in older stubs, and `NotImplemented` and `Ellipsis` are named as types.
const SPECIAL_FORMS: &[(&str, &str, Suffix)] = &[
    ("typing", "Any", Suffix::Term),
    ("builtins", "NotImplemented", Suffix::Type),
    ("builtins", "Ellipsis", Suffix::Type),
];

struct Indexer<'a> {
    transaction: &'a Transaction<'a>,
    project_root: &'a Path,
    project: &'a Package,
    /// The definitions of every module seen so far, keyed by the range of the defining identifier.
    definitions: &'a Mutex<HashMap<Handle, Arc<SmallMap<TextRange, ScipSymbol>>>>,
    /// The parsed AST of every dependency seen so far, since only the indexed files keep theirs.
    asts: &'a Mutex<HashMap<Handle, Arc<ModModule>>>,
    /// The installed distributions of every site-packages directory seen so far.
    distributions: &'a Mutex<HashMap<PathBuf, Arc<HashMap<Name, Package>>>>,
    /// The first use, in path and position order, of every symbol defined outside the project and
    /// referenced so far. The symbol is documented with the hover there, so its documentation
    /// doesn't depend on the order documents are built in.
    external_symbols: &'a Mutex<HashMap<String, ExternalUse>>,
}

/// For a symbol defined outside the project, where to document it when the hover at its use has
/// nothing.
type External = Option<Option<(Handle, TextSize)>>;

struct ExternalUse {
    relative_path: PathBuf,
    position: TextSize,
    documentation: Vec<String>,
    /// Where the symbol is defined, to document it there when the hover at the use has nothing.
    definition: Option<(Handle, TextSize)>,
}

impl Indexer<'_> {
    fn document(&self, handle: &Handle) -> Option<Document> {
        let module = self.transaction.get_module_info(handle)?;
        let relative_path = module
            .path()
            .as_path()
            .strip_prefix(self.project_root)
            .ok()?;
        let ast = self.transaction.get_ast(handle)?;
        let definitions = self.definitions(&module, handle.sys_info().dupe())?;
        let mut occurrences = vec![Occurrence {
            range: vec![0, 0, 0],
            symbol: module_symbol(self.project, module.name()),
            symbol_roles: SymbolRole::Definition as i32,
            ..Default::default()
        }];
        let mut enclosing_ranges = SmallMap::new();
        ast.body
            .iter()
            .for_each(|x| collect_enclosing_ranges(x, &mut enclosing_ranges));
        // Only the first binding of a symbol defines it, and later ones refer to it. A parameter
        // is defined again by each overload.
        let mut parameters = SmallSet::new();
        ast.body
            .iter()
            .for_each(|x| collect_parameters(x, &mut parameters));
        let mut defined = SmallSet::new();
        occurrences.extend(definitions.iter().map(|(range, symbol)| {
            let symbol = format_scip_symbol(self.project, symbol);
            let is_definition = defined.insert(symbol.clone()) || parameters.contains(range);
            Occurrence {
                range: scip_range(&module, *range),
                symbol,
                symbol_roles: if is_definition {
                    SymbolRole::Definition as i32
                } else {
                    0
                },
                enclosing_range: enclosing_ranges
                    .get(range)
                    .filter(|_| is_definition)
                    .map_or_else(Vec::new, |x| scip_range(&module, *x)),
                ..Default::default()
            }
        }));

        // Names that resolve to no definition get one local per name, numbered after the
        // module's own locals.
        let first_unresolved = definitions
            .values()
            .filter_map(|symbol| match symbol {
                ScipSymbol::Local(id) => Some(id + 1),
                ScipSymbol::Global(_) => None,
            })
            .max()
            .unwrap_or(0);
        let mut unresolved: SmallMap<Name, usize> = SmallMap::new();
        let mut uses = Vec::new();
        ast.visit(&mut |x| collect_expr_uses(x, &mut uses));
        ast.body.iter().for_each(|x| collect_imports(x, &mut uses));
        // The first `self.x = ...` of an attribute is its definition, not a use.
        uses.retain(|x| !definitions.contains_key(&x.range));
        uses.sort_by_key(|x| x.range.start());
        for x in uses {
            let symbol = match (self.reference(handle, &module, &x), &x.kind) {
                (Some((symbol, external)), _) => {
                    if let Some(definition) = external {
                        let position = use_position(&x);
                        let is_first = |external_symbols: &HashMap<String, ExternalUse>| {
                            external_symbols.get(&symbol).is_none_or(|first_use| {
                                (relative_path, position)
                                    < (first_use.relative_path.as_path(), first_use.position)
                            })
                        };
                        // Documented here, while this file's answers are still in memory.
                        if is_first(&self.external_symbols.lock()) {
                            let documentation = documentation(self.transaction, handle, position);
                            let mut external_symbols = self.external_symbols.lock();
                            if is_first(&external_symbols) {
                                external_symbols.insert(
                                    symbol.clone(),
                                    ExternalUse {
                                        relative_path: relative_path.to_owned(),
                                        position,
                                        documentation,
                                        definition,
                                    },
                                );
                            }
                        }
                    }
                    symbol
                }
                (None, UseKind::Name | UseKind::Import) => {
                    let id = first_unresolved + unresolved.len();
                    format!("local {}", unresolved.entry(x.identifier.id).or_insert(id))
                }
                // An unresolved attribute or keyword isn't a variable, so it gets no local.
                (None, UseKind::Attribute | UseKind::Keyword(_)) => continue,
            };
            occurrences.push(Occurrence {
                range: scip_range(&module, x.range),
                symbol,
                ..Default::default()
            });
        }
        // A symbol defined more than once is documented at its first definition.
        let mut symbols: SmallMap<String, SymbolInformation> = SmallMap::new();
        let mut module_documentation = format!("```python\n(module) {}\n```", module.name());
        if let Some(range) = Docstring::range_from_stmts(&ast.body) {
            module_documentation.push_str("\n---\n");
            module_documentation.push_str(Docstring(range, module.dupe()).resolve().trim());
        }
        let module_symbol = module_symbol(self.project, module.name());
        symbols.insert(
            module_symbol.clone(),
            SymbolInformation {
                symbol: module_symbol,
                documentation: vec![module_documentation],
                ..Default::default()
            },
        );
        for (range, symbol) in definitions.iter() {
            let symbol = format_scip_symbol(self.project, symbol);
            if !symbols.contains_key(&symbol) {
                let documentation = documentation(self.transaction, handle, range.start());
                symbols.insert(
                    symbol.clone(),
                    SymbolInformation {
                        symbol,
                        documentation,
                        ..Default::default()
                    },
                );
            }
        }
        let mut classes = Vec::new();
        ast.body
            .iter()
            .for_each(|x| collect_classes(x, &mut classes));
        for class in classes {
            for (range, implemented) in self.implementations(handle, &module, class) {
                let Some(symbol) = definitions.get(&range) else {
                    continue;
                };
                if let Some(info) = symbols.get_mut(&format_scip_symbol(self.project, symbol)) {
                    info.relationships = implemented
                        .into_iter()
                        .map(|symbol| Relationship {
                            symbol,
                            is_implementation: true,
                            ..Default::default()
                        })
                        .collect();
                }
            }
        }
        let symbols = symbols
            .into_values()
            .filter(|info| !info.documentation.is_empty() || !info.relationships.is_empty())
            .collect();
        Some(Document {
            language: "python".to_owned(),
            relative_path: path_to_unix_string(relative_path),
            position_encoding: PositionEncoding::UTF32CodeUnitOffsetFromLineStart.into(),
            occurrences,
            symbols,
            ..Default::default()
        })
    }

    /// The symbols that a class and its methods implement, keyed by the range of the defining
    /// name. A class implements each of its bases, and a method implements the first method of
    /// the same name found through the bases, in order. A method whose name starts with `__`
    /// implements nothing.
    fn implementations(
        &self,
        handle: &Handle,
        module: &Module,
        class: &StmtClassDef,
    ) -> Vec<(TextRange, Vec<String>)> {
        let bases: Vec<Use> = class
            .bases()
            .iter()
            .filter_map(|base| {
                let base = match base {
                    Expr::Subscript(x) => &*x.value,
                    x => x,
                };
                let (identifier, kind) = match base {
                    Expr::Name(x) => (Ast::expr_name_identifier(x.clone()), UseKind::Name),
                    Expr::Attribute(x) => (x.attr.clone(), UseKind::Attribute),
                    _ => return None,
                };
                Some(Use {
                    range: base.range(),
                    identifier,
                    kind,
                })
            })
            .collect();
        let class_bases: Vec<_> = bases
            .iter()
            .filter_map(|base| {
                self.reference(handle, module, base)
                    .map(|(symbol, _)| symbol)
            })
            .collect();
        let mut implementations = vec![(class.name.range, class_bases)];
        for x in &class.body {
            let Stmt::FunctionDef(method) = x else {
                continue;
            };
            let name = &method.name.id;
            if name.starts_with("__") {
                continue;
            }
            let implemented = bases.iter().find_map(|base| {
                let definitions = self
                    .transaction
                    .find_definition_for_attribute(handle, base.range, name, preference())
                    .ok()?;
                let definition = definitions.into_iter().next()?;
                self.symbol(handle, module, definition)
                    .map(|(symbol, _)| symbol)
            });
            implementations.push((method.name.range, implemented.into_iter().collect()));
        }
        implementations.retain(|(_, implemented)| !implemented.is_empty());
        implementations
    }

    /// The symbol a use refers to, and where it is defined if that is outside the project, or
    /// `None` if it has no definition we can name.
    fn reference(&self, handle: &Handle, module: &Module, x: &Use) -> Option<(String, External)> {
        let preference = preference();
        let definition = match &x.kind {
            UseKind::Name => {
                let definition = self
                    .transaction
                    .find_definition_for_name_use(handle, &x.identifier, preference)
                    .ok()
                    .flatten();
                // Implicit module globals like `__name__` have no definition, so they are named
                // as stdlib builtins.
                if definition.is_none()
                    && ImplicitGlobal::implicit_globals(false).any(|g| *g.name() == x.identifier.id)
                {
                    let descriptors = vec![
                        descriptor("builtins", Suffix::Namespace),
                        descriptor(&x.identifier.id, Suffix::Type),
                    ];
                    return Some((
                        global_symbol(&stdlib_package(handle), descriptors),
                        Some(None),
                    ));
                }
                definition
            }
            UseKind::Keyword(callee_kind) => {
                // Dependencies keep no AST, so the callee's module comes from our cache rather
                // than being parsed again for every keyword.
                let definition = self
                    .transaction
                    .find_definition_for_keyword_argument_with_ast(
                        handle,
                        &x.identifier,
                        callee_kind,
                        preference,
                        |module| {
                            let defining = Handle::new(
                                module.name(),
                                module.path().dupe(),
                                handle.sys_info().dupe(),
                            );
                            self.ast(module, &defining)
                        },
                    )
                    .into_iter()
                    .next()?;
                Some(FindDefinitionItemWithDocstring {
                    metadata: definition.metadata,
                    definition_range: definition.definition_range,
                    module: definition.module,
                    docstring_range: None,
                    display_name: Some(x.identifier.id.to_string()),
                })
            }
            UseKind::Import | UseKind::Attribute => {
                let definitions = self
                    .transaction
                    .find_definition(handle, use_position(x), preference)
                    .ok()?;
                definitions.into_iter().next()
            }
        }?;
        self.symbol(handle, module, definition)
    }

    /// The symbol of a definition, and whether it is defined outside the project, or `None` if
    /// it is a local of another module or has no definition we can name.
    fn symbol(
        &self,
        handle: &Handle,
        module: &Module,
        definition: FindDefinitionItemWithDocstring,
    ) -> Option<(String, External)> {
        let external = (!self.in_project(&definition.module, handle)).then(|| {
            let defining = Handle::new(
                definition.module.name(),
                definition.module.path().dupe(),
                handle.sys_info().dupe(),
            );
            Some((defining, definition.definition_range.start()))
        });
        let package = self.package(&definition.module, handle);
        // A submodule accessed as an attribute resolves to the empty range at the module's start.
        if definition.metadata.symbol_kind() == Some(SymbolKind::Module)
            || definition.definition_range.is_empty()
        {
            return Some((
                module_symbol(&package, definition.module.name()),
                external.map(|_| None),
            ));
        }
        let symbols = self.definitions(&definition.module, handle.sys_info().dupe())?;
        match symbols.get(&definition.definition_range)? {
            ScipSymbol::Global(descriptors) => {
                Some((global_symbol(&package, descriptors.clone()), external))
            }
            ScipSymbol::Local(id) => {
                (definition.module.path() == module.path()).then(|| (format!("local {id}"), None))
            }
        }
    }

    fn definitions(
        &self,
        module: &Module,
        sys_info: SysInfo,
    ) -> Option<Arc<SmallMap<TextRange, ScipSymbol>>> {
        let handle = Handle::new(module.name(), module.path().dupe(), sys_info);
        if let Some(definitions) = self.definitions.lock().get(&handle) {
            return Some(definitions.dupe());
        }
        let ast = self.ast(module, &handle);
        let mut table: SmallMap<_, _> = definitions(module.name(), &ast).into_iter().collect();
        for (module_name, name, suffix) in SPECIAL_FORMS {
            if module.name().as_str() != *module_name {
                continue;
            }
            for symbol in table.values_mut() {
                if let ScipSymbol::Global(descriptors) = symbol
                    && let [_, descriptor] = descriptors.as_mut_slice()
                    && descriptor.name == *name
                {
                    descriptor.suffix = (*suffix).into();
                }
            }
        }
        let table = Arc::new(table);
        self.definitions.lock().insert(handle, table.dupe());
        Some(table)
    }

    /// The AST of `module`, parsed again if it is a dependency.
    fn ast(&self, module: &Module, handle: &Handle) -> Arc<ModModule> {
        if let Some(ast) = self.transaction.get_ast(handle) {
            return ast;
        }
        if let Some(ast) = self.asts.lock().get(handle) {
            return ast.dupe();
        }
        let ast = Arc::new(Ast::parse(module.contents(), module.source_type()).0);
        self.asts.lock().insert(handle.clone(), ast.dupe());
        ast
    }

    /// Whether a module referenced from `handle` belongs to the project: it is under the project
    /// root, and not in a site-packages directory there, such as a virtual environment's.
    fn in_project(&self, module: &Module, handle: &Handle) -> bool {
        let path = module.path().as_path();
        path.starts_with(self.project_root)
            && !self
                .transaction
                .get_config(handle)
                .iter()
                .flat_map(|config| config.site_package_path())
                .any(|root| path.starts_with(root))
    }

    /// The package of a module referenced from `handle`, whose site-packages are searched for
    /// installed distributions.
    fn package(&self, module: &Module, handle: &Handle) -> Package {
        let bundled = matches!(
            module.path().details(),
            ModulePathDetails::BundledTypeshed(_)
        );
        if !bundled && self.in_project(module, handle) {
            return self.project.clone();
        }
        // Match by top-level module, so that bundled stubs get the installed package too, even for
        // a stdlib module like `typing_extensions`.
        let top_level = module.name().first_component();
        let config = self.transaction.get_config(handle);
        let installed = config
            .iter()
            .flat_map(|config| config.site_package_path())
            .find_map(|root| {
                let installed = self
                    .distributions
                    .lock()
                    .entry(root.clone())
                    .or_insert_with(|| Arc::new(distributions(root)))
                    .dupe();
                installed.get(&top_level).cloned()
            });
        installed.unwrap_or_else(|| {
            if bundled {
                stdlib_package(handle)
            } else {
                // Without package metadata, name the package after the top-level module.
                Package {
                    name: top_level.to_string(),
                    version: String::new(),
                }
            }
        })
    }
}

/// The hover text at a position, as SCIP documentation.
fn documentation(transaction: &Transaction, handle: &Handle, position: TextSize) -> Vec<String> {
    let options = HoverOptions {
        show_go_to_links: false,
        verbosity_level: 0,
    };
    match get_hover_with_verbosity(transaction, handle, position, options) {
        Some(HoverResult {
            hover:
                Hover {
                    contents: Contents::MarkupContent(markup),
                    ..
                },
            ..
        }) => vec![markup.value],
        _ => Vec::new(),
    }
}

fn stdlib_package(handle: &Handle) -> Package {
    let version = handle.sys_info().version();
    Package {
        name: "python-stdlib".to_owned(),
        version: format!("{}.{}", version.major, version.minor),
    }
}

/// The distributions installed in a site-packages directory, keyed by each top-level module they
/// installed. A distribution whose metadata can't be read is left out.
fn distributions(site_packages: &Path) -> HashMap<Name, Package> {
    let mut modules = HashMap::new();
    let pattern = site_packages.join("*.dist-info");
    for dist_info in glob::glob(&pattern.to_string_lossy())
        .into_iter()
        .flatten()
        .flatten()
    {
        let (Ok(metadata), Ok(record)) = (
            fs_anyhow::read_to_string(&dist_info.join("METADATA")),
            fs_anyhow::read_to_string(&dist_info.join("RECORD")),
        ) else {
            continue;
        };
        // The header fields end at the first blank line.
        let field = |key: &str| {
            metadata
                .lines()
                .take_while(|line| !line.is_empty())
                .find_map(|line| Some(line.strip_prefix(key)?.strip_prefix(':')?.trim().to_owned()))
        };
        let (Some(name), Some(version)) = (field("Name"), field("Version")) else {
            continue;
        };
        let package = Package { name, version };
        // Each RECORD line is a CSV row whose first column is the installed file. Its first path
        // component, up to the first `.`, names a top-level module (or a `-stubs` package), unless
        // it isn't an identifier, as for the `.dist-info` directory itself.
        for line in record.lines() {
            let component = line.split(['/', ',']).next().unwrap_or_default();
            let module = component
                .split('.')
                .next()
                .unwrap_or_default()
                .trim_end_matches("-stubs");
            if !module.is_empty() && module.chars().all(|c| c.is_alphanumeric() || c == '_') {
                modules
                    .entry(Name::new(module))
                    .or_insert_with(|| package.clone());
            }
        }
    }
    modules
}

/// A name that refers to a definition elsewhere.
struct Use {
    /// The range of the occurrence. For a relative module, it also covers the leading dots.
    range: TextRange,
    identifier: Identifier,
    kind: UseKind,
}

enum UseKind {
    /// A name in an expression.
    Name,
    /// A module or name in an import statement.
    Import,
    Attribute,
    /// The name of a keyword argument in a call to the callee.
    Keyword(CalleeKind),
}

/// Collects every name that is read or deleted, every attribute, and every keyword argument.
fn collect_expr_uses(x: &Expr, uses: &mut Vec<Use>) {
    let mut push = |identifier: Identifier, kind| {
        uses.push(Use {
            range: identifier.range,
            identifier,
            kind,
        })
    };
    match x {
        Expr::Name(name) if name.ctx != ExprContext::Store => {
            push(Ast::expr_name_identifier(name.clone()), UseKind::Name)
        }
        Expr::Attribute(attribute) => push(attribute.attr.clone(), UseKind::Attribute),
        Expr::Call(call) => {
            for keyword in call.arguments.keywords.iter() {
                if let Some(arg) = &keyword.arg {
                    push(arg.clone(), UseKind::Keyword(callee_kind_from_call(call)));
                }
            }
        }
        _ => {}
    }
    x.recurse(&mut |x| collect_expr_uses(x, uses));
}

/// The position at which a use is resolved. At the identifier's last character, a dotted module
/// name in an import resolves to the whole module rather than to its first component.
fn use_position(x: &Use) -> TextSize {
    x.identifier.range.end() - TextSize::from(1)
}

fn preference() -> FindPreference {
    FindPreference {
        // A call like `C()` refers to the class `C`, not to `C.__init__`.
        resolve_call_dunders: false,
        ..Default::default()
    }
}

/// Collects every class definition, including nested ones.
fn collect_classes<'a>(x: &'a Stmt, classes: &mut Vec<&'a StmtClassDef>) {
    if let Stmt::ClassDef(x) = x {
        classes.push(x);
    }
    x.recurse(&mut |x| collect_classes(x, classes));
}

fn collect_parameters(x: &Stmt, ranges: &mut SmallSet<TextRange>) {
    if let Stmt::FunctionDef(x) = x {
        ranges.extend(x.parameters.iter().map(|x| x.name().range));
    }
    x.recurse(&mut |x| collect_parameters(x, ranges));
}

/// Maps the name of every function and class to its whole definition, decorators included.
fn collect_enclosing_ranges(x: &Stmt, ranges: &mut SmallMap<TextRange, TextRange>) {
    match x {
        Stmt::FunctionDef(x) => ranges.insert(x.name.range, x.range),
        Stmt::ClassDef(x) => ranges.insert(x.name.range, x.range),
        _ => None,
    };
    x.recurse(&mut |x| collect_enclosing_ranges(x, ranges));
}

/// Collects the module and name identifiers of every import statement.
fn collect_imports(x: &Stmt, uses: &mut Vec<Use>) {
    let mut import = |range, identifier: &Identifier| {
        uses.push(Use {
            range,
            identifier: identifier.clone(),
            kind: UseKind::Import,
        })
    };
    let aliases = match x {
        Stmt::Import(x) => &x.names,
        Stmt::ImportFrom(x) => {
            if let Some(module) = &x.module {
                let dots = TextSize::from(x.level);
                import(TextRange::new(module.start() - dots, module.end()), module);
            }
            &x.names
        }
        _ => {
            x.recurse(&mut |x| collect_imports(x, uses));
            return;
        }
    };
    for alias in aliases.iter().filter(|alias| alias.name.as_str() != "*") {
        for identifier in iter::once(&alias.name).chain(&alias.asname) {
            import(identifier.range, identifier);
        }
    }
}

/// A SCIP range: `[line, start, end]` on one line, or `[start line, start, end line, end]`.
fn scip_range(module: &Module, range: TextRange) -> Vec<i32> {
    let DisplayRange { start, end } = module.display_range(range);
    let position = |pos: DisplayPos| {
        (
            pos.line_within_file().to_zero_indexed() as i32,
            pos.column().get() as i32 - 1,
        )
    };
    let ((start_line, start_column), (end_line, end_column)) = (position(start), position(end));
    if start_line == end_line {
        vec![start_line, start_column, end_column]
    } else {
        vec![start_line, start_column, end_line, end_column]
    }
}

fn format_scip_symbol(package: &Package, symbol: &ScipSymbol) -> String {
    match symbol {
        ScipSymbol::Global(descriptors) => global_symbol(package, descriptors.clone()),
        ScipSymbol::Local(id) => format!("local {id}"),
    }
}

fn global_symbol(package: &Package, descriptors: Vec<Descriptor>) -> String {
    // The SCIP spec escapes spaces in package fields by doubling them.
    let escape = |x: &str| x.replace(' ', "  ");
    format_symbol(Symbol {
        scheme: SCHEME.to_owned(),
        package: Some(scip::types::Package {
            manager: MANAGER.to_owned(),
            name: escape(&package.name),
            version: escape(&package.version),
            ..Default::default()
        })
        .into(),
        descriptors,
        ..Default::default()
    })
}

fn module_symbol(package: &Package, module: ModuleName) -> String {
    global_symbol(
        package,
        vec![
            descriptor(module.as_str(), Suffix::Namespace),
            descriptor("__init__", Suffix::Meta),
        ],
    )
}

#[cfg(test)]
mod tests {
    use std::slice;

    use pyrefly_python::module_path::ModulePath;
    use pyrefly_util::thread_pool::TEST_THREAD_COUNT;

    use super::*;
    use crate::report::scip::snapshot::format_snapshot;
    use crate::state::state::State;
    use crate::test::util::TestEnv;

    fn snapshots_dir() -> PathBuf {
        let path = std::env::var("SCIP_SNAPSHOTS_PATH")
            .expect("SCIP_SNAPSHOTS_PATH env var not set: buck should set this automatically");
        std::env::current_dir().unwrap().join(path)
    }

    /// A test case uses its own `pyproject.toml` when it has one.
    fn project_package(snapshots: &Path, case: &str) -> Package {
        let info: serde_json::Value = serde_json::from_str(
            &fs_anyhow::read_to_string(&snapshots.join("packageInfo.json")).unwrap(),
        )
        .unwrap();
        let input_dir = snapshots.join("input").join(case);
        let package = match info["special"].get(case) {
            Some(package) => Some(package),
            None if input_dir.join(ConfigFile::PYPROJECT_FILE_NAME).exists() => None,
            None => Some(&info["default"]),
        };
        let field = |key: &str| package.map(|package| package[key].as_str().unwrap().to_owned());
        Package::project(&input_dir, field("name"), field("version")).unwrap()
    }

    fn index(case: &str) -> Index {
        let snapshots = snapshots_dir();
        let input_dir = snapshots.join("input").join(case);
        // Case-local dependencies model a virtual environment inside the project root.
        let site_packages = input_dir.join("site-packages");
        let mut env = TestEnv::new();
        let mut handles = Vec::new();
        for path in glob::glob(&format!("{}/**/*.py", input_dir.display())).unwrap() {
            let path = path.unwrap();
            if path.starts_with(&site_packages) {
                continue;
            }
            // As with an import root, modules under `src/` are named without the prefix.
            let relative = path
                .strip_prefix(input_dir.join("src"))
                .or_else(|_| path.strip_prefix(&input_dir))
                .unwrap();
            let name = ModuleName::from_relative_path(relative).unwrap();
            env.add_real_path(name.as_str(), path.clone());
            handles.push(Handle::new(
                name,
                ModulePath::filesystem(path),
                env.sys_info(),
            ));
        }
        let env = env.with_site_package_paths(vec![site_packages, snapshots.join("site-packages")]);
        let state = State::new(env.config_finder(), TEST_THREAD_COUNT);
        let mut transaction = state.new_transaction(Require::Exports, None);
        let scip = ScipWriter::new(
            Vec::new(),
            &handles,
            &input_dir,
            project_package(&snapshots, case),
            "test",
        )
        .unwrap()
        .start(&mut transaction);
        transaction.run(&handles, Require::Everything, None);
        for handle in &handles {
            assert!(
                transaction.get_answers(handle).is_none(),
                "answers were not evicted"
            );
        }
        let bytes = scip.finish(&mut transaction).unwrap();
        let index = Index::parse_from_bytes(&bytes).unwrap();
        assert_eq!(
            index.documents.len(),
            handles.len(),
            "missing snapshot documents"
        );
        index
    }

    fn assert_snapshot(case: &str) {
        let snapshots = snapshots_dir();
        let input_dir = snapshots.join("input").join(case);
        let output_dir = snapshots.join("output").join(case);
        let index = index(case);
        let update = std::env::var("SCIP_UPDATE_SNAPSHOTS").is_ok();
        for doc in &index.documents {
            let input = fs_anyhow::read_to_string(&input_dir.join(&doc.relative_path)).unwrap();
            let actual = format_snapshot(&input, doc, &index.external_symbols);
            let output_path = output_dir
                .join(&doc.relative_path)
                .with_extension("py.snap");
            if update {
                fs_anyhow::create_dir_all(output_path.parent().unwrap()).unwrap();
                fs_anyhow::write(&output_path, actual).unwrap();
                continue;
            }
            let expected = fs_anyhow::read_to_string(&output_path).unwrap_or_else(|_| {
                panic!(
                    "{} does not exist. Run with SCIP_UPDATE_SNAPSHOTS=1 to generate it.",
                    output_path.display()
                )
            });
            pretty_assertions::assert_eq!(
                expected,
                actual,
                "Snapshot mismatch for {}. Run with SCIP_UPDATE_SNAPSHOTS=1 to update.",
                output_path.display()
            );
        }
    }

    #[test]
    fn keyword_documentation_does_not_retain_dependency_answers() {
        let root = std::env::current_dir()
            .unwrap()
            .join("scip-keyword-project");
        let main_path = root.join("main.py");
        let lib_path = root.parent().unwrap().join("scip-keyword-lib.py");
        let library = "def target(value: str) -> None: ...\n";
        let mut env = TestEnv::new();
        env.add_with_path(
            "main",
            main_path.to_str().unwrap(),
            "from lib import target\ntarget(value='text')\n",
        );
        env.add_with_path("lib", lib_path.to_str().unwrap(), library);
        let main = Handle::new(
            ModuleName::from_str("main"),
            ModulePath::memory(main_path),
            env.sys_info(),
        );
        let lib = Handle::new(
            ModuleName::from_str("lib"),
            ModulePath::memory(lib_path),
            env.sys_info(),
        );
        let state = State::new(env.config_finder(), TEST_THREAD_COUNT);
        let mut transaction = state.new_transaction(Require::Exports, None);
        transaction.set_memory(env.get_memory());
        let scip = ScipWriter::new(
            Vec::new(),
            slice::from_ref(&main),
            &root,
            Package {
                name: "demo".to_owned(),
                version: "1".to_owned(),
            },
            "test",
        )
        .unwrap()
        .start(&mut transaction);
        transaction.run(slice::from_ref(&lib), Require::Errors, None);
        transaction.run(slice::from_ref(&main), Require::Everything, None);
        assert!(transaction.get_answers(&main).is_none());
        assert!(transaction.get_answers(&lib).is_none());
        let parameter_position = TextSize::try_from(library.find("value").unwrap()).unwrap();
        let (symbol, documented_at_use) = scip
            .external_symbols
            .lock()
            .iter()
            .find_map(|(symbol, first_use)| {
                let (handle, position) = first_use.definition.as_ref()?;
                (handle == &lib && *position == parameter_position)
                    .then(|| (symbol.clone(), !first_use.documentation.is_empty()))
            })
            .expect("the keyword refers to the external parameter");
        assert!(documented_at_use);
        let index = Index::parse_from_bytes(&scip.finish(&mut transaction).unwrap()).unwrap();
        assert_eq!(index.documents.len(), 1);
        assert!(transaction.get_answers(&lib).is_none());
        let parameter = index
            .external_symbols
            .iter()
            .find(|info| info.symbol == symbol)
            .unwrap();
        assert!(
            parameter
                .documentation
                .iter()
                .any(|doc| doc.contains("value: str"))
        );
    }

    #[test]
    fn external_symbols_are_deduplicated() {
        // `print` and `int` are each referenced more than once.
        let symbols: Vec<String> = index("class_nohint")
            .external_symbols
            .into_iter()
            .map(|x| x.symbol)
            .collect();
        assert_eq!(
            symbols,
            [
                "scip-python python python-stdlib 3.13 builtins/int#",
                "scip-python python python-stdlib 3.13 builtins/print().",
            ]
        );
    }

    macro_rules! snapshot_tests {
        ($($case:ident),* $(,)?) => {
            $(
                #[test]
                fn $case() {
                    assert_snapshot(stringify!($case));
                }
            )*
        };
    }

    snapshot_tests!(
        aliased_import,
        builtin_imports,
        class_nohint,
        comprehensions,
        dunder_vars,
        empty_project_name_version,
        enclosing_range_class,
        enclosing_range_function,
        f_string,
        file_from_module_import,
        graph_1278,
        lambda_walrus,
        nameparts_import,
        nested_items,
        nonlocal_skips_scope,
        odd_pkg_name_1,
        project_site_packages,
        pyproject_1,
        relative_import,
        request_goofiness,
        single_class,
        single_function,
        type_parameters,
        unique,
        unresolved_import,
    );
}
