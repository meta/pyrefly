/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_types::type_alias::TypeAliasData;
use ruff_python_ast::Arguments;
use ruff_python_ast::Expr;
use ruff_text_size::TextRange;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::callable::CallArg;
use crate::alt::callable::CallKeyword;
use crate::alt::callable::CallWithTypes;
use crate::alt::unwrap::HintRef;
use crate::error::collector::ErrorCollector;
use crate::types::class::ClassType;
use crate::types::literal::Lit;
use crate::types::types::Type;

impl<'ctx, 'answer, Ans: LookupAnswer> AnswersSolver<'ctx, 'answer, Ans> {
    pub(super) fn call_sqlalchemy_mapped_column(
        &self,
        callee_ty: Type,
        args: &[CallArg],
        keywords: &[CallKeyword],
        callee_range: TextRange,
        arguments: &Arguments,
        hint: Option<HintRef>,
        errors: &ErrorCollector,
    ) -> Type {
        if keywords.iter().any(|keyword| keyword.arg.is_none())
            || hint.is_some_and(|hint| {
                hint.types()
                    .iter()
                    .any(Self::is_sqlalchemy_mapped_annotation)
            })
        {
            return self
                .freeform_call_infer(
                    callee_ty,
                    args,
                    keywords,
                    callee_range,
                    arguments.range,
                    hint,
                    errors,
                )
                .ty;
        }
        let call_types = CallWithTypes::new();
        let typed_args = call_types.vec_call_arg(args, self, errors);
        let typed_keywords = call_types.vec_call_keyword(keywords, self, errors);
        let mut ret = self
            .freeform_call_infer(
                callee_ty,
                &typed_args,
                &typed_keywords,
                callee_range,
                arguments.range,
                hint,
                errors,
            )
            .ty;
        let swallowed_errors = self.error_swallower();
        // CallWithTypes preserves source order; syntax is needed only for Enum constructors.
        let positional_type =
            arguments
                .args
                .iter()
                .zip(&typed_args)
                .take(2)
                .find_map(|(source, arg)| match arg {
                    CallArg::Arg(value) => self.python_type_from_type_engine_arg(
                        source,
                        &value.infer(self, &swallowed_errors),
                    ),
                    CallArg::Star(..) => None,
                });
        let Some(mut python_type) = positional_type.or_else(|| {
            arguments
                .keywords
                .iter()
                .zip(&typed_keywords)
                .find_map(|(source, keyword)| {
                    if keyword.arg.is_some_and(|name| name.as_str() == "type_") {
                        self.python_type_from_type_engine_arg(
                            &source.value,
                            &keyword.value.infer(self, &swallowed_errors),
                        )
                    } else {
                        None
                    }
                })
        }) else {
            return ret;
        };
        let flag = |name: &str| {
            typed_keywords
                .iter()
                .find(|keyword| keyword.arg.is_some_and(|arg| arg.as_str() == name))
                .map(|keyword| keyword.value.infer(self, &swallowed_errors))
        };
        let nullable = if let Some(nullable) = flag("nullable") {
            // Only a known false flag rules out null values.
            !matches!(nullable, Type::Literal(literal) if literal.value == Lit::Bool(false))
        } else {
            !matches!(flag("primary_key"), Some(Type::Literal(literal)) if literal.value == Lit::Bool(true))
        };
        if nullable {
            python_type = Type::optional(python_type);
        }
        if let Type::ClassType(class_type) = &mut ret
            && class_type.has_qname("sqlalchemy.orm.properties", "MappedColumn")
            && let Some(slot) = class_type.targs_mut().as_mut().get_mut(0)
        {
            *slot = python_type;
        }
        ret
    }

    fn python_type_from_type_engine_arg(&self, source: &Expr, ty: &Type) -> Option<Type> {
        // Enum's Python value type is available at its constructor, not its String base.
        if let Type::ClassType(cls) = ty
            && cls.has_qname("sqlalchemy.sql.sqltypes", "Enum")
            && let Expr::Call(call) = source
            && matches!(
                self.expr_infer(&call.func, &self.error_swallower()),
                Type::ClassDef(cls)
                    if cls.has_toplevel_qname("sqlalchemy.sql.sqltypes", "Enum")
            )
            && let Some(enum_class) = call.arguments.args.first()
        {
            return match self.expr_infer(enum_class, &self.error_swallower()) {
                Type::ClassDef(cls) => Some(self.promote_silently(&cls)),
                Type::Type(instance) => Some(*instance),
                _ => None,
            };
        }
        self.python_type_from_type_engine_type(ty)
    }

    fn python_type_from_type_engine_type(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::ClassType(cls) => self.python_type_from_type_engine_class(cls),
            Type::ClassDef(cls) => {
                let inst = self.promote_silently(cls);
                self.python_type_from_type_engine_type(&inst)
            }
            Type::Type(inner) => self.python_type_from_type_engine_type(inner),
            Type::TypeAlias(alias) => match alias.as_ref() {
                TypeAliasData::Value(alias) => {
                    self.python_type_from_type_engine_type(&alias.as_type())
                }
                TypeAliasData::Ref(_) => {
                    self.python_type_from_type_engine_type(&self.untype_alias(alias))
                }
            },
            Type::Union(u) => {
                let inferred = u
                    .members
                    .iter()
                    .map(|member| self.python_type_from_type_engine_type(member))
                    .collect::<Option<Vec<_>>>()?;
                Some(self.unions(inferred))
            }
            _ => None,
        }
    }

    fn python_type_from_type_engine_class(&self, cls: &ClassType) -> Option<Type> {
        let mro = self.get_mro_for_class(cls.class_object());
        // Enum's String base does not describe its Python enum values.
        if cls.has_qname("sqlalchemy.sql.sqltypes", "Enum")
            || mro
                .ancestors_no_object()
                .iter()
                .any(|ancestor| ancestor.has_qname("sqlalchemy.sql.sqltypes", "Enum"))
        {
            return None;
        }
        // Concrete SQL subclasses supply the value type; bare TypeEngine stays gradual.
        let type_engine = mro.ancestors(self.stdlib).find(|ancestor| {
            ancestor
                .class_object()
                .has_toplevel_qname("sqlalchemy.sql.type_api", "TypeEngine")
        })?;
        self.as_superclass(cls, type_engine.class_object())?
            .targs()
            .as_slice()
            .first()
            .cloned()
    }

    pub(super) fn is_sqlalchemy_mapped_annotation(ty: &Type) -> bool {
        match ty {
            Type::ClassType(cls) => {
                cls.has_qname("sqlalchemy.orm", "Mapped")
                    || cls.has_qname("sqlalchemy.orm.base", "Mapped")
            }
            Type::Annotated(inner, _) => Self::is_sqlalchemy_mapped_annotation(inner),
            _ => false,
        }
    }
}
