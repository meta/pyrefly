/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for the variance of ParamSpec class type parameters.
//! See <https://typing.readthedocs.io/en/latest/spec/generics.html#variance-inference>.

use crate::testcase;

testcase!(
    test_infer_contravariant_from_paramspec_components,
    r#"
class A[**P]:
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj
    y: A[[object]] = int_  # E: `A[[int]]` is not assignable to `A[[object]]`
"#,
);

testcase!(
    test_infer_contravariant_from_returned_callable,
    r#"
from typing import Callable

class A[**P]:
    def get(self) -> Callable[P, None]: ...

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj
    y: A[[object]] = int_  # E: `A[[int]]` is not assignable to `A[[object]]`
"#,
);

testcase!(
    test_infer_covariant_from_callable_parameter,
    r#"
from typing import Callable

class A[**P]:
    def accept(self, fn: Callable[P, None]) -> None: ...

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj  # E: `A[[object]]` is not assignable to `A[[int]]`
    y: A[[object]] = int_
"#,
);

testcase!(
    test_infer_invariant_from_mutable_attribute,
    r#"
from typing import Callable

class A[**P]:
    fn: Callable[P, None]

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj  # E: `A[[object]]` is not assignable to `A[[int]]`
    y: A[[object]] = int_  # E: `A[[int]]` is not assignable to `A[[object]]`
"#,
);

testcase!(
    test_concatenate_prefix_preserves_variance,
    r#"
from typing import Callable, Concatenate

class A[**P]:
    def get(self) -> Callable[Concatenate[int, P], None]: ...

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj
    y: A[[object]] = int_  # E: `A[[int]]` is not assignable to `A[[object]]`
"#,
);

testcase!(
    test_infer_variance_from_base_class,
    r#"
class A[**P]:
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...

class B[**P](A[P]): ...

def f(obj: B[[object]], int_: B[[int]]):
    x: B[[int]] = obj
    y: B[[object]] = int_  # E: `B[[int]]` is not assignable to `B[[object]]`
"#,
);

testcase!(
    test_gradual_paramspec_argument,
    r#"
class A[**P]:
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...

def f(gradual: A[...], int_: A[[int]]):
    x: A[[int]] = gradual
    y: A[...] = int_
"#,
);

testcase!(
    bug = "A legacy ParamSpec should accept a variance keyword",
    test_legacy_paramspec_accepts_variance_keyword,
    r#"
from typing import ParamSpec

P = ParamSpec("P", covariant=True)  # E: Unexpected keyword argument `covariant` to ParamSpec
"#,
);

testcase!(
    bug = "A legacy ParamSpec's declared variance should be used for assignability",
    test_legacy_paramspec_declared_variance_used_for_assignability,
    r#"
from typing import Generic, ParamSpec

P = ParamSpec("P", contravariant=True)  # E: Unexpected keyword argument `contravariant` to ParamSpec

class A(Generic[P]):
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj  # E: `A[[object]]` is not assignable to `A[[int]]`
    y: A[[object]] = int_  # E: `A[[int]]` is not assignable to `A[[object]]`
"#,
);

testcase!(
    bug = "Declared ParamSpec variance should be checked against ParamSpec components",
    test_covariant_paramspec_used_in_paramspec_components,
    r#"
from typing import Generic, ParamSpec

P = ParamSpec("P", covariant=True)  # E: Unexpected keyword argument `covariant` to ParamSpec

class A(Generic[P]):
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...
"#,
);

testcase!(
    bug = "Declared ParamSpec variance should be checked against Callable parameters",
    test_contravariant_paramspec_used_in_callable_parameter,
    r#"
from typing import Callable, Generic, ParamSpec

P = ParamSpec("P", contravariant=True)  # E: Unexpected keyword argument `contravariant` to ParamSpec

class A(Generic[P]):
    def accept(self, fn: Callable[P, None]) -> None: ...
"#,
);
