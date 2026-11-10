/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_python::module_name::ModuleName;
use pyrefly_types::function::FunctionKind;
use pyrefly_types::known_class::KnownClass;
use pyrefly_types::type_alias::TypeAliasData;
use pyrefly_util::suggest::Candidate;
use pyrefly_util::suggest::best_suggestion;
use ruff_python_ast::Arguments;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprCall;
use ruff_python_ast::name::Name;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use starlark_map::small_set::SmallSet;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::callable::CallArg;
use crate::alt::callable::CallKeyword;
use crate::alt::callable::CallWithTypes;
use crate::alt::unwrap::HintRef;
use crate::config::error_kind::ErrorKind;
use crate::error::collector::ErrorCollector;
use crate::types::class::Class;
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
            && class_type.is(KnownClass::SqlAlchemyMappedColumn)
            && let Some(slot) = class_type.targs_mut().as_mut().get_mut(0)
        {
            *slot = python_type;
        }
        ret
    }

    fn python_type_from_type_engine_arg(&self, source: &Expr, ty: &Type) -> Option<Type> {
        // Enum's Python value type is available at its constructor, not its String base.
        if let Type::ClassType(cls) = ty
            && cls.is(KnownClass::SqlAlchemyEnum)
            && let Expr::Call(call) = source
            && matches!(
                self.expr_infer(&call.func, &self.error_swallower()),
                Type::ClassDef(cls)
                    if cls.is(KnownClass::SqlAlchemyEnum)
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
        if cls.is(KnownClass::SqlAlchemyEnum)
            || mro
                .ancestors_no_object()
                .iter()
                .any(|ancestor| ancestor.is(KnownClass::SqlAlchemyEnum))
        {
            return None;
        }
        // Concrete SQL subclasses supply the value type; bare TypeEngine stays gradual.
        let type_engine = mro
            .ancestors(self.stdlib)
            .find(|ancestor| ancestor.class_object().is(KnownClass::SqlAlchemyTypeEngine))?;
        self.as_superclass(cls, type_engine.class_object())?
            .targs()
            .as_slice()
            .first()
            .cloned()
    }

    pub(super) fn is_sqlalchemy_mapped_annotation(ty: &Type) -> bool {
        match ty {
            Type::ClassType(cls) => {
                cls.is(KnownClass::SqlAlchemyMapped) || cls.is(KnownClass::SqlAlchemyBaseMapped)
            }
            Type::Annotated(inner, _) => Self::is_sqlalchemy_mapped_annotation(inner),
            _ => false,
        }
    }

    /// Validate SQLAlchemy keyword filters and updates against the `Mapped[...]` fields on the
    /// model that the statement targets.
    pub(super) fn check_sqlalchemy_keyword_call(&self, x: &ExprCall, errors: &ErrorCollector) {
        let Expr::Attribute(attr_expr) = &*x.func else {
            return;
        };
        let (model, statement_name) = match attr_expr.attr.id.as_str() {
            "values" => (
                self.sqlalchemy_model_from_chain(&attr_expr.value, "update"),
                "update",
            ),
            "filter_by" => (
                self.sqlalchemy_model_from_chain(&attr_expr.value, "select"),
                "filter_by",
            ),
            _ => return,
        };
        let Some(model) = model else { return };
        let mapped_fields = self.sqlalchemy_mapped_model_fields(&model);
        if mapped_fields.is_empty() {
            return;
        }
        for keyword in &x.arguments.keywords {
            let Some(field_identifier) = keyword.arg.as_ref() else {
                continue;
            };
            let field_name = &field_identifier.id;
            if mapped_fields.contains(field_name) {
                let expected_ty = self.sqlalchemy_mapped_field_type(&model, field_name);
                let got_ty = self.expr_infer(&keyword.value, errors);
                if !self.is_subset_eq(&got_ty, &expected_ty)
                    && !Self::is_sqlalchemy_owned_type(&got_ty)
                {
                    self.error(
                        errors,
                        keyword.value.range(),
                        ErrorKind::BadArgumentType,
                        format!(
                            "`{}` is not assignable to field `{field_name}` with type `{}`",
                            self.for_display(got_ty),
                            self.for_display(expected_ty)
                        ),
                    );
                }
            } else {
                let mut builder = errors.error_builder(
                    field_identifier.range(),
                    ErrorKind::UnexpectedKeyword,
                    format!("Unexpected SQLAlchemy {statement_name} field `{field_name}`"),
                );
                if let Some(suggestion) = best_suggestion(
                    field_name,
                    mapped_fields
                        .iter()
                        .map(|candidate| Candidate::measured(candidate, 0)),
                ) {
                    builder = builder.with_detail(format!("Did you mean `{suggestion}`?"));
                }
                builder.emit();
            }
        }
    }

    fn sqlalchemy_model_from_chain(&self, expr: &Expr, function_name: &str) -> Option<Class> {
        let Expr::Call(call) = expr else {
            return None;
        };
        if let Some(model) = self.sqlalchemy_model_from_call(call, function_name) {
            return Some(model);
        }
        let Expr::Attribute(attr_expr) = &*call.func else {
            return None;
        };
        // Joins and explicit FROM clauses can change the namespace used by `filter_by`.
        if matches!(
            attr_expr.attr.id.as_str(),
            "join" | "outerjoin" | "join_from" | "outerjoin_from" | "select_from"
        ) {
            return None;
        }
        self.sqlalchemy_model_from_chain(&attr_expr.value, function_name)
    }

    fn sqlalchemy_model_from_call(&self, call: &ExprCall, function_name: &str) -> Option<Class> {
        let errors = self.error_swallower();
        let func_ty = self.expr_infer(&call.func, &errors);
        let Some(FunctionKind::Def(func_id)) = func_ty.to_func_kind() else {
            return None;
        };
        if func_id.qname.id().as_str() != function_name
            || !Self::is_sqlalchemy_module(func_id.qname.module_name())
        {
            return None;
        }
        let model_expr = call.arguments.args.first()?;
        match self.expr_infer(model_expr, &errors) {
            Type::ClassDef(cls) => Some(cls),
            Type::Type(boxed) => match *boxed {
                Type::ClassType(cls) => Some(cls.into_class_object()),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_sqlalchemy_module(module: ModuleName) -> bool {
        let name = module.as_str();
        // The `.` keeps unrelated distributions such as `sqlalchemy_utils` out.
        name == "sqlalchemy" || name.starts_with("sqlalchemy.")
    }

    /// SQLAlchemy accepts a SQL expression anywhere a column value is expected, so
    /// a value the library itself produced is not checked against the mapped type.
    fn is_sqlalchemy_owned_type(ty: &Type) -> bool {
        match ty {
            Type::ClassType(cls) => Self::is_sqlalchemy_module(cls.qname().module_name()),
            Type::Annotated(inner, _) => Self::is_sqlalchemy_owned_type(inner),
            Type::Union(union) => union.members.iter().any(Self::is_sqlalchemy_owned_type),
            _ => false,
        }
    }

    fn sqlalchemy_mapped_model_fields(&self, model: &Class) -> SmallSet<Name> {
        // Find the SQLModel table model in the mro if there is one
        let sqlmodel_table = self
            .get_mro_for_class(model)
            .ancestors_no_object()
            .iter()
            .rev()
            .map(|cls| cls.class_object())
            .chain(std::iter::once(model))
            .skip_while(|cls| !(cls.is(KnownClass::SqlModel) || cls.is(KnownClass::SqlModelMain)))
            .find(|cls| {
                self.get_metadata_for_class(cls)
                    .keywords()
                    .iter()
                    .any(|(name, typ)| {
                        name == "table"
                            && matches!(typ, Type::Literal(l) if l.value==Lit::Bool(true))
                    })
            });

        // If there is a SQLModel table model we want to ignore the fields below it in the mro
        let find_fields_from = sqlmodel_table.unwrap_or(model);

        let mut seen_fields = SmallSet::new();
        std::iter::once(find_fields_from)
            .chain(
                self.get_mro_for_class(find_fields_from)
                    .ancestors_no_object()
                    .iter()
                    .map(|ancestor| ancestor.class_object()),
            )
            .flat_map(|class| self.get_class_field_map(class))
            .filter_map(|(name, field)| {
                if !seen_fields.insert(name.clone())
                    || name.as_str().starts_with('_')
                    || field.is_class_var()
                {
                    return None;
                }
                let (_, annotation, _) = field.for_variance_inference();
                let is_mapped_annotation = annotation.is_some_and(|annotation| {
                    Self::is_sqlalchemy_mapped_annotation(annotation.get_type())
                });
                (is_mapped_annotation || (sqlmodel_table.is_some() && annotation.is_some()))
                    .then_some(name)
            })
            .collect()
    }

    fn sqlalchemy_mapped_field_type(&self, model: &Class, field_name: &Name) -> Type {
        let model_ty = self.instantiate(model);
        let errors = self.error_swallower();
        self.attr_infer_for_type(&model_ty, field_name, TextRange::default(), &errors, None)
    }
}
