/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_derive::TypeEq;
use pyrefly_derive::Visit;
use pyrefly_derive::VisitMut;

use crate::class::ClassType;
use crate::types::Type;

/// A generated mock retains its own behavior and a separate signature for call assertions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Visit, VisitMut, TypeEq)]
pub struct UnittestMock {
    pub class: ClassType,
    pub assertion_signature: Type,
}
