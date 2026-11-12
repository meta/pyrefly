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
    test_legacy_paramspec_accepts_variance_keyword,
    r#"
from typing import ParamSpec
P = ParamSpec("P", covariant=True)
"#,
);

testcase!(
    test_legacy_paramspec_declared_variance_used_for_assignability,
    r#"
from typing import Generic, ParamSpec

P = ParamSpec("P", contravariant=True)

class A(Generic[P]):
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...

def f(obj: A[[object]], int_: A[[int]]):
    x: A[[int]] = obj
    y: A[[object]] = int_  # E: `A[[int]]` is not assignable to `A[[object]]`
"#,
);

testcase!(
    test_covariant_paramspec_used_in_paramspec_components,
    r#"
from typing import Generic, ParamSpec

P = ParamSpec("P", covariant=True)

class A(Generic[P]):
    def __call__(self, *args: P.args, **kwargs: P.kwargs) -> None: ...  # E: `P` is covariant but is used in contravariant position
"#,
);

testcase!(
    test_contravariant_paramspec_used_in_callable_parameter,
    r#"
from typing import Callable, Generic, ParamSpec

P = ParamSpec("P", contravariant=True)

class A(Generic[P]):
    def accept(self, fn: Callable[P, None]) -> None: ...  # E: `P` is contravariant but is used in covariant position
"#,
);

testcase!(
    test_covariant_paramspec_used_in_callable_parameter_in_return,
    r#"
from typing import Callable, Generic, ParamSpec

P_co = ParamSpec("P_co", covariant=True)

class C(Generic[P_co]):
    def f(self) -> Callable[P_co, int]: ...  # E: `P_co` is covariant but is used in contravariant position
    "#,
);

testcase!(
    test_user_defined_paramspec_class,
    r#"
from typing import Generic, ParamSpec

P_co = ParamSpec("P_co", covariant=True)
P_contra = ParamSpec("P_contra", contravariant=True)

class C_co(Generic[P_co]):
    pass

class C_contra(Generic[P_contra]):
    pass

class D_co(Generic[P_co]):
    def f(self, c: C_co[P_co]):  # E: `P_co` is covariant but is used in contravariant position
        pass
    def g(self, c: C_contra[P_co]):
        pass

class D_contra(Generic[P_contra]):
    def f(self, c: C_co[P_contra]):
        pass
    def g(self, c: C_contra[P_contra]):  # E: `P_contra` is contravariant but is used in covariant position
        pass
    "#,
);

testcase!(
    test_concatenate_in_user_defined_class,
    r#"
from typing import Concatenate, Generic, ParamSpec

P_co = ParamSpec("P_co", covariant=True)

class C(Generic[P_co]):
    def f(self, c: C[Concatenate[int, P_co]]): ...  # E: `P_co` is covariant but is used in contravariant position
    "#,
);
