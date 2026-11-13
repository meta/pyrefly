/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Parsing of legacy `TypeVar`, `ParamSpec`, and `TypeVarTuple` declarations, and validation
//! of type variable defaults.

use ruff_python_ast::Expr;
use ruff_python_ast::ExprCall;
use ruff_python_ast::Identifier;
use ruff_python_ast::name::Name;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::solve::TypeFormContext;
use crate::config::error_kind::ErrorKind;
use crate::error::collector::ErrorCollector;
use crate::types::literal::Lit;
use crate::types::quantified::QuantifiedKind;
use crate::types::quantified::TypeVarLike;
use crate::types::type_var::PreInferenceVariance;
use crate::types::type_var::Restriction;
use crate::types::types::Type;

/// The arguments to a legacy `TypeVar`, `ParamSpec`, or `TypeVarTuple` call.
pub struct TypeVarLikeArgs {
    pub restriction: Restriction,
    pub default: Option<Type>,
    pub variance: PreInferenceVariance,
}

/// The variance declared by a variance keyword argument to a legacy type variable.
fn keyword_variance(keyword: &str) -> Option<PreInferenceVariance> {
    match keyword {
        "covariant" => Some(PreInferenceVariance::Covariant),
        "contravariant" => Some(PreInferenceVariance::Contravariant),
        "invariant" => Some(PreInferenceVariance::Invariant),
        "infer_variance" => Some(PreInferenceVariance::Undefined),
        _ => None,
    }
}

/// The error kind for problems with the declaration of a type variable of the given kind.
fn quantified_kind_error(kind: QuantifiedKind) -> ErrorKind {
    match kind {
        QuantifiedKind::TypeVar | QuantifiedKind::IntVar => ErrorKind::InvalidTypeVar,
        QuantifiedKind::ParamSpec => ErrorKind::InvalidParamSpec,
        QuantifiedKind::TypeVarTuple => ErrorKind::InvalidTypeVarTuple,
    }
}

impl<'ctx, 'answer, Ans: LookupAnswer> AnswersSolver<'ctx, 'answer, Ans> {
    /// Parse the arguments to a legacy `TypeVar`, `ParamSpec`, or `TypeVarTuple` call.
    pub fn type_var_like_args_from_call(
        &self,
        name: &Identifier,
        x: &ExprCall,
        kind: QuantifiedKind,
        errors: &ErrorCollector,
    ) -> TypeVarLikeArgs {
        let error_kind = quantified_kind_error(kind);
        let accepts_restriction = matches!(kind, QuantifiedKind::TypeVar | QuantifiedKind::IntVar);
        let mut arg_name = false;
        let mut restriction = None;
        let mut default: Option<&Expr> = None;
        let mut variance = None;

        let check_name_arg = |arg: &Expr| {
            if let Expr::StringLiteral(lit) = arg {
                if lit.value.to_str() != name.id.as_str() {
                    self.error(
                        errors,
                        x.range(),
                        error_kind,
                        format!(
                            "{kind} must be assigned to a variable named `{}`",
                            lit.value.to_str()
                        ),
                    );
                }
            } else {
                self.error(
                    errors,
                    arg.range(),
                    error_kind,
                    format!("Expected first argument of {kind} to be a string literal"),
                );
            }
        };

        let mut iargs = x.arguments.args.iter();
        if let Some(arg) = iargs.next() {
            check_name_arg(arg);
            arg_name = true;
        }

        if accepts_restriction {
            let constraints: Vec<Type> = iargs
                .map(|arg| self.expr_untype(arg, TypeFormContext::TypeVarConstraint, errors))
                .collect();
            if !constraints.is_empty() {
                restriction = Some(Restriction::Constraints(constraints));
            }
        } else {
            for arg in iargs {
                self.error(
                    errors,
                    arg.range(),
                    error_kind,
                    format!("Unexpected positional argument to {kind}"),
                );
            }
        }

        for kw in &x.arguments.keywords {
            match &kw.arg {
                Some(id) => match id.id.as_str() {
                    "bound" if accepts_restriction => {
                        let bound =
                            self.expr_untype(&kw.value, TypeFormContext::TypeVarConstraint, errors);
                        if restriction.is_some() {
                            self.error(
                                errors,
                                kw.range,
                                error_kind,
                                format!("{kind} cannot have both constraints and bound"),
                            );
                            restriction = Some(Restriction::Unrestricted);
                        } else if self.reject_legacy_shape_extension_bound(
                            &bound,
                            kw.value.range(),
                            errors,
                        ) {
                            restriction = Some(Restriction::Unrestricted);
                        } else {
                            restriction = Some(Restriction::Bound(bound));
                        }
                    }
                    "default" => {
                        default = Some(&kw.value);
                    }
                    keyword
                        if kind != QuantifiedKind::TypeVarTuple
                            && let Some(new_variance) = keyword_variance(keyword) =>
                    {
                        if self.literal_bool_infer(&kw.value, errors) {
                            if variance.is_some() {
                                self.error(
                                    errors,
                                    kw.range,
                                    error_kind,
                                    "Contradictory variance specifications".to_owned(),
                                );
                            } else {
                                variance = Some(new_variance);
                            }
                        }
                    }
                    "name" => {
                        if arg_name {
                            self.error(
                                errors,
                                kw.range,
                                error_kind,
                                "Multiple values for argument `name`".to_owned(),
                            );
                        } else {
                            check_name_arg(&kw.value);
                            arg_name = true;
                        }
                    }
                    _ => {
                        self.error(
                            errors,
                            kw.range,
                            error_kind,
                            format!("Unexpected keyword argument `{}` to {kind}", id.id),
                        );
                    }
                },
                _ => {
                    self.error(
                        errors,
                        kw.range,
                        error_kind,
                        format!("Cannot pass unpacked keyword arguments to {kind}"),
                    );
                }
            }
        }

        if !arg_name {
            self.error(
                errors,
                x.range(),
                error_kind,
                "Missing `name` argument".to_owned(),
            );
        }
        // If we ended up with a single constraint, emit an error and treat as unrestricted.
        if let Some(Restriction::Constraints(cs)) = &restriction
            && cs.len() < 2
        {
            self.error(
                errors,
                x.range(),
                error_kind,
                format!(
                    "Expected at least 2 constraints in {kind} `{}`, got {}",
                    name.id,
                    cs.len(),
                ),
            );
            restriction = Some(Restriction::Unrestricted);
        }
        let restriction = restriction.unwrap_or(Restriction::Unrestricted);
        let default = default.map(|default_expr| {
            let default_ty = if let Some(default) =
                self.parse_int_tuple_type_var_default(default_expr, &restriction, errors)
            {
                default
            } else {
                self.expr_untype(
                    default_expr,
                    TypeFormContext::quantified_kind_default(kind),
                    errors,
                )
            };
            self.validate_type_var_default(
                &name.id,
                kind,
                &default_ty,
                default_expr.range(),
                &restriction,
                errors,
            )
        });
        TypeVarLikeArgs {
            restriction,
            default,
            variance: variance.unwrap_or(PreInferenceVariance::Invariant),
        }
    }

    fn literal_bool_infer(&self, x: &Expr, errors: &ErrorCollector) -> bool {
        let ty = self.expr_infer(x, errors);
        match ty {
            Type::Literal(lit) if let Lit::Bool(b) = lit.value => b,
            _ => {
                self.error(
                    errors,
                    x.range(),
                    ErrorKind::InvalidLiteral,
                    format!(
                        "Expected literal `True` or `False`, got `{}`",
                        self.for_display(ty)
                    ),
                );
                false
            }
        }
    }

    pub fn validate_type_var_default(
        &self,
        name: &Name,
        kind: QuantifiedKind,
        default: &Type,
        range: TextRange,
        restriction: &Restriction,
        errors: &ErrorCollector,
    ) -> Type {
        if default.is_error() {
            return default.clone();
        }
        if let Some(default) = self.validate_shape_extension_type_parameter_default(
            name,
            default,
            range,
            restriction,
            errors,
        ) {
            return default;
        }
        // A shape extension may apply specialized validation above. Extensions without one use
        // the same safe fallback as an ordinary bound.
        let upper_bound = match restriction {
            Restriction::Bound(bound) => Some(bound.clone()),
            Restriction::ShapeExtension(extension) => {
                Some(extension.upper_bound(self.stdlib, self.heap))
            }
            Restriction::Constraints(_) | Restriction::Unrestricted => None,
        };
        if let Some(bound_ty) = upper_bound {
            // Default must be a subtype of the upper bound.
            // Per PEP 696: when default is a TypeVar, "T1's bound must be a subtype of T2's bound"
            let default_for_check = match default {
                Type::TypeVar(tv) => tv.upper_bound(self.stdlib, self.heap),
                Type::Quantified(q) if q.is_type_var() => q.upper_bound(self.stdlib, self.heap),
                _ => default.clone(),
            };
            if !self.is_subset_eq(&default_for_check, &bound_ty) {
                self.error(
                    errors,
                    range,
                    quantified_kind_error(kind),
                    format!(
                        "Expected default `{default}` of `{name}` to be assignable to the upper bound of `{bound_ty}`",
                    ),
                );
                return self.heap.mk_any_error();
            }
        } else if let Restriction::Constraints(constraints) = restriction {
            // Per PEP 696: when default is a TypeVar, "the constraints of T2 must be a
            // superset of the constraints of T1". A bounded or unrestricted TypeVar cannot
            // be a valid default for a constrained TypeVar since it can't guarantee an
            // exact constraint match.
            let valid = match default {
                Type::TypeVar(tv) => match tv.restriction() {
                    Restriction::Constraints(default_constraints) => default_constraints
                        .iter()
                        .all(|dc| constraints.iter().any(|c| self.is_consistent(c, dc))),
                    Restriction::Bound(_)
                    | Restriction::ShapeExtension(_)
                    | Restriction::Unrestricted => false,
                },
                Type::Quantified(q) if q.is_type_var() => match q.restriction() {
                    Restriction::Constraints(default_constraints) => default_constraints
                        .iter()
                        .all(|dc| constraints.iter().any(|c| self.is_consistent(c, dc))),
                    Restriction::Bound(_)
                    | Restriction::ShapeExtension(_)
                    | Restriction::Unrestricted => false,
                },
                _ => constraints.iter().any(|c| self.is_consistent(c, default)),
            };
            if !valid {
                let formatted_constraints = constraints
                    .iter()
                    .map(|x| format!("`{x}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.error(
                    errors,
                    range,
                    quantified_kind_error(kind),
                    format!(
                        "Expected default `{default}` of `{name}` to be one of the following constraints: {formatted_constraints}"
                    ),
                );
                return self.heap.mk_any_error();
            }
        }
        match kind {
            QuantifiedKind::ParamSpec => {
                if default.is_kind_param_spec() {
                    default.clone()
                } else {
                    self.error(
                        errors,
                        range,
                        ErrorKind::InvalidParamSpec,
                        format!("Default for `ParamSpec` must be a parameter list, `...`, or another `ParamSpec`, got `{default}`"),
                    );
                    self.heap.mk_any_error()
                }
            }
            QuantifiedKind::TypeVarTuple => {
                if let Type::Unpack(inner) = default
                    && (matches!(&**inner, Type::Tuple(_)) || inner.is_kind_type_var_tuple())
                {
                    (**inner).clone()
                } else {
                    self.error(
                        errors,
                        range,
                        ErrorKind::InvalidTypeVarTuple,
                        format!("Default for `TypeVarTuple` must be an unpacked tuple form or another `TypeVarTuple`, got `{default}`"),
                    );
                    self.heap.mk_any_error()
                }
            }
            QuantifiedKind::TypeVar | QuantifiedKind::IntVar => {
                if default.is_kind_param_spec() || default.is_kind_type_var_tuple() {
                    self.error(
                        errors,
                        range,
                        ErrorKind::InvalidTypeVar,
                        format!(
                            "Default for `{kind}` may not be a `TypeVarTuple` or `ParamSpec`, got `{default}`"
                        ),
                    );
                    self.heap.mk_any_error()
                } else {
                    default.clone()
                }
            }
        }
    }
}
