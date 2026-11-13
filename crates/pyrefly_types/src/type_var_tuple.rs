/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fmt;
use std::fmt::Display;

use dupe::Dupe;
use pyrefly_derive::TypeEq;
use pyrefly_python::module::Module;
use pyrefly_python::nesting_context::NestingContext;
use pyrefly_python::qname::QName;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::visit::Visit;
use pyrefly_util::visit::VisitMut;
use ruff_python_ast::Identifier;

use crate::equality::TypeEq;
use crate::equality::TypeEqCtx;
use crate::heap::TypeHeap;
use crate::quantified::QuantifiedKind;
use crate::quantified::TypeVarLike;
use crate::type_var::PreInferenceVariance;
use crate::type_var::Restriction;
use crate::types::Type;

/// Used to represent TypeVarTuple calls. Each TypeVarTuple is unique, so use the ArcId to separate them.
#[derive(Clone, Dupe, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct TypeVarTuple(ArcId<TypeVarTupleInner>);

impl Visit<Type> for TypeVarTuple {
    const RECURSE_CONTAINS: bool = false;
    fn recurse<'a>(&'a self, _: &mut dyn FnMut(&'a Type)) {}
}

impl VisitMut<Type> for TypeVarTuple {
    const RECURSE_CONTAINS: bool = false;
    fn recurse_mut(&mut self, _: &mut dyn FnMut(&mut Type)) {}
}

impl Display for TypeVarTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.qname.id())
    }
}

#[derive(Debug, PartialEq, TypeEq, Eq, Ord, PartialOrd)]
struct TypeVarTupleInner {
    qname: QName,
    default: Option<Type>,
    variance: PreInferenceVariance,
}

impl TypeVarLike for TypeVarTuple {
    fn qname(&self) -> &QName {
        &self.0.qname
    }

    fn kind(&self) -> QuantifiedKind {
        QuantifiedKind::TypeVarTuple
    }

    fn default(&self) -> Option<&Type> {
        self.0.default.as_ref()
    }

    fn restriction(&self) -> &Restriction {
        &Restriction::Unrestricted
    }

    fn variance(&self) -> PreInferenceVariance {
        self.0.variance
    }
}

impl TypeVarTuple {
    pub fn new(
        name: Identifier,
        module: Module,
        default: Option<Type>,
        variance: PreInferenceVariance,
    ) -> Self {
        Self(ArcId::new(TypeVarTupleInner {
            // TODO: properly take parent from caller of new()
            qname: QName::new(name, NestingContext::toplevel(), module),
            default,
            variance,
        }))
    }

    pub fn to_type(&self, heap: &TypeHeap) -> Type {
        heap.mk_type_var_tuple(self.dupe())
    }

    pub fn type_eq_inner(&self, other: &Self, ctx: &mut TypeEqCtx) -> bool {
        self.0.type_eq(&other.0, ctx)
    }
}
