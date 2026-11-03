/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;

use pyrefly_python::ast::Ast;
use pyrefly_python::module_name::ModuleName;
use ruff_python_ast::Comprehension;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprAttribute;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Identifier;
use ruff_python_ast::ModModule;
use ruff_python_ast::Parameters;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::TypeParams;
use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use ruff_python_ast::visitor::source_order::walk_except_handler;
use ruff_python_ast::visitor::source_order::walk_expr;
use ruff_python_ast::visitor::source_order::walk_pattern;
use ruff_python_ast::visitor::source_order::walk_stmt;
use ruff_python_ast::visitor::source_order::walk_type_param;
use ruff_text_size::TextRange;
use scip::types::Descriptor;
use scip::types::descriptor::Suffix;
use starlark_map::small_set::SmallSet;

use crate::report::glean::convert::gather_nonlocal_variables;

#[derive(Debug, Clone, PartialEq)]
pub enum ScipSymbol {
    Global(Vec<Descriptor>),
    Local(usize),
}

/// The SCIP symbol of every name a module defines, keyed by the defining identifier, in source
/// order.
pub fn definitions(module: ModuleName, ast: &ModModule) -> Vec<(TextRange, ScipSymbol)> {
    let module_path = vec![descriptor(module.as_str(), Suffix::Namespace)];
    let mut builder = Builder {
        scopes: vec![Scope::new(ScopeKind::Module, module_path.clone())],
        module_path,
        definitions: Vec::new(),
        next_local: 0,
    };
    builder.visit_body(&ast.body);
    builder.definitions
}

pub fn descriptor(name: &str, suffix: Suffix) -> Descriptor {
    Descriptor {
        name: name.to_owned(),
        suffix: suffix.into(),
        ..Default::default()
    }
}

enum ScopeKind {
    Module,
    Class {
        /// Names bound directly in the class body, plus `self.x` attributes seen so far.
        attributes: SmallSet<Name>,
    },
    Function {
        receiver: Option<Name>,
        globals: SmallSet<Name>,
        nonlocals: SmallSet<Name>,
    },
    /// Every name a comprehension binds is a local, except a walrus target.
    Comprehension,
    /// Every name a lambda binds is a local.
    Lambda,
}

struct Scope {
    kind: ScopeKind,
    path: Vec<Descriptor>,
    names: HashMap<Name, ScipSymbol>,
}

impl Scope {
    fn new(kind: ScopeKind, path: Vec<Descriptor>) -> Self {
        Self {
            kind,
            path,
            names: HashMap::new(),
        }
    }
}

struct Builder {
    module_path: Vec<Descriptor>,
    scopes: Vec<Scope>,
    definitions: Vec<(TextRange, ScipSymbol)>,
    next_local: usize,
}

fn child(path: &[Descriptor], name: &str, suffix: Suffix) -> Vec<Descriptor> {
    let mut path = path.to_vec();
    path.push(descriptor(name, suffix));
    path
}

impl Builder {
    fn scope(&self) -> &Scope {
        self.scopes
            .last()
            .expect("the module scope is never popped")
    }

    fn new_local(&mut self) -> ScipSymbol {
        self.next_local += 1;
        ScipSymbol::Local(self.next_local - 1)
    }

    fn define(&mut self, id: &Identifier, symbol: ScipSymbol) {
        self.definitions.push((id.range, symbol));
    }

    /// Define a name in the current scope, regardless of any earlier binding of it.
    fn define_in_scope(&mut self, id: &Identifier, symbol: ScipSymbol) {
        self.scopes
            .last_mut()
            .expect("the module scope is never popped")
            .names
            .insert(id.id.clone(), symbol.clone());
        self.define(id, symbol);
    }

    /// Define a function or class in the current scope, returning its path.
    fn declare(&mut self, id: &Identifier, suffix: Suffix) -> Vec<Descriptor> {
        let path = child(&self.scope().path, &id.id, suffix);
        self.define_in_scope(id, ScipSymbol::Global(path.clone()));
        path
    }

    /// The symbol for binding `name` in scope `index`, creating it on first use.
    fn binding(&mut self, index: usize, name: &Name) -> ScipSymbol {
        if let Some(symbol) = self.scopes[index].names.get(name) {
            return symbol.clone();
        }
        let symbol = match &self.scopes[index].kind {
            ScopeKind::Module | ScopeKind::Class { .. } => {
                ScipSymbol::Global(child(&self.scopes[index].path, name, Suffix::Term))
            }
            ScopeKind::Function { globals, .. } if globals.contains(name) => {
                ScipSymbol::Global(child(&self.module_path, name, Suffix::Term))
            }
            ScopeKind::Function { nonlocals, .. } if nonlocals.contains(name) => {
                // The nearest enclosing function that binds the name, or else the nearest one.
                let functions: Vec<usize> = (0..index)
                    .rev()
                    .filter(|i| matches!(self.scopes[*i].kind, ScopeKind::Function { .. }))
                    .collect();
                match functions
                    .iter()
                    .find(|i| self.scopes[**i].names.contains_key(name))
                    .or(functions.first())
                {
                    Some(enclosing) => self.binding(*enclosing, name),
                    None => self.new_local(),
                }
            }
            ScopeKind::Function { .. } | ScopeKind::Comprehension | ScopeKind::Lambda => {
                self.new_local()
            }
        };
        self.scopes[index]
            .names
            .insert(name.clone(), symbol.clone());
        symbol
    }

    /// Bind a name in the current scope. A walrus inside a comprehension binds in the enclosing
    /// scope instead.
    fn bind(&mut self, id: &Identifier, walrus: bool) {
        let mut index = self.scopes.len() - 1;
        if walrus {
            while index > 0 && matches!(self.scopes[index].kind, ScopeKind::Comprehension) {
                index -= 1;
            }
        }
        let symbol = self.binding(index, &id.id);
        self.define(id, symbol);
    }

    /// `self.x = ...` in a method defines the attribute `x`, unless the class already did.
    fn bind_attribute(&mut self, attribute: &ExprAttribute) {
        let [.., class, method] = self.scopes.as_mut_slice() else {
            return;
        };
        let (
            ScopeKind::Class { attributes },
            ScopeKind::Function {
                receiver: Some(receiver),
                ..
            },
        ) = (&mut class.kind, &method.kind)
        else {
            return;
        };
        if let Some(attr) = Ast::expr_receiver_attr(attribute, receiver)
            && attributes.insert(attr.id.clone())
        {
            let symbol = ScipSymbol::Global(child(&class.path, &attr.id, Suffix::Term));
            self.define(attr, symbol);
        }
    }

    fn bind_parameters(&mut self, parameters: &Parameters) {
        for parameter in parameters.iter() {
            let id = parameter.name();
            let symbol = match &self.scope().kind {
                ScopeKind::Comprehension | ScopeKind::Lambda => self.new_local(),
                _ => ScipSymbol::Global(child(&self.scope().path, &id.id, Suffix::Parameter)),
            };
            self.define_in_scope(id, symbol);
        }
    }

    /// Type parameters belong to the function, class or type alias that declares them.
    fn bind_type_params(&mut self, type_params: Option<&TypeParams>, owner: &ScipSymbol) {
        for type_param in type_params.iter().flat_map(|x| &x.type_params) {
            let symbol = match owner {
                ScipSymbol::Global(path) => {
                    ScipSymbol::Global(child(path, &type_param.name().id, Suffix::TypeParameter))
                }
                ScipSymbol::Local(_) => self.new_local(),
            };
            self.define(type_param.name(), symbol);
            walk_type_param(self, type_param);
        }
    }

    fn visit_comprehensions(&mut self, generators: &[Comprehension], elements: &[&Expr]) {
        // The first iterable is evaluated in the enclosing scope.
        if let Some(first) = generators.first() {
            self.visit_expr(&first.iter);
        }
        self.scopes.push(Scope::new(
            ScopeKind::Comprehension,
            self.scope().path.clone(),
        ));
        for (i, generator) in generators.iter().enumerate() {
            if i > 0 {
                self.visit_expr(&generator.iter);
            }
            self.visit_expr(&generator.target);
            for condition in &generator.ifs {
                self.visit_expr(condition);
            }
        }
        for element in elements {
            self.visit_expr(element);
        }
        self.scopes.pop();
    }
}

impl<'a> SourceOrderVisitor<'a> for Builder {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(function) => {
                for decorator in &function.decorator_list {
                    self.visit_decorator(decorator);
                }
                self.visit_parameters(&function.parameters);
                if let Some(returns) = &function.returns {
                    self.visit_expr(returns);
                }
                let path = self.declare(&function.name, Suffix::Method);
                self.bind_type_params(
                    function.type_params.as_deref(),
                    &ScipSymbol::Global(path.clone()),
                );
                let receiver = match &self.scope().kind {
                    ScopeKind::Class { .. } => function
                        .parameters
                        .iter_non_variadic_params()
                        .next()
                        .map(|p| p.parameter.name.id.clone()),
                    _ => None,
                };
                let (globals, nonlocals) = gather_nonlocal_variables(&function.body);
                self.scopes.push(Scope::new(
                    ScopeKind::Function {
                        receiver,
                        globals: (*globals).clone(),
                        nonlocals: (*nonlocals).clone(),
                    },
                    path,
                ));
                self.bind_parameters(&function.parameters);
                self.visit_body(&function.body);
                self.scopes.pop();
            }
            Stmt::ClassDef(class) => {
                for decorator in &class.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(arguments) = &class.arguments {
                    self.visit_arguments(arguments);
                }
                let path = self.declare(&class.name, Suffix::Type);
                self.bind_type_params(
                    class.type_params.as_deref(),
                    &ScipSymbol::Global(path.clone()),
                );
                let mut attributes = SmallSet::new();
                for stmt in &class.body {
                    match stmt {
                        Stmt::FunctionDef(x) => {
                            attributes.insert(x.name.id.clone());
                        }
                        Stmt::ClassDef(x) => {
                            attributes.insert(x.name.id.clone());
                        }
                        Stmt::Assign(x) => x.targets.iter().for_each(|t| {
                            Ast::expr_lvalue(t, &mut |n| {
                                attributes.insert(n.id.clone());
                            })
                        }),
                        Stmt::AnnAssign(x) => Ast::expr_lvalue(&x.target, &mut |n| {
                            attributes.insert(n.id.clone());
                        }),
                        _ => {}
                    }
                }
                self.scopes
                    .push(Scope::new(ScopeKind::Class { attributes }, path));
                self.visit_body(&class.body);
                self.scopes.pop();
            }
            Stmt::TypeAlias(alias) => {
                self.visit_expr(&alias.name);
                if let Expr::Name(name) = &*alias.name
                    && let Some(owner) = self.scope().names.get(&name.id).cloned()
                {
                    self.bind_type_params(alias.type_params.as_deref(), &owner);
                }
                self.visit_expr(&alias.value);
            }
            Stmt::AugAssign(assign) => {
                if !matches!(&*assign.target, Expr::Name(_)) {
                    self.visit_expr(&assign.target);
                }
                self.visit_expr(&assign.value);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Name(name) if name.ctx == ExprContext::Store => {
                self.bind(&Ast::expr_name_identifier(name.clone()), false)
            }
            Expr::Named(named) => {
                if let Expr::Name(name) = &*named.target {
                    self.bind(&Ast::expr_name_identifier(name.clone()), true);
                }
                self.visit_expr(&named.value);
            }
            Expr::Attribute(attribute) if attribute.ctx == ExprContext::Store => {
                self.bind_attribute(attribute);
                self.visit_expr(&attribute.value);
            }
            Expr::ListComp(x) => self.visit_comprehensions(&x.generators, &[&x.elt]),
            Expr::SetComp(x) => self.visit_comprehensions(&x.generators, &[&x.elt]),
            Expr::Generator(x) => self.visit_comprehensions(&x.generators, &[&x.elt]),
            Expr::DictComp(x) => {
                let elements: Vec<&Expr> =
                    x.key.as_deref().into_iter().chain([&*x.value]).collect();
                self.visit_comprehensions(&x.generators, &elements)
            }
            Expr::Lambda(lambda) => {
                if let Some(parameters) = &lambda.parameters {
                    self.visit_parameters(parameters);
                }
                self.scopes
                    .push(Scope::new(ScopeKind::Lambda, self.scope().path.clone()));
                if let Some(parameters) = &lambda.parameters {
                    self.bind_parameters(parameters);
                }
                self.visit_expr(&lambda.body);
                self.scopes.pop();
            }
            _ => walk_expr(self, expr),
        }
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        let ExceptHandler::ExceptHandler(x) = handler;
        if let Some(name) = &x.name {
            self.bind(name, false);
        }
        walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        let name = match pattern {
            Pattern::MatchAs(x) => x.name.as_ref(),
            Pattern::MatchStar(x) => x.name.as_ref(),
            Pattern::MatchMapping(x) => x.rest.as_ref(),
            _ => None,
        };
        if let Some(name) = name {
            self.bind(name, false);
        }
        walk_pattern(self, pattern);
    }
}

#[cfg(test)]
mod tests {
    use pyrefly_python::ast::Ast;
    use ruff_python_ast::PySourceType;
    use scip::symbol::format_symbol;
    use scip::types::Symbol;

    use super::*;

    fn symbols(code: &str) -> Vec<(String, String)> {
        let (ast, _, _) = Ast::parse(code, PySourceType::Python);
        definitions(ModuleName::from_str("m"), &ast)
            .iter()
            .map(|(range, symbol)| {
                let symbol = match symbol {
                    ScipSymbol::Global(descriptors) => format_symbol(Symbol {
                        scheme: "s".to_owned(),
                        descriptors: descriptors.clone(),
                        ..Default::default()
                    }),
                    ScipSymbol::Local(i) => format!("local {i}"),
                };
                (code[*range].to_owned(), symbol)
            })
            .collect()
    }

    fn assert_symbols(code: &str, expected: &[(&str, &str)]) {
        let expected: Vec<_> = expected
            .iter()
            .map(|(name, symbol)| (name.to_string(), format!("s . . . {symbol}")))
            .collect();
        assert_eq!(symbols(code), expected);
    }

    #[test]
    fn test_classes_and_functions() {
        assert_symbols(
            r#"
class C:
    x: int
    class D: ...
    def f(self, a, *args, **kwargs):
        def g(): ...
"#,
            &[
                ("C", "m/C#"),
                ("x", "m/C#x."),
                ("D", "m/C#D#"),
                ("f", "m/C#f()."),
                ("self", "m/C#f().(self)"),
                ("a", "m/C#f().(a)"),
                ("args", "m/C#f().(args)"),
                ("kwargs", "m/C#f().(kwargs)"),
                ("g", "m/C#f().g()."),
            ],
        );
    }

    #[test]
    fn test_self_attributes() {
        assert_symbols(
            r#"
class C:
    y: int
    def __init__(self, v):
        self.x = v
        self.x = v
        self.y = v
"#,
            &[
                ("C", "m/C#"),
                ("y", "m/C#y."),
                ("__init__", "m/C#__init__()."),
                ("self", "m/C#__init__().(self)"),
                ("v", "m/C#__init__().(v)"),
                ("x", "m/C#x."),
            ],
        );
    }

    #[test]
    fn test_locals() {
        let actual = symbols(
            r#"
x = 1
for y in []: pass
def f(p):
    p = 1
    z = [w for w in []]
    z = 2
    global x
    x = 3
squares = [w * w for w in []]
"#,
        );
        let expected = [
            ("x", "s . . . m/x."),
            ("y", "s . . . m/y."),
            ("f", "s . . . m/f()."),
            ("p", "s . . . m/f().(p)"),
            ("p", "s . . . m/f().(p)"),
            ("z", "local 0"),
            ("w", "local 1"),
            ("z", "local 0"),
            ("x", "s . . . m/x."),
            ("squares", "s . . . m/squares."),
            ("w", "local 2"),
        ];
        assert_eq!(
            actual,
            expected
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect::<Vec<_>>()
        );
    }
}
