/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Detect runnable Python definitions from the AST.

use pyrefly_python::ast::Ast;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprAttribute;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_text_size::TextRange;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunnableKind {
    Run,
    Test,
}

#[derive(Clone, Debug)]
pub(crate) struct RunnableEntry {
    pub(crate) range: TextRange,
    pub(crate) kind: RunnableKind,
    pub(crate) test_name: Option<String>,
    pub(crate) class_name: Option<String>,
    pub(crate) is_unittest: bool,
}

/// Return runnable entries from the module statements.
pub(crate) fn runnable_entries(stmts: &[Stmt]) -> Vec<RunnableEntry> {
    let mut entries = Vec::new();
    collect_module_entries(stmts, &mut entries);
    entries
}

fn collect_module_entries(stmts: &[Stmt], entries: &mut Vec<RunnableEntry>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                maybe_push_test(entries, func.name.as_str(), func.name.range, None, false);
            }
            Stmt::ClassDef(class_def) => {
                let is_unittest = is_unittest_class(class_def);
                if is_test_class(class_def, is_unittest) {
                    entries.push(RunnableEntry {
                        range: class_def.name.range,
                        kind: RunnableKind::Test,
                        test_name: None,
                        class_name: Some(class_def.name.as_str().to_owned()),
                        is_unittest,
                    });
                }
                collect_class_entries(
                    &class_def.body,
                    entries,
                    class_def.name.as_str(),
                    is_unittest,
                );
            }
            Stmt::If(stmt_if) if Ast::is_main_guard(&stmt_if.test) => {
                entries.push(RunnableEntry {
                    range: stmt_if.range,
                    kind: RunnableKind::Run,
                    test_name: None,
                    class_name: None,
                    is_unittest: false,
                });
            }
            _ => {}
        }
    }
}

fn collect_class_entries(
    stmts: &[Stmt],
    entries: &mut Vec<RunnableEntry>,
    class_name: &str,
    is_unittest: bool,
) {
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            maybe_push_test(
                entries,
                func.name.as_str(),
                func.name.range,
                Some(class_name.to_owned()),
                is_unittest,
            );
        }
    }
}

fn maybe_push_test(
    entries: &mut Vec<RunnableEntry>,
    name: &str,
    range: TextRange,
    class_name: Option<String>,
    is_unittest: bool,
) {
    if is_test_name(name) {
        entries.push(RunnableEntry {
            range,
            kind: RunnableKind::Test,
            test_name: Some(name.to_owned()),
            class_name,
            is_unittest,
        });
    }
}

fn is_test_name(name: &str) -> bool {
    name.starts_with("test_")
}

fn is_test_class(class_def: &StmtClassDef, is_unittest: bool) -> bool {
    class_def.name.as_str().starts_with("Test") || is_unittest
}

fn is_unittest_class(class_def: &StmtClassDef) -> bool {
    class_def.bases().iter().any(is_unittest_base)
}

fn is_unittest_base(base: &Expr) -> bool {
    match base {
        Expr::Name(name) => name.id.as_str().ends_with("TestCase"),
        Expr::Attribute(ExprAttribute { attr, .. }) => attr.id.as_str().ends_with("TestCase"),
        _ => false,
    }
}
