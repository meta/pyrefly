/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Access to the definition data that types refer to.

use crate::class::Class;
use crate::class::ClassDef;
use crate::quantified::Quantified;
use crate::stdlib::Stdlib;
use crate::type_var::Restriction;
use crate::types::TParams;
use crate::types::Type;

/// Read access to definition data that `Type` refers to but does not contain.
/// Every context that holds types can provide one: the solver, a transaction
/// after solving, and tests. Returned borrows live as long as both the env
/// borrow and the argument borrow, so the same signatures work whether the data
/// is stored in the type or in a table owned by the env.
pub trait TypeEnv {
    fn stdlib(&self) -> &Stdlib;

    /// Binding-time data of a class definition. The default reads the data stored in the
    /// `Class` itself.
    fn class_def<'a>(&'a self, cls: &'a Class) -> &'a ClassDef {
        cls.inline_def()
    }

    /// A class's type parameters, which are empty if the class is not generic.
    fn class_tparams<'a>(&'a self, cls: &'a Class) -> &'a TParams;

    /// A type variable's bound or constraints. The default reads the restriction stored in the
    /// `Quantified` itself.
    fn type_var_restriction<'a>(&'a self, q: &'a Quantified) -> &'a Restriction {
        q.inline_restriction()
    }

    /// A type variable's default. The default reads the default stored in the `Quantified`
    /// itself.
    fn type_var_default<'a>(&'a self, q: &'a Quantified) -> Option<&'a Type> {
        q.inline_default()
    }
}
