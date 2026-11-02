/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use dupe::Dupe;
use pyrefly_python::ast::Ast;
use pyrefly_python::module::Module;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ModModule;
use ruff_python_ast::Stmt;
use ruff_python_ast::helpers::is_docstring_stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

use crate::ModuleInfo;
use crate::state::lsp::quick_fixes::extract_shared::find_enclosing_function;
use crate::state::lsp::quick_fixes::extract_shared::line_indent_and_start;
use crate::state::lsp::quick_fixes::extract_shared::needs_pass_after_removal;
use crate::state::lsp::quick_fixes::extract_shared::statement_removal_range;

/// Removes an isolated assignment only when its value and binding can be discarded.
pub(crate) fn remove_unused_variable_code_action(
    module_info: &ModuleInfo,
    ast: &ModModule,
    unused_name: &str,
    unused_range: TextRange,
) -> Option<(String, Module, TextRange, String)> {
    if Ast::is_intentionally_unused(unused_name) {
        return None;
    }
    let function = find_enclosing_function(ast, unused_range)?;
    // Replacing a parameter releases its previous value even when the initializer is pure.
    if function.parameters.includes(unused_name) {
        return None;
    }
    let context = find_unused_assignment_context(&function.body, unused_range)?;

    // The unused-variable detector can miss references before an assignment.
    // Reject other occurrences, including closures and potentially shadowed names.
    let mut references = ReferenceFinder {
        name: unused_name,
        definition_range: unused_range,
        found: false,
    };
    references.visit_body(&function.body);
    if references.found {
        return None;
    }

    let source = module_info.contents();
    let removal_range = statement_removal_range(source, context.stmt)?;
    let prefix = &source[removal_range.start().to_usize()..context.stmt.start().to_usize()];
    let suffix = &source[context.stmt.end().to_usize()..removal_range.end().to_usize()];
    let suffix = suffix.trim_start_matches([' ', '\t', '\r', '\n']);
    // Full-line deletion must not remove a suite header or another statement.
    // A preceding explicit continuation can also put this statement on a shared logical line.
    if !prefix.chars().all(|c| matches!(c, ' ' | '\t'))
        || !(suffix.is_empty() || suffix.starts_with('#'))
        || source[..removal_range.start().to_usize()]
            .trim_end_matches(['\r', '\n'])
            .ends_with('\\')
    {
        return None;
    }
    // A formerly non-leading string must not become the function's docstring.
    let exposes_docstring = function
        .body
        .first()
        .is_some_and(|stmt| stmt.range() == context.stmt.range())
        && function.body.get(1).is_some_and(is_docstring_stmt);
    let replacement = if needs_pass_after_removal(context.parent_body, context.stmt.range())
        || exposes_docstring
    {
        let (indent, _) = line_indent_and_start(source, context.stmt.range().start())?;
        format!("{indent}pass\n")
    } else {
        String::new()
    };
    Some((
        format!("Remove unused variable `{unused_name}`"),
        module_info.dupe(),
        removal_range,
        replacement,
    ))
}

struct UnusedAssignmentContext<'a> {
    stmt: &'a Stmt,
    parent_body: &'a [Stmt],
}

struct ReferenceFinder<'a> {
    name: &'a str,
    definition_range: TextRange,
    found: bool,
}

impl<'a> Visitor<'a> for ReferenceFinder<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if self.found {
            return;
        }
        if let Stmt::Nonlocal(nonlocal) = stmt {
            // A nonlocal declaration requires an enclosing binding without reading its value.
            self.found = nonlocal
                .names
                .iter()
                .any(|name| name.id.as_str() == self.name);
        } else {
            walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if self.found {
            return;
        }
        if let Expr::Name(name) = expr
            && name.id.as_str() == self.name
            && name.range() != self.definition_range
        {
            self.found = true;
            return;
        }
        walk_expr(self, expr);
    }
}

/// Locates an assignment without crossing into nested function or class scopes.
fn find_unused_assignment_context(
    body: &[Stmt],
    unused_range: TextRange,
) -> Option<UnusedAssignmentContext<'_>> {
    for stmt in body {
        if removable_assignment_range(stmt) == Some(unused_range) {
            return Some(UnusedAssignmentContext {
                stmt,
                parent_body: body,
            });
        }
        if let Some(context) = find_unused_assignment_context_in_child_body(stmt, unused_range) {
            return Some(context);
        }
    }
    None
}

fn find_unused_assignment_context_in_child_body(
    stmt: &Stmt,
    unused_range: TextRange,
) -> Option<UnusedAssignmentContext<'_>> {
    match stmt {
        Stmt::For(for_stmt) => find_unused_assignment_context(&for_stmt.body, unused_range)
            .or_else(|| find_unused_assignment_context(&for_stmt.orelse, unused_range)),
        Stmt::While(while_stmt) => find_unused_assignment_context(&while_stmt.body, unused_range)
            .or_else(|| find_unused_assignment_context(&while_stmt.orelse, unused_range)),
        Stmt::If(if_stmt) => {
            find_unused_assignment_context(&if_stmt.body, unused_range).or_else(|| {
                if_stmt
                    .elif_else_clauses
                    .iter()
                    .find_map(|clause| find_unused_assignment_context(&clause.body, unused_range))
            })
        }
        Stmt::With(with_stmt) => find_unused_assignment_context(&with_stmt.body, unused_range),
        Stmt::Match(match_stmt) => match_stmt
            .cases
            .iter()
            .find_map(|case| find_unused_assignment_context(&case.body, unused_range)),
        Stmt::Try(try_stmt) => find_unused_assignment_context(&try_stmt.body, unused_range)
            .or_else(|| {
                try_stmt.handlers.iter().find_map(|handler| {
                    let ExceptHandler::ExceptHandler(handler) = handler;
                    find_unused_assignment_context(&handler.body, unused_range)
                })
            })
            .or_else(|| find_unused_assignment_context(&try_stmt.orelse, unused_range))
            .or_else(|| find_unused_assignment_context(&try_stmt.finalbody, unused_range)),
        _ => None,
    }
}

fn removable_assignment_range(stmt: &Stmt) -> Option<TextRange> {
    let (target, value) = match stmt {
        Stmt::Assign(assign) if assign.targets.len() == 1 => {
            (&assign.targets[0], assign.value.as_ref())
        }
        Stmt::AnnAssign(assign) => (assign.target.as_ref(), assign.value.as_deref()?),
        _ => return None,
    };
    // Only literal values are known to have no observable evaluation here.
    // Other expressions can call user code, raise, suspend, or bind another name.
    if !matches!(
        value,
        Expr::NumberLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
    ) {
        return None;
    }
    let Expr::Name(name) = target else {
        return None;
    };
    Some(name.range())
}
