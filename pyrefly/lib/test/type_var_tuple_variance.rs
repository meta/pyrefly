/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for the variance of TypeVarTuple class type parameters.
//! See <https://typing.readthedocs.io/en/latest/spec/generics.html#variance-inference>.

use crate::testcase;

testcase!(
    test_infer_covariant_from_returned_tuple,
    r#"
class A[*Ts]:
    def get(self) -> tuple[*Ts]: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_
"#,
);

testcase!(
    test_infer_contravariant_from_tuple_parameter,
    r#"
class A[*Ts]:
    def put(self, t: tuple[*Ts]) -> None: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    test_infer_invariant_from_mutable_attribute,
    r#"
class A[*Ts]:
    x: tuple[*Ts]

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    test_prefix_and_suffix_preserve_variance,
    r#"
class A[*Ts]:
    def get(self) -> tuple[int, *Ts, str]: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_
"#,
);

testcase!(
    test_infer_variance_from_base_class,
    r#"
class A[*Ts]:
    def get(self) -> tuple[*Ts]: ...

class B[*Ts](A[*Ts]): ...

def f(obj: B[object], int_: B[int]):
    x: B[int] = obj  # E: `B[object]` is not assignable to `B[int]`
    y: B[object] = int_
"#,
);

testcase!(
    test_covariance_checks_each_element,
    r#"
class A[*Ts]:
    def get(self) -> tuple[*Ts]: ...

def f(a: A[bool, bool], b: A[bool, object]):
    x: A[int, int] = a
    y: A[int, int] = b  # E: `A[bool, object]` is not assignable to `A[int, int]`
"#,
);

testcase!(
    test_covariance_with_unbounded_tuple_argument,
    r#"
class A[*Ts]:
    def get(self) -> tuple[*Ts]: ...

def f(obj: A[*tuple[object, ...]], int_: A[*tuple[int, ...]]):
    x: A[*tuple[int, ...]] = obj  # E: `A[*tuple[object, ...]]` is not assignable to `A[*tuple[int, ...]]`
    y: A[*tuple[object, ...]] = int_
"#,
);

testcase!(
    test_covariance_between_fixed_and_unbounded_arguments,
    r#"
class A[*Ts]:
    def get(self) -> tuple[*Ts]: ...

def f(fixed: A[int], unbounded: A[*tuple[int, ...]]):
    x: A[*tuple[int, ...]] = fixed
    y: A[int] = unbounded  # E: `A[*tuple[int, ...]]` is not assignable to `A[int]`
"#,
);

testcase!(
    test_legacy_type_var_tuple_is_invariant,
    r#"
from typing import Generic, TypeVarTuple

Ts = TypeVarTuple("Ts")

class A(Generic[*Ts]):
    def get(self) -> tuple[*Ts]: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    bug = "A legacy TypeVarTuple should accept a variance keyword",
    test_legacy_type_var_tuple_accepts_variance_keyword,
    r#"
from typing import TypeVarTuple

Ts = TypeVarTuple("Ts", covariant=True)  # E: Unexpected keyword argument `covariant` to TypeVarTuple
"#,
);

testcase!(
    bug = "A legacy TypeVarTuple should accept `infer_variance`",
    test_legacy_type_var_tuple_accepts_infer_variance_keyword,
    r#"
from typing import TypeVarTuple

Ts = TypeVarTuple("Ts", infer_variance=True)  # E: Unexpected keyword argument `infer_variance` to TypeVarTuple
"#,
);

testcase!(
    bug = "A legacy TypeVarTuple's declared variance should be used for assignability",
    test_legacy_type_var_tuple_declared_variance_used_for_assignability,
    r#"
from typing import Generic, TypeVarTuple

Ts = TypeVarTuple("Ts", contravariant=True)  # E: Unexpected keyword argument `contravariant` to TypeVarTuple

class A(Generic[*Ts]):
    def put(self, t: tuple[*Ts]) -> None: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    bug = "A legacy TypeVarTuple with `infer_variance` should have its variance inferred",
    test_legacy_type_var_tuple_infer_variance,
    r#"
from typing import Generic, TypeVarTuple

Ts = TypeVarTuple("Ts", infer_variance=True)  # E: Unexpected keyword argument `infer_variance` to TypeVarTuple

class A(Generic[*Ts]):
    def get(self) -> tuple[*Ts]: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    bug = "Unpacked `*args` should make a TypeVarTuple contravariant",
    test_infer_contravariant_from_unpacked_varargs,
    r#"
class A[*Ts]:
    def put(self, *args: *Ts) -> None: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    bug = "A returned callable with unpacked parameters should make a TypeVarTuple contravariant",
    test_infer_contravariant_from_returned_callable,
    r#"
from typing import Callable

class A[*Ts]:
    def get(self) -> Callable[[*Ts], None]: ...

def f(obj: A[object], int_: A[int]):
    x: A[int] = obj  # E: `A[object]` is not assignable to `A[int]`
    y: A[object] = int_  # E: `A[int]` is not assignable to `A[object]`
"#,
);

testcase!(
    bug = "Declared TypeVarTuple variance should be checked against unpacked `*args`",
    test_covariant_type_var_tuple_used_in_unpacked_varargs,
    r#"
from typing import Generic, TypeVarTuple

Ts_co = TypeVarTuple("Ts_co", covariant=True)  # E: Unexpected keyword argument `covariant` to TypeVarTuple

class A(Generic[*Ts_co]):
    def put(self, *args: *Ts_co) -> None: ...
"#,
);

testcase!(
    bug = "Declared TypeVarTuple variance should be checked against a returned tuple",
    test_contravariant_type_var_tuple_used_in_returned_tuple,
    r#"
from typing import Generic, TypeVarTuple

Ts_contra = TypeVarTuple("Ts_contra", contravariant=True)  # E: Unexpected keyword argument `contravariant` to TypeVarTuple

class A(Generic[*Ts_contra]):
    def get(self) -> tuple[*Ts_contra]: ...
"#,
);
