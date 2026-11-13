/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_python::ast::Ast;
use pyrefly_python::docstring::Docstring;
use pyrefly_python::module::Module;
use pyrefly_python::module_path::ModuleStyle;
use pyrefly_python::sys_info::SysInfo;
use ruff_python_ast::Expr;
use ruff_python_ast::ModModule;
use ruff_python_ast::Stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

#[derive(Debug, Clone)]
pub(crate) enum AttributeKind {
    Function,
    Class,
    Assignment,
}

#[derive(Debug, Clone)]
pub(crate) struct AttributeContext {
    pub(crate) parent_classes: Vec<ruff_python_ast::name::Name>,
    pub(crate) kind: AttributeKind,
}

impl AttributeContext {
    /// Finds a module-level or class-level declaration with this exact name range.
    pub(crate) fn from_module(
        module: &Module,
        target_range: TextRange,
    ) -> Option<AttributeContext> {
        let ast = Ast::parse(module.contents(), module.source_type()).0;
        let mut parents = Vec::new();
        Self::from_body(ast.body.as_slice(), &mut parents, target_range)
    }

    fn from_body(
        body: &[Stmt],
        parents: &mut Vec<ruff_python_ast::name::Name>,
        target_range: TextRange,
    ) -> Option<AttributeContext> {
        for stmt in body {
            let kind = match stmt {
                Stmt::ClassDef(class_def) if class_def.name.range == target_range => {
                    AttributeKind::Class
                }
                Stmt::ClassDef(class_def) if class_def.range().contains_range(target_range) => {
                    parents.push(class_def.name.id.clone());
                    return Self::from_body(&class_def.body, parents, target_range);
                }
                Stmt::FunctionDef(function) if function.name.range == target_range => {
                    AttributeKind::Function
                }
                Stmt::If(if_stmt) => {
                    if let Some(context) = Ast::if_branches(if_stmt).find_map(|(_, _, branch)| {
                        Self::from_body(branch, &mut parents.clone(), target_range)
                    }) {
                        return Some(context);
                    }
                    continue;
                }
                Stmt::Assign(assign)
                    if assign.targets.iter().any(
                        |target| matches!(target, Expr::Name(name) if name.range() == target_range),
                    ) =>
                {
                    AttributeKind::Assignment
                }
                Stmt::AnnAssign(assign)
                    if assign.target.range() == target_range
                        && matches!(assign.target.as_ref(), Expr::Name(_)) =>
                {
                    AttributeKind::Assignment
                }
                _ => continue,
            };
            return Some(AttributeContext {
                parent_classes: parents.clone(),
                kind,
            });
        }
        None
    }
}

pub(crate) fn expr_matches_name(expr: &Expr, attr_name: &ruff_python_ast::name::Name) -> bool {
    match expr {
        Expr::Name(name) => &name.id == attr_name,
        _ => false,
    }
}

/// Finds the declaration of `attr_name` in the scope that `context` describes. Only the branches
/// of `if` statements that `sys_info` does not rule out are searched, so a declaration is never
/// taken from a branch that the checker considers unreachable.
pub(crate) fn definition_from_ast(
    ast: &ModModule,
    context: &AttributeContext,
    attr_name: &ruff_python_ast::name::Name,
    sys_info: SysInfo,
    module_style: ModuleStyle,
) -> Option<(TextRange, Option<TextRange>)> {
    definition_in_suite(
        &ast.body,
        &context.parent_classes,
        context,
        attr_name,
        sys_info,
        module_style,
    )
}

/// Searches `body` for the declaration, descending through the remaining `parent_classes` and
/// then into the live branches of `if` statements, each of which is its own suite.
fn definition_in_suite(
    body: &[Stmt],
    parent_classes: &[ruff_python_ast::name::Name],
    context: &AttributeContext,
    attr_name: &ruff_python_ast::name::Name,
    sys_info: SysInfo,
    module_style: ModuleStyle,
) -> Option<(TextRange, Option<TextRange>)> {
    let direct = match parent_classes.split_first() {
        Some((class_name, rest)) => body.iter().find_map(|stmt| match stmt {
            Stmt::ClassDef(class_def) if &class_def.name.id == class_name => definition_in_suite(
                &class_def.body,
                rest,
                context,
                attr_name,
                sys_info,
                module_style,
            ),
            _ => None,
        }),
        None => match context.kind {
            AttributeKind::Function => definition_from_function(body, attr_name),
            AttributeKind::Class => definition_from_class(body, attr_name),
            AttributeKind::Assignment => definition_from_assignment(body, attr_name),
        },
    };
    direct.or_else(|| {
        let find_in_if = |stmt: &Stmt| match stmt {
            Stmt::If(if_stmt) => sys_info
                .pruned_if_branches(if_stmt)
                .find_map(|(_, _, branch)| {
                    definition_in_suite(
                        branch,
                        parent_classes,
                        context,
                        attr_name,
                        sys_info,
                        module_style,
                    )
                }),
            _ => None,
        };
        // References use the interface's first declaration as its identity; source documentation
        // follows the last live definition across separate conditional statements.
        match module_style {
            ModuleStyle::Interface => body.iter().find_map(find_in_if),
            ModuleStyle::Executable => body.iter().rev().find_map(find_in_if),
        }
    })
}

fn definition_from_function(
    body: &[Stmt],
    attr_name: &ruff_python_ast::name::Name,
) -> Option<(TextRange, Option<TextRange>)> {
    for stmt in body {
        if let Stmt::FunctionDef(func_def) = stmt
            && &func_def.name.id == attr_name
        {
            let docstring_range = Docstring::range_from_stmts(&func_def.body);
            return Some((func_def.name.range, docstring_range));
        }
    }
    None
}

fn definition_from_class(
    body: &[Stmt],
    attr_name: &ruff_python_ast::name::Name,
) -> Option<(TextRange, Option<TextRange>)> {
    for stmt in body {
        if let Stmt::ClassDef(class_def) = stmt
            && &class_def.name.id == attr_name
        {
            let docstring_range = Docstring::range_from_stmts(&class_def.body);
            return Some((class_def.name.range, docstring_range));
        }
    }
    None
}

fn definition_from_assignment(
    body: &[Stmt],
    attr_name: &ruff_python_ast::name::Name,
) -> Option<(TextRange, Option<TextRange>)> {
    for (idx, stmt) in body.iter().enumerate() {
        let (matches, def_range) = match stmt {
            Stmt::Assign(assign) => {
                let target_opt = assign
                    .targets
                    .iter()
                    .find(|target| expr_matches_name(target, attr_name));
                (target_opt.is_some(), target_opt.map(|t| t.range()))
            }
            Stmt::AnnAssign(assign) => {
                let matches = expr_matches_name(assign.target.as_ref(), attr_name);
                (matches, Some(assign.target.range()))
            }
            _ => (false, None),
        };
        if matches {
            let docstring_range = if let Some(Stmt::Expr(expr_stmt)) = body.get(idx + 1)
                && matches!(expr_stmt.value.as_ref(), Expr::StringLiteral(_))
            {
                Some(expr_stmt.range())
            } else {
                None
            };
            return Some((def_range?, docstring_range));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pyrefly_python::module_name::ModuleName;
    use pyrefly_python::module_path::ModulePath;
    use ruff_python_ast::PySourceType;
    use ruff_python_ast::name::Name;

    use super::*;

    /// A method declared under `if sys.version_info` in a stub maps to the unconditional
    /// method in the source, and the source docstring is found.
    #[test]
    fn test_method_in_conditional_block() {
        let stub = "class C:\n    if sys.version_info >= (3, 12):\n        def f(self): ...\n    else:\n        def f(self, x): ...\n";
        let source = "class C:\n    def f(self):\n        \"\"\"Doc.\"\"\"\n";
        let stub_module = Module::new(
            ModuleName::from_str("m"),
            ModulePath::filesystem("m.pyi".into()),
            Arc::new(stub.to_owned()),
        );
        let offset = stub.find("f(self)").unwrap();
        let range = TextRange::at(u32::try_from(offset).unwrap().into(), 1.into());
        let context = AttributeContext::from_module(&stub_module, range).unwrap();
        let ast = Ast::parse(source, stub_module.source_type()).0;
        let (_, docstring) = definition_from_ast(
            &ast,
            &context,
            &Name::new("f"),
            SysInfo::default(),
            ModuleStyle::Executable,
        )
        .unwrap();
        assert!(docstring.is_some());
    }

    /// Only the branch selected by the environment is searched, not the first branch in the file.
    #[test]
    fn test_unreachable_branch_is_skipped() {
        let source = "if sys.version_info >= (3, 99):\n    def f(): \"\"\"Future.\"\"\"\nelse:\n    def f(): \"\"\"Current.\"\"\"\n";
        let context = AttributeContext {
            parent_classes: Vec::new(),
            kind: AttributeKind::Function,
        };
        let ast = Ast::parse(source, PySourceType::Python).0;
        let (_, docstring) = definition_from_ast(
            &ast,
            &context,
            &Name::new("f"),
            SysInfo::default(),
            ModuleStyle::Executable,
        )
        .unwrap();
        assert_eq!(&source[docstring.unwrap()], "\"\"\"Current.\"\"\"");
    }

    #[test]
    fn test_repeated_live_conditionals() {
        let cases = [
            (
                AttributeKind::Function,
                Vec::new(),
                "if sys.version_info >= (3, 8):\n    def f(): \"\"\"First.\"\"\"\nif sys.version_info >= (3, 10):\n    def f(): \"\"\"Last.\"\"\"\n",
            ),
            (
                AttributeKind::Function,
                vec![Name::new("C")],
                "class C:\n    if sys.version_info >= (3, 8):\n        def f(self): \"\"\"First.\"\"\"\n    if sys.version_info >= (3, 10):\n        def f(self): \"\"\"Last.\"\"\"\n",
            ),
            (
                AttributeKind::Class,
                Vec::new(),
                "if sys.version_info >= (3, 8):\n    class f:\n        \"\"\"First.\"\"\"\nif sys.version_info >= (3, 10):\n    class f:\n        \"\"\"Last.\"\"\"\n",
            ),
            (
                AttributeKind::Function,
                vec![Name::new("C")],
                "if sys.version_info >= (3, 8):\n    class C:\n        def f(self): \"\"\"First.\"\"\"\nif sys.version_info >= (3, 10):\n    class C:\n        def f(self): \"\"\"Last.\"\"\"\n",
            ),
            (
                AttributeKind::Assignment,
                Vec::new(),
                "if sys.version_info >= (3, 8):\n    f = 1\n    \"\"\"First.\"\"\"\nif sys.version_info >= (3, 10):\n    f: int = 2\n    \"\"\"Last.\"\"\"\n",
            ),
        ];
        for (kind, parent_classes, source) in cases {
            let context = AttributeContext {
                parent_classes,
                kind,
            };
            let ast = Ast::parse(source, PySourceType::Python).0;
            let (_, docstring) = definition_from_ast(
                &ast,
                &context,
                &Name::new("f"),
                SysInfo::default(),
                ModuleStyle::Executable,
            )
            .unwrap();
            assert_eq!(&source[docstring.unwrap()], "\"\"\"Last.\"\"\"", "{source}");
        }
    }

    #[test]
    fn test_later_unreachable_conditional_does_not_hide_live_definition() {
        let source = "if sys.version_info >= (3, 8):\n    def f(): \"\"\"Live.\"\"\"\nif sys.version_info >= (3, 99):\n    def f(): \"\"\"Unreachable.\"\"\"\n";
        let context = AttributeContext {
            parent_classes: Vec::new(),
            kind: AttributeKind::Function,
        };
        let ast = Ast::parse(source, PySourceType::Python).0;
        let (_, docstring) = definition_from_ast(
            &ast,
            &context,
            &Name::new("f"),
            SysInfo::default(),
            ModuleStyle::Executable,
        )
        .unwrap();
        assert_eq!(&source[docstring.unwrap()], "\"\"\"Live.\"\"\"");
    }
}
