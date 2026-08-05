/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_python::dunder;
use pyrefly_python::module_name::ModuleName;
use pyrefly_types::unittest_mock::UnittestMock;
use pyrefly_util::owner::Owner;
use ruff_python_ast::Arguments;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprCall;
use ruff_python_ast::ExprStringLiteral;
use ruff_python_ast::name::Name;
use ruff_text_size::Ranged;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::callable::CallArg;
use crate::alt::callable::CallKeyword;
use crate::alt::expr::TypeOrExpr;
use crate::config::error_kind::ErrorKind;
use crate::error::collector::ErrorCollector;
use crate::types::function::FuncFlags;
use crate::types::module::ModuleType;
use crate::types::types::Type;

/// The resolved target records whether ordinary patching must remove the method receiver.
pub(super) struct UnittestMockTarget {
    signature: Type,
    unbound_method: bool,
}

impl<'ctx, 'answer, Ans: LookupAnswer> AnswersSolver<'ctx, 'answer, Ans> {
    pub(super) fn check_unittest_mock_patch_target(
        &self,
        callee_ty: &Type,
        arguments: &Arguments,
        errors: &ErrorCollector,
    ) -> Option<UnittestMockTarget> {
        let Type::ClassType(cls) = callee_ty else {
            return None;
        };
        if !cls.has_qname("unittest.mock", "_patcher") {
            return None;
        }
        if arguments.args.len() > 1 || arguments.keywords.iter().any(|kw| kw.arg.is_none()) {
            return None;
        }
        if arguments
            .find_keyword("create")
            .is_some_and(|kw| !matches!(&kw.value, Expr::BooleanLiteral(lit) if !lit.value))
        {
            return None;
        }
        let target_expr = arguments.find_argument_value("target", 0)?;
        let Expr::StringLiteral(ExprStringLiteral { value, .. }) = target_expr else {
            return None;
        };
        let range = target_expr.range();
        let target = value.to_str();
        let parts: Vec<&str> = target.split('.').collect();
        if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) {
            return None;
        }

        let Some((module_prefix_len, module)) = (1..parts.len()).rev().find_map(|i| {
            let candidate = ModuleName::from_str(&parts[..i].join("."));
            (candidate == self.module().name()
                || self.exports.module_exists(candidate).finding().is_some())
            .then_some((i, candidate))
        }) else {
            // The target may be resolved dynamically at runtime, so only check paths rooted in a
            // module that is available in the current environment.
            return None;
        };

        let attrs = &parts[module_prefix_len..];
        let mut base_ty = ModuleType::new_as(module).to_type(self.heap);
        let mut on_class = false;
        for (i, attr) in attrs.iter().enumerate() {
            let name = Name::new(*attr);
            // Patching a public builtin name on a module always succeeds: `mock.patch` sets
            // `create=True` itself in that case, overriding whatever the caller passed. So
            // `patch("mod.open")` is legal even when `mod` defines no `open` of its own.
            // Documented at
            // https://docs.python.org/3/library/unittest.mock.html#unittest.mock.patch ("If you
            // are patching builtins in a module then you don't need to pass `create=True`").
            if i + 1 == attrs.len()
                && base_ty.as_module().is_some()
                && !attr.starts_with('_')
                && self.exports.export_exists(ModuleName::builtins(), &name)
                && !self
                    .exports
                    .is_implicit_reexport(ModuleName::builtins(), &name)
            {
                return None;
            }
            on_class = matches!(base_ty, Type::ClassDef(_));
            base_ty = self.type_of_attr_get(
                &base_ty,
                &name,
                range,
                errors,
                ErrorKind::MissingAttributePatchTarget,
                None,
                "unittest.mock.patch target",
            );
        }
        let unbound_method = on_class
            && !matches!(base_ty, Type::BoundMethod(_))
            && base_ty.toplevel_func_metadata().is_some_and(|metadata| {
                !metadata.flags.is_staticmethod && !metadata.flags.is_classmethod
            });
        Some(UnittestMockTarget {
            signature: base_ty,
            unbound_method,
        })
    }

    /// Retain the assertion signature independently of the generated mock's call behavior.
    pub(super) fn specialize_unittest_mock_patch(
        &self,
        result: Type,
        target: UnittestMockTarget,
        arguments: &Arguments,
    ) -> Type {
        if arguments.find_keyword("new").is_some()
            || arguments.find_keyword("new_callable").is_some()
        {
            return result;
        }
        let autospec = match arguments.find_keyword("autospec").map(|kw| &kw.value) {
            None | Some(Expr::NoneLiteral(_)) => false,
            Some(Expr::BooleanLiteral(lit)) => lit.value,
            // An alternate or dynamic specification need not have the original target's signature.
            _ => return result,
        };
        for name in ["spec", "spec_set"] {
            if arguments.find_keyword(name).is_some_and(|kw| {
                !matches!(&kw.value, Expr::NoneLiteral(_) | Expr::BooleanLiteral(_))
            }) {
                return result;
            }
        }
        let mut signature = target.signature;
        let Some(metadata) = signature.toplevel_func_metadata() else {
            return result;
        };
        // Async generators do not return a coroutine and use MagicMock.
        let is_async = metadata.flags.is_async
            && signature
                .toplevel_callable_signatures()
                .any(|(callable, _)| {
                    matches!(&callable.ret, Type::ClassType(cls)
                    if cls.class_object() == self.stdlib.coroutine_object())
                });
        if let Type::BoundMethod(method) = &signature {
            let Some(bound) =
                self.bind_boundmethod(method, &mut |got, want| self.is_subset_eq(got, want))
            else {
                return result;
            };
            signature = bound;
        } else if target.unbound_method && !autospec {
            let mut valid = true;
            signature.transform_toplevel_callable_signatures(|callable, _| {
                if let Some(stripped) = callable.strip_first_param() {
                    *callable = stripped;
                } else {
                    valid = false;
                }
            });
            if !valid {
                return result;
            }
        }
        let Type::ClassType(mut patcher) = result else {
            return result;
        };
        if !patcher.has_qname("unittest.mock", "_patch_pass_arg") {
            return Type::ClassType(patcher);
        }
        let [mock] = patcher.targs_mut().as_mut() else {
            // An alternate stub can give the patcher a different generic shape.
            return Type::ClassType(patcher);
        };
        let Type::Union(mock_union) = mock else {
            return Type::ClassType(patcher);
        };
        let Some(class) = mock_union.members.iter().find_map(|ty| match ty {
            Type::ClassType(cls)
                if cls.has_qname(
                    "unittest.mock",
                    if is_async { "AsyncMock" } else { "MagicMock" },
                ) =>
            {
                Some(cls.clone())
            }
            _ => None,
        }) else {
            return Type::ClassType(patcher);
        };
        *mock = Type::UnittestMock(Box::new(UnittestMock {
            class,
            assertion_signature: signature,
        }));
        Type::ClassType(patcher)
    }

    /// Expose the retained signature only for assertion methods that accept expected arguments.
    pub(super) fn unittest_mock_assertion_signature(
        &self,
        base: &Type,
        attr_name: &Name,
    ) -> Option<Type> {
        let Type::UnittestMock(mock) = base else {
            return None;
        };
        match attr_name.as_str() {
            "assert_called_with" | "assert_called_once_with" | "assert_any_call" => {}
            "assert_awaited_with" | "assert_awaited_once_with" | "assert_any_await"
                if mock.class.has_qname("unittest.mock", "AsyncMock") => {}
            _ => return None,
        }
        let mut signature = mock.assertion_signature.clone();
        signature.transform_toplevel_callable_signatures(|callable, _| callable.ret = Type::None);
        let metadata = signature.toplevel_func_metadata_mut()?;
        metadata.flags = FuncFlags {
            is_unittest_mock_assertion: true,
            has_gradual_variadic_params: metadata.flags.has_gradual_variadic_params,
            ..FuncFlags::default()
        };
        Some(signature)
    }

    /// Check expected arguments while permitting objects with custom equality.
    pub(super) fn call_unittest_mock_assertion(
        &self,
        signature: &Type,
        args: &[CallArg],
        kws: &[CallKeyword],
        call: &ExprCall,
        errors: &ErrorCollector,
    ) -> Type {
        let owner = Owner::new();
        let prepare = |value: TypeOrExpr| {
            let range = value.range();
            let mut ty = value.infer(self, errors);
            self.expand_mut(&mut ty);
            // A custom equality method can match values outside the matcher's own type.
            // Check each argument separately so one matcher does not hide other invalid arguments.
            let has_matcher = ty.any(|ty: &Type| {
                matches!(ty, Type::ClassType(_))
                    && self
                        .type_of_magic_dunder_attr(
                            ty,
                            &dunder::EQ,
                            range,
                            &self.error_swallower(),
                            None,
                            "mock assertion equality matcher",
                            false,
                        )
                        .is_some_and(|eq| {
                            eq.to_func_kind()
                                .is_some_and(|kind| kind.module_name() != ModuleName::builtins())
                        })
            });
            if has_matcher {
                ty = self.heap.mk_any_explicit();
            }
            TypeOrExpr::Type(owner.push(ty), range)
        };
        let args = args
            .iter()
            .map(|arg| match arg {
                CallArg::Arg(value) => CallArg::Arg(prepare(*value)),
                CallArg::Star(value, range) => CallArg::Star(prepare(*value), *range),
            })
            .collect::<Vec<_>>();
        let kws = kws
            .iter()
            .map(|kw| CallKeyword {
                range: kw.range,
                arg: kw.arg,
                value: prepare(kw.value),
            })
            .collect::<Vec<_>>();
        let call_errors = self.error_collector();
        self.freeform_call_infer(
            signature.clone(),
            &args,
            &kws,
            call.func.range(),
            call.arguments.range(),
            None,
            &call_errors,
        );
        errors.extend_hard_with_kind(call_errors, ErrorKind::InvalidMockAssertion);
        Type::None
    }
}
