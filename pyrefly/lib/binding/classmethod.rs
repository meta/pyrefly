/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::mem;

use pyrefly_graph::index::Idx;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use ruff_python_ast::visitor::source_order::walk_expr;
use ruff_python_ast::visitor::source_order::walk_stmt;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::binding::binding::Binding;
use crate::binding::binding::Key;
use crate::binding::binding::KeyUndecoratedFunction;
use crate::binding::bindings::BindingsBuilder;
use crate::binding::scope::FlowStyle;
use crate::export::special::SpecialExport;

/// The inferred receiver mode of a recognized class-body reassignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MethodReceiverKind {
    #[default]
    None,
    ClassOnly,
    Shared,
}

/// Captured once where a function's decorators are evaluated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum FunctionDecorators {
    #[default]
    Undecorated,
    Overload,
    OverloadWithOther,
    ClassMethod,
    StaticMethod,
    Other,
}

impl FunctionDecorators {
    pub(super) fn single(special: Option<SpecialExport>) -> Self {
        match special {
            Some(SpecialExport::Overload) => Self::Overload,
            Some(SpecialExport::ClassMethod) => Self::ClassMethod,
            Some(SpecialExport::StaticMethod) => Self::StaticMethod,
            _ => Self::Other,
        }
    }

    fn has_overload(self) -> bool {
        matches!(self, Self::Overload | Self::OverloadWithOther)
    }

    fn is_invariant(self) -> bool {
        matches!(self, Self::ClassMethod | Self::StaticMethod)
    }
}

/// Allocated only for classes with syntactic reassignment candidates.
#[derive(Clone, Debug, Default)]
pub(super) struct ClassmethodReceiverState {
    candidates: SmallSet<Name>,
    /// An entry exists only for an unconditional candidate definition. `None`
    /// means its operations require separate instance/class body inference.
    uses: SmallMap<Idx<KeyUndecoratedFunction>, Option<Box<[Name]>>>,
    decorators: SmallMap<Idx<KeyUndecoratedFunction>, FunctionDecorators>,
    used: SmallSet<Idx<KeyUndecoratedFunction>>,
    /// Queuing a group excludes the classmethod constructor's own operand read.
    wrapped: SmallSet<Idx<KeyUndecoratedFunction>>,
    pending: Vec<Box<[Idx<KeyUndecoratedFunction>]>>,
}

impl BindingsBuilder<'_> {
    /// Receiver rewrites need direct import or implicit-builtin provenance. Assigned
    /// aliases and non-import merge styles are unsupported, not spelling-based proofs.
    pub(super) fn as_receiver_special_export(&self, expr: &Expr) -> Option<SpecialExport> {
        let (name, base) = match expr {
            Expr::Name(name) => (&name.id, None),
            Expr::Attribute(attr) if let Expr::Name(base) = attr.value.as_ref() => {
                (&attr.attr.id, Some(&base.id))
            }
            _ => return None,
        };
        if let Some((_, style)) = self.scopes.binding_idx_for_name(base.unwrap_or(name))
            && !matches!(
                style,
                FlowStyle::Import(..) | FlowStyle::ImportAs(_) | FlowStyle::MergeableImport(_)
            )
        {
            return None;
        }
        self.scopes
            .as_special_export(name, base, self.module_info.name(), self.lookup)
    }

    /// Enter an eager class body and retain the enclosing class's tracking mode.
    pub(super) fn begin_classmethod_receivers(&mut self, body: &[Stmt]) -> bool {
        let previous = self.track_classmethod_uses;
        let candidates: SmallSet<Name> = body
            .iter()
            .filter_map(|stmt| {
                if let Stmt::Assign(assign) = stmt
                    && let [Expr::Name(name)] = assign.targets.as_slice()
                    && let Expr::Call(call) = assign.value.as_ref()
                    && let [Expr::Name(argument)] = &*call.arguments.args
                    && argument.id == name.id
                    && call.arguments.keywords.is_empty()
                {
                    Some(name.id.clone())
                } else {
                    None
                }
            })
            .collect();
        self.track_classmethod_uses = !candidates.is_empty();
        if self.track_classmethod_uses {
            *self
                .scopes
                .classmethod_receivers_mut()
                .expect("receiver tracking starts after entering a class body") =
                Some(Box::new(ClassmethodReceiverState {
                    candidates,
                    ..ClassmethodReceiverState::default()
                }));
        }
        previous
    }

    /// `None` is not a candidate; `Some(None)` is a candidate with unsupported uses.
    pub(super) fn classmethod_receiver_uses(
        &self,
        def: &StmtFunctionDef,
    ) -> Option<Option<Box<[Name]>>> {
        if !self.track_classmethod_uses
            || !self.scopes.in_unconditional_class_body()
            || !self
                .scopes
                .classmethod_receivers()
                .is_some_and(|state| state.candidates.contains(&def.name.id))
        {
            return None;
        }
        Some(
            def.parameters
                .iter()
                .next()
                .and_then(|p| ReceiverUseFinder::find(&p.name().id, &def.body)),
        )
    }

    pub(super) fn record_classmethod_function(
        &mut self,
        function: Idx<KeyUndecoratedFunction>,
        decorators: FunctionDecorators,
        uses: Option<Option<Box<[Name]>>>,
    ) {
        if !self.track_classmethod_uses {
            return;
        }
        let Some(state) = self
            .scopes
            .classmethod_receivers_mut()
            .and_then(Option::as_deref_mut)
        else {
            return;
        };
        state.decorators.insert(function, decorators);
        if let Some(members) = uses {
            state.uses.insert(function, members);
        }
    }

    /// A complete class-function flow binding, if it is available.
    fn class_function_at(&self, idx: Idx<Key>) -> Option<Idx<KeyUndecoratedFunction>> {
        let Binding::Function {
            decorated_idx,
            in_class: true,
            ..
        } = self.idx_to_binding(idx)?
        else {
            return None;
        };
        Some(self.idx_to_binding(*decorated_idx)?.undecorated_idx)
    }

    pub(super) fn record_classmethod_use(&mut self, idx: Idx<Key>) {
        let Some(function) = self.class_function_at(idx) else {
            return;
        };
        let Some(state) = self
            .scopes
            .classmethod_receivers_mut()
            .and_then(Option::as_deref_mut)
        else {
            return;
        };
        if state.uses.contains_key(&function) && !state.wrapped.contains(&function) {
            state.used.insert(function);
        }
    }

    pub(super) fn infer_classmethod_receiver(&mut self, name: &Name) {
        let Some(state) = self.scopes.classmethod_receivers() else {
            return;
        };
        let Some(mut idx) = self.scopes.current_flow_idx(name) else {
            return;
        };
        let mut functions = Vec::new();
        while let Some(function) = self.class_function_at(idx) {
            let decorators = state
                .decorators
                .get(&function)
                .copied()
                .unwrap_or(FunctionDecorators::Other);
            if !functions.is_empty() && !decorators.has_overload() {
                break;
            }
            if !matches!(
                decorators,
                FunctionDecorators::Undecorated | FunctionDecorators::Overload
            ) || !state.uses.contains_key(&function)
            {
                return;
            }
            functions.push(function);
            let Some(Binding::Function {
                pred_idx: Some(previous),
                ..
            }) = self.idx_to_binding(idx)
            else {
                break;
            };
            idx = *previous;
        }
        if functions.is_empty() {
            return;
        }
        let state = self
            .scopes
            .classmethod_receivers_mut()
            .and_then(Option::as_deref_mut)
            .expect("an existing receiver state remains active while queuing a group");
        state.wrapped.extend(functions.iter().copied());
        state.pending.push(functions.into_boxed_slice());
    }

    fn invariant_member(&self, state: &ClassmethodReceiverState, member: &Name) -> bool {
        self.scopes
            .current_flow_idx(member)
            .and_then(|idx| self.class_function_at(idx))
            .and_then(|function| state.decorators.get(&function))
            .is_some_and(|decorators| decorators.is_invariant())
    }

    pub(super) fn finish_classmethod_receivers(&mut self) {
        let Some(state) = self
            .scopes
            .classmethod_receivers_mut()
            .and_then(Option::take)
        else {
            return;
        };
        for functions in &state.pending {
            let shared = functions
                .iter()
                .any(|function| state.used.contains(function));
            let supported = !shared
                || functions.iter().all(|function| {
                    state
                        .uses
                        .get(function)
                        .and_then(Option::as_ref)
                        .is_some_and(|members| {
                            members
                                .iter()
                                .all(|member| self.invariant_member(&state, member))
                        })
                });
            let kind = if !supported {
                MethodReceiverKind::None
            } else if shared {
                MethodReceiverKind::Shared
            } else {
                MethodReceiverKind::ClassOnly
            };
            let leader = self
                .idx_to_binding(functions[0])
                .expect("queued groups contain complete function bindings");
            let name = leader.def.name.id.clone();
            let range = leader.def.range;
            if supported {
                self.scopes.mark_method_as_classmethod(&name, range);
            }
            for function in functions {
                self.idx_to_binding_mut(*function)
                    .expect("queued groups contain complete function bindings")
                    .def
                    .method_receiver = kind;
            }
        }
    }
}

/// Receiver escapes, mutation, captures and implicit super need two body contexts.
struct ReceiverUseFinder<'a> {
    receiver: &'a Name,
    members: SmallSet<Name>,
    supported: bool,
    nested_scope: bool,
}

impl ReceiverUseFinder<'_> {
    fn find(receiver: &Name, body: &[Stmt]) -> Option<Box<[Name]>> {
        let mut finder = ReceiverUseFinder {
            receiver,
            members: SmallSet::new(),
            supported: true,
            nested_scope: false,
        };
        for stmt in body {
            finder.visit_stmt(stmt);
        }
        finder
            .supported
            .then(|| finder.members.into_iter().collect())
    }
}

impl<'a> SourceOrderVisitor<'a> for ReceiverUseFinder<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if !self.supported {
            return;
        }
        match stmt {
            Stmt::Return(ret)
                if !self.nested_scope
                    && matches!(ret.value.as_deref(), Some(Expr::Name(name))
                        if name.id == *self.receiver) => {}
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {
                let was_nested = mem::replace(&mut self.nested_scope, true);
                walk_stmt(self, stmt);
                self.nested_scope = was_nested;
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if !self.supported {
            return;
        }
        match expr {
            Expr::Call(call) if matches!(call.func.as_ref(), Expr::Name(name) if name.id.as_str() == "super") =>
            {
                self.supported = false;
            }
            Expr::Attribute(attr) if matches!(attr.value.as_ref(), Expr::Name(name) if name.id == *self.receiver) => {
                if !self.nested_scope && attr.ctx == ExprContext::Load {
                    self.members.insert(attr.attr.id.clone());
                } else {
                    self.supported = false;
                }
            }
            Expr::Name(name) if name.id == *self.receiver => self.supported = false,
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {
                let was_nested = mem::replace(&mut self.nested_scope, true);
                walk_expr(self, expr);
                self.nested_scope = was_nested;
            }
            _ => walk_expr(self, expr),
        }
    }
}
