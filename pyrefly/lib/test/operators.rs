/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::util::TestEnv;
use crate::testcase;

testcase!(
    test_binop,
    r#"
from typing import assert_type
def f(a: int, b: int) -> None:
    c = a + b
    assert_type(c, int)
    d = a + 1
    assert_type(d, int)
"#,
);

testcase!(
    test_literal_int_arithmetic,
    r#"
from typing import Literal, assert_type

C = 128

assert_type(1 + 2, Literal[3])
assert_type(10 - 3, Literal[7])
assert_type(6 * 7, Literal[42])
assert_type(7 // 2, Literal[3])
assert_type(-7 // 2, Literal[-4])
assert_type(7 // -2, Literal[-4])
assert_type(-7 // -2, Literal[3])
assert_type(9223372036854775807 + 1, Literal[9223372036854775808])
assert_type(C + 64, Literal[192])

def f(x: int, y: Literal[1, 2]) -> None:
    assert_type(x + 1, int)
    assert_type(y + 1, int)

def loop() -> None:
    total = 0
    for _ in range(3):
        total = total + 1
    assert_type(total, int)
"#,
);

testcase!(
    test_dict_union_literal_keys,
    r#"
from typing import Literal

Allowed = Literal["a", "b", "c"]
d: dict[Allowed, int] = {"a": 0, "b": 0}
e: dict[Allowed, int] = d | {"c": 0}
d |= {"c": 0}
bad_key: dict[Allowed, int] = d | {"not-allowed": 0}  # E: `dict[Literal['a', 'b', 'c'] | str, int]` is not assignable to `dict[Allowed, int]`
bad_value: dict[Allowed, int] = d | {"a": "not-an-int"}  # E: `dict[Literal['a', 'b', 'c'] | str, int | str]` is not assignable to `dict[Allowed, int]`
    "#,
);

testcase!(
    test_bounded_type_var_comparison,
    r#"
def compare[T: int](x: T, y: T) -> bool:
    return x > y
"#,
);

testcase!(
    test_constrained_type_var_comparison,
    r#"
from typing import Generic, TypeVar, assert_type

N = TypeVar("N", int, str)

class A(Generic[N]):
    a: N

    def foo(self, b: N) -> None:
        assert_type(self.a < b, bool)
"#,
);

testcase!(
    test_bound_typevar_comparison,
    r#"
def f1[T: int | float](x: T, y: T) -> bool:
    # Ok because you can compare an int and a float
    return x < y
def f2[T: int | str](x: T, y: T) -> bool:
    return x < y  # E: not supported between `int` and `str`  # E: not supported between `str` and `int`
    "#,
);

testcase!(
    test_cannot_compare_unrestricted_typevar,
    r#"
def f1[T](x: T) -> bool:
    return x < 0  # E: not supported between `T` and `Literal[0]`
def f2[T](x: T) -> bool:
    return 0 < x  # E: not supported between `Literal[0]` and `T`
    "#,
);

testcase!(
    test_preserve_typevar_through_comparison_and_binop,
    r#"
from typing import Any, Protocol, Self, TypeVar

class NativeExpr:
    def __ge__(self, value: Any, /) -> Self: ...
    def __add__(self, value: Any, /) -> Self: ...

NativeExprT = TypeVar("NativeExprT", bound=NativeExpr)

class SQLExpr(Protocol[NativeExprT]):
    def _window(self) -> NativeExprT: ...
    def _when(self, condition: NativeExprT) -> NativeExprT: ...

    def do_stuff(self) -> None:
        x = self._window()
        y_ge = x >= 1
        z_add = x + 1
        self._when(y_ge)
        self._when(z_add)
    "#,
);

testcase!(
    test_compare_typevars,
    r#"
from typing import Generic, Protocol, TypeVar

Weight = TypeVar("Weight", bound="ImplementationWeight")
SelfWeight = TypeVar("SelfWeight", bound="ImplementationWeight")

class ImplementationWeight(Protocol):
    def __lt__(self: SelfWeight, other: SelfWeight) -> bool: ...

class CandidateWeight(Generic[Weight]):
    weight: Weight
    def __lt__(self, other: CandidateWeight[Weight]) -> bool:
        return self.weight < other.weight
    "#,
);

testcase!(
    test_add_typevars,
    r#"
from typing import Generic, Protocol, TypeVar

Weight = TypeVar("Weight", bound="ImplementationWeight")
SelfWeight = TypeVar("SelfWeight", bound="ImplementationWeight")

class ImplementationWeight(Protocol):
    def __add__(self: SelfWeight, other: SelfWeight) -> SelfWeight: ...

class CandidateWeight(Generic[Weight]):
    weight: Weight
    def __add__(self, other: CandidateWeight[Weight]) -> Weight:
        return self.weight + other.weight
    "#,
);

testcase!(
    test_incompatible_equality_comparison,
    TestEnv::new().enable_incompatible_comparison_error(),
    r#"
from collections import UserString
from decimal import Decimal
from fractions import Fraction

def compare(
    x: int,
    y: str,
    z: int | str,
    f: float,
    b: bytes,
    ba: bytearray,
    s: set[int],
    fs: frozenset[int],
    d: Decimal,
    bo: bool,
    c: complex,
    mv: memoryview,
    fr: Fraction,
    us: UserString,
) -> None:
    x == y  # E: Comparison `==` between incompatible types `int` and `str`
    x != y  # E: Comparison `!=` between incompatible types `int` and `str`
    z == y
    x == f
    x == d
    b == ba
    s == fs
    x == bo
    c == f
    b == mv
    bo == y  # E: Comparison `==` between incompatible types `bool` and `str`
    mv == y  # E: Comparison `==` between incompatible types `memoryview` and `str`
    x == fr
    us == y
    fr == y  # E: Comparison `==` between incompatible types `Fraction` and `str`
    us == x  # E: Comparison `==` between incompatible types `UserString` and `int`
"#,
);

testcase!(
    test_incompatible_equality_comparison_default_off,
    r#"
def compare(x: int, y: str) -> None:
    x == y
"#,
);

testcase!(
    test_negative_literals,
    r#"
from typing import Literal
x: Literal[-1] = -1
"#,
);

testcase!(
    test_positive_literals,
    r#"
from typing import Literal
y = -1
x: Literal[-1] = +y
"#,
);
testcase!(
    test_positive_literals_error,
    r#"
from typing import Literal
y = 1
x: Literal[-1] = +y # E: `Literal[1]` is not assignable to `Literal[-1]`
"#,
);

testcase!(
    test_inversion_literals,
    r#"
from typing import Literal
y = -1
x: Literal[0] = ~y
"#,
);

testcase!(
    test_inversion_literals_error,
    r#"
from typing import Literal
y = -2
x: Literal[0] = ~y # E: `Literal[1]` is not assignable to `Literal[0]`
"#,
);

testcase!(
    test_union_unary_op,
    r#"
from typing import Literal, Union, assert_type
x: Literal[-1, 2] = 2
y = -x
assert_type(y, Literal[-2, 1])
"#,
);

testcase!(
    test_boolean_or_simple,
    r#"
from typing import assert_type
def f(x: int, y: str) -> None:
    z = x or y
    assert_type(z, int | str)
    "#,
);

testcase!(
    test_boolean_or_filter,
    r#"
from typing import assert_type, Literal

x = False or True
assert_type(x, Literal[True])

y = False or None
assert_type(y, None)

def f(a: None, b: int, c: str, cond: bool) -> None:
    if cond:
        b2 = a
    else:
        b2 = b
    d = b2 or c
    assert_type(d, int | str)
    "#,
);

testcase!(
    test_boolean_or_shortcircuit,
    r#"
from typing import assert_type, Literal
x = True or False
assert_type(x, Literal[True])
    "#,
);

testcase!(
    test_integer_or_shortcircuit,
    r#"
from typing import assert_type, Literal
x = 1 or 0
assert_type(x, Literal[1])
    "#,
);

testcase!(
    test_string_or_shortcircuit,
    r#"
from typing import assert_type, Literal
x = "a" or ""
assert_type(x, Literal["a"])
    "#,
);

testcase!(
    test_html_escape_or_str_regression,
    r#"
import html
from typing import assert_type

x = html.escape("") or "a"
assert_type(x, str)
    "#,
);

testcase!(
    test_boolean_and_simple,
    r#"
from typing import assert_type, Literal

def f(x: int, y: str) -> None:
    z = x and y
    assert_type(z, Literal[0] | str)
    "#,
);

testcase!(
    test_boolean_and_filter,
    r#"
from typing import assert_type, Literal
x = True and False
assert_type(x, Literal[False])

y = True and True
assert_type(y, Literal[True])
    "#,
);

testcase!(
    test_boolean_and_shortcircuit,
    r#"
from typing import assert_type, Literal
x = False and True
assert_type(x, Literal[False])
    "#,
);

testcase!(
    test_integer_and_shortcircuit,
    r#"
from typing import assert_type, Literal
x = 0 and 1
assert_type(x, Literal[0])
    "#,
);

testcase!(
    test_string_and_shortcircuit,
    r#"
from typing import assert_type, Literal
x = "" and "a"
assert_type(x, Literal[""])
    "#,
);

testcase!(
    test_boolean_operator_narrow,
    r#"
from typing import assert_type, Literal

def f(x: bool, y: int):
    assert_type(x and y, Literal[False] | int)

def g(x: bool, y: Literal['a'], z: Literal['b']):
    assert_type((x and y) or z, Literal['a', 'b'])

def h(x: int, y: str, z: bool, v: float):
    assert_type(x or y or z or v, int | float | str | Literal[True])
    assert_type(x and y and z and v, Literal[0, "", False] | float)
"#,
);

testcase!(
    test_boolean_union,
    r#"
from typing import assert_type, Literal

def f(x: int, y: str | Literal[False]):
    assert_type(x or y, int | str | Literal[False])
"#,
);

// Regression test: generic function calls in `and` expressions should not
// have the narrowed type from earlier branches used as a hint for type variable
// inference. The narrowing of `bool` to `Literal[False]` is for the result of
// the `and` expression, not for contextual typing of later generic calls.
testcase!(
    test_boolop_generic_call_hint,
    r#"
def f[U](u: U) -> U:
    return u

def caller(x: bool, y: bool) -> int:
    if f(x) and f(y):
        return 1
    else:
        return 0
"#,
);

testcase!(
    test_unary_not_unknown,
    r#"
from typing import assert_type
def f(x):
    y = not x
    assert_type(y, bool)
    "#,
);

testcase!(
    test_unary_not_literal,
    r#"
from typing import assert_type, Literal

x1 = True
y1 = not x1
assert_type(y1, Literal[False])

x2 = False
y2 = not x2
assert_type(y2, Literal[True])

x3 = 1
y3 = not x3
assert_type(y3, Literal[False])

x4 = 0
y4 = not x4
assert_type(y4, Literal[True])

x5 = "a"
y5 = not x5
assert_type(y5, Literal[False])

x6 = ""
y6 = not x6
assert_type(y6, Literal[True])
    "#,
);

testcase!(
    test_unary_bool_literals,
    r#"
from typing import Literal, assert_type

def invert_literal_false(x: Literal[False]) -> None:
    assert_type(~x, Literal[-1])

def invert_literal_true(x: Literal[True]) -> None:
    assert_type(~x, Literal[-2])

def negate_literal_false(x: Literal[False]) -> None:
    assert_type(-x, Literal[0])

def negate_literal_true(x: Literal[True]) -> None:
    assert_type(-x, Literal[-1])
    "#,
);

testcase!(
    test_unary_dunders,
    r#"
from typing import Literal, assert_type
class C:
    def __pos__(self) -> Literal[5]:
        return 5
    def __neg__(self) -> Literal[-5]:
        return -5
    def __invert__(self) -> Literal[100]:
        return 100
c = C()
assert_type(+c, Literal[5])
assert_type(-c, Literal[-5])
assert_type(~c, Literal[100])
    "#,
);

testcase!(
    test_unary_dunders_typevar_bound,
    r#"
from typing import Self

class Foo:
    def __neg__(self) -> Self:
        return self
    def __pos__(self) -> Self:
        return self
    def __invert__(self) -> Self:
        return self

def test[F: Foo](foo: F) -> F:
    a: F = -foo
    b: F = +foo
    c: F = ~foo
    return c
    "#,
);

testcase!(
    test_unary_error,
    r#"
+None  # E: Unary `+` is not supported on `None`
+"oh no"  # E: Unary `+` is not supported on `Literal['oh no']`
-"oops"  # E: Unary `-` is not supported on `Literal['oops']`
class A:
    def __invert__(self, extra_arg):
        pass
~A()  # E: Unary `~` is not supported on `A`\n  Missing argument `extra_arg` in function `A.__invert__`
    "#,
);

testcase!(
    test_unary_enum,
    r#"
from enum import Enum
class A(Enum):
    X = 1
class B(Enum):
    X = 1
    def __pos__(self):
        return 0
+A.X  # E: Unary `+` is not supported on `Literal[A.X]`
+B.X  # OK
    "#,
);

testcase!(
    test_operator_error,
    r#"
class C: pass

x = C() + 1  # E:  `+` is not supported between `C` and `Literal[1]`"#,
);

testcase!(
    test_float_int_add,
    r#"
from typing import assert_type
x = 3 + 3.0
assert_type(x, float)
"#,
);

testcase!(
    test_float_int_compare,
    r#"
0 < 1.0
1.0 < 2
1 < 2
1.0 < 3.0
"#,
);

testcase!(
    test_inplace_operator_rhs_union,
    r#"
class A:
    def __radd__(self, other: int) -> int:
        return other
def f(x: int, y: int | A) -> None:
    x += y
    "#,
);

testcase!(
    test_unop_on_any,
    r#"
from typing import Any, assert_type
def f(x: Any):
    assert_type(-x, Any)
    "#,
);

testcase!(
    test_binop_on_any,
    r#"
from typing import Any, Self, assert_type

def f(x: Any):
    assert_type(1 + x, Any)
    assert_type(x < 10, Any)

def f2(x: int | Any):
    assert_type(1 + x, int | Any)
    assert_type(x < 10,  bool | Any )

class C:
    def __add__(self, other: object, /) -> Self: ...
    def __radd__(self, other: object, /) -> Self: ...

def f3(x: C, y: Any) -> None:
    assert_type(x + y, Any)
    assert_type(y + x, Any)
    "#,
);

testcase!(
    test_compare_on_any,
    r#"
from typing import Any, assert_type

def test1(x: Any) -> None:
    assert_type(x == 1, Any)
    assert_type(1 == x, Any)
    assert_type(x != 1, Any)
    assert_type(x is None, bool)
    assert_type(x is not None, bool)
    assert_type(None is x, bool)
    assert_type(x in [1, 2], bool)
    assert_type(x not in [1, 2], bool)
    assert_type(1 in x, bool)
    assert_type(1 not in x, bool)

def test2(x: float, y: Any) -> None:
    any( x == y )
    assert_type(x==y, Any)
    assert_type(y==x, Any)
    assert_type(x!=y, Any)
    assert_type(y!=x, Any)
    "#,
);

testcase!(
    test_binop_type_var,
    r#"
from typing import TypeVar, reveal_type
T_co = TypeVar("T_co", covariant=True)
T_co == int
reveal_type(T_co) # E:  revealed type: TypeVar[T_co]
    "#,
);

testcase!(
    test_comparison_return_type,
    r#"
from typing import Literal, assert_type
class A:
  def __lt__(self, other):
    return 1
assert_type(A() < A(), Literal[1])
    "#,
);

testcase!(
    test_missing_binop_attr,
    r#"
class A:
    pass
class B:
    pass
A() + B()  # E: Cannot find `__add__` or `__radd__`
    "#,
);

// Both __add__ and __radd__ are tried, but it's less confusing to use __add__ when both fail.
testcase!(
    test_binop_error,
    r#"
from typing import Never, assert_type
class A:
    def __add__(self, other: Never) -> "A":
        return self
class B:
    def __radd__(self, other: Never) -> "B":
        return self
a = A() + B()  # E: `B` is not assignable to parameter `other` with type `Never` in function `A.__add__`
assert_type(a, A)
    "#,
);

#[cfg(feature = "third-party-stubs")]
testcase!(
    test_bundled_sympy_operators,
    r#"
from types import NotImplementedType
from typing import TYPE_CHECKING, assert_type

if TYPE_CHECKING:
    from sympy.polys.matrices.domainmatrix import DomainMatrix
    from sympy.polys.rings import PolyElement

def matrices(left: DomainMatrix, right: DomainMatrix) -> None:
    assert_type(left + right, DomainMatrix)
    assert_type(left - right, DomainMatrix)
    assert_type(left / 2, DomainMatrix)
    assert_type(left.__add__(right), DomainMatrix | NotImplementedType)
    assert_type(left.__sub__(right), DomainMatrix | NotImplementedType)
    assert_type(left.__truediv__(2), DomainMatrix | NotImplementedType)

def polynomials(left: PolyElement, right: PolyElement) -> None:
    assert_type(left * right, PolyElement)
    assert_type(left * 2, PolyElement)
    assert_type(2 * left, PolyElement)
    assert_type(2 - left, PolyElement)
    assert_type(divmod(2, left), tuple[PolyElement, PolyElement] | NotImplementedType)
    assert_type(2 % left, PolyElement)
    assert_type(0 / left, PolyElement)
    assert_type(left.__mul__(right), PolyElement | NotImplementedType)
    assert_type(left.__rmul__(2), PolyElement | NotImplementedType)
    assert_type(left.__rsub__(2), PolyElement | NotImplementedType)
    assert_type(left.__rdivmod__(2), tuple[PolyElement, PolyElement] | NotImplementedType)
    assert_type(left.__rmod__(2), PolyElement | NotImplementedType)
    assert_type(left.__rtruediv__(0), PolyElement | NotImplementedType)
"#,
);

testcase!(
    test_not_implemented_same_class_arithmetic,
    r#"
from types import NotImplementedType
from typing import assert_type

class Same:
    def __add__(self, other: object) -> NotImplementedType:
        return NotImplemented

    def __radd__(self, other: object) -> str:
        return "reflected"

Same() + Same()  # E: Cannot find `__add__`

def inplace() -> None:
    value = Same()
    value += Same()  # E: Cannot find `__iadd__` or `__add__`

class SameInplace(Same):
    def __iadd__(self, other: object) -> NotImplementedType:
        return NotImplemented

def declined_inplace() -> None:
    value = SameInplace()
    value += SameInplace()  # E: Cannot find `__iadd__` or `__add__`

class Partial:
    def __add__(self, other: object) -> int | NotImplementedType:
        return NotImplemented

    def __radd__(self, other: object) -> str:
        return "reflected"

assert_type(Partial() + Partial(), int)

def partial_inplace() -> None:
    value = Partial()
    value += Partial()
    assert_type(value, int)

class ReflectedOnly:
    def __radd__(self, other: object) -> str:
        return "reflected"

ReflectedOnly() + ReflectedOnly()  # E: Cannot find `__add__`
"#,
);

testcase!(
    test_inplace_reflected_subclass_priority,
    r#"
from types import NotImplementedType
from typing import Never, assert_type

class Base:
    def __add__(self, other: object) -> int:
        return 1

class Child(Base):
    def __radd__(self, other: object) -> str:
        return "reflected"

def absent_inplace() -> None:
    value = Base()
    value += Child()
    assert_type(value, str)

class BadSignature(Base):
    def __iadd__(self, other: Never) -> bytes:
        return b""

class BadSignatureChild(BadSignature):
    def __radd__(self, other: object) -> str:
        return "reflected"

def signature_recovery() -> None:
    value = BadSignature()
    value += BadSignatureChild()
    assert_type(value, str)

class Declining(Base):
    def __iadd__(self, other: object) -> NotImplementedType:
        return NotImplemented

class DecliningChild(Declining):
    def __radd__(self, other: object) -> str:
        return "reflected"

def declined_inplace() -> None:
    value = Declining()
    value += DecliningChild()
    assert_type(value, str)

class Partial(Base):
    def __iadd__(self, other: object) -> bytes | NotImplementedType:
        return NotImplemented

class PartialChild(Partial):
    def __radd__(self, other: object) -> str:
        return "reflected"

def partial_inplace() -> None:
    value = Partial()
    value += PartialChild()
    assert_type(value, bytes | str)

class Successful(Base):
    def __iadd__(self, other: object) -> float:
        return 1.0

class SuccessfulChild(Successful):
    def __radd__(self, other: object) -> str:
        return "reflected"

def successful_inplace() -> None:
    value = Successful()
    value += SuccessfulChild()
    assert_type(value, float)
"#,
);

testcase!(
    test_reflected_subclass_method_identity,
    r#"
from types import NotImplementedType
from typing import assert_type

class Base:
    def __add__(self, other: object) -> int:
        return 1

    def __radd__(self, other: object) -> str:
        return "reflected"

class Inherited(Base):
    pass

class Aliased(Base):
    __radd__ = Base.__radd__

class Overridden(Base):
    def __radd__(self, other: object) -> str:
        return "overridden"

assert_type(Base() + Inherited(), int)
assert_type(Base() + Aliased(), int)
assert_type(Base() + Overridden(), str)

def inherited_inplace() -> None:
    value = Base()
    value += Inherited()
    assert_type(value, int)

def aliased_inplace() -> None:
    value = Base()
    value += Aliased()
    assert_type(value, int)

def overridden_inplace() -> None:
    value = Base()
    value += Overridden()
    assert_type(value, str)

class Inplace(Base):
    def __iadd__(self, other: object) -> NotImplementedType:
        return NotImplemented

class InplaceChild(Inplace):
    pass

def declined_inplace() -> None:
    value = Inplace()
    value += InplaceChild()
    assert_type(value, int)

class PartialInplace(Base):
    def __iadd__(self, other: object) -> bytes | NotImplementedType:
        return NotImplemented

class PartialInplaceChild(PartialInplace):
    pass

def partial_inplace() -> None:
    value = PartialInplace()
    value += PartialInplaceChild()
    assert_type(value, bytes | int)

class Partial:
    def __add__(self, other: object) -> int | NotImplementedType:
        return NotImplemented

    def __radd__(self, other: object) -> str:
        return "reflected"

class PartialChild(Partial):
    pass

assert_type(Partial() + PartialChild(), int | str)

class Generic[T]:
    def __add__(self, other: object) -> int:
        return 1

    def __radd__(self, other: object) -> str:
        return "reflected"

class GenericChild(Generic[str]):
    pass

assert_type(Generic[int]() + GenericChild(), int)

class Static:
    def __add__(self, other: object) -> int:
        return 1

    @staticmethod
    def __radd__(other: object) -> str:
        return "staticmethod"

class StaticChild(Static):
    pass

class StaticAlias(Static):
    __radd__ = staticmethod(Static.__radd__)

assert_type(Static() + StaticChild(), int)
assert_type(Static() + StaticAlias(), int)
"#,
);

testcase!(
    test_reflected_subclass_dynamic_methods,
    r#"
from typing import assert_type

def reflected():
    def method(self: object, other: object) -> str:
        return "factory"
    return method

class FactoryBase:
    def __add__(self, other: object) -> int:
        return 1

    __radd__ = reflected()

class FactoryChild(FactoryBase):
    __radd__ = reflected()

assert_type(FactoryBase() + FactoryChild(), str)

def factory_inplace() -> None:
    value = FactoryBase()
    value += FactoryChild()
    assert_type(value, str)

class ClassMethodBase:
    def __add__(self, other: object) -> int:
        return 1

    @classmethod
    def __radd__(cls, other: object) -> str:
        return "classmethod"

class ClassMethodChild(ClassMethodBase):
    pass

assert_type(ClassMethodBase() + ClassMethodChild(), str)

def classmethod_inplace() -> None:
    value = ClassMethodBase()
    value += ClassMethodChild()
    assert_type(value, str)
"#,
);

testcase!(
    test_reflected_protocol_operands,
    r#"
from typing import Protocol, assert_type

class Reflected(Protocol):
    def __radd__(self, other: object) -> int: ...

def same_protocol(left: Reflected, right: Reflected) -> None:
    assert_type(left + right, int)
    value = left
    value += right
    assert_type(value, int)

class GenericReflected[T](Protocol):
    def __radd__(self, other: object) -> T: ...

def generic_protocol(left: GenericReflected[int], right: GenericReflected[str]) -> None:
    assert_type(left + right, str)
    value = left
    value += right
    assert_type(value, str)

class ForwardAndReflected(Protocol):
    def __add__(self, other: object) -> bytes: ...
    def __radd__(self, other: object) -> int: ...

def forward_still_first(left: ForwardAndReflected, right: ForwardAndReflected) -> None:
    assert_type(left + right, bytes)
    value = left
    value += right
    assert_type(value, bytes)

class Related(ForwardAndReflected, Protocol):
    pass

def related_protocols(left: ForwardAndReflected, right: Related) -> None:
    assert_type(left + right, int)
    value = left
    value += right
    assert_type(value, int)

class Concrete:
    def __radd__(self, other: object) -> int:
        return 1

Concrete() + Concrete()  # E: Cannot find `__add__`
"#,
);

testcase!(
    test_not_implemented_binary_returns,
    r#"
from types import NotImplementedType
from typing import Never, NoReturn, assert_type

class Declines:
    def __add__(self, other: object) -> NotImplementedType:
        return NotImplemented

class Partial:
    def __add__(self, other: object) -> int | NotImplementedType:
        return NotImplemented

class Right:
    def __radd__(self, other: object) -> str:
        return ""

class Raises:
    def __add__(self, other: object) -> Never:
        raise RuntimeError

class LegacyRaises:
    def __add__(self, other: object) -> NoReturn:
        raise RuntimeError

assert_type(Declines() + Right(), str)
assert_type(Partial() + Right(), int | str)
assert_type(Partial() + object(), int)
Declines() + object()  # E: Cannot find `__add__` or `__radd__`

class BadSignature:
    def __add__(self, other: Never) -> bytes:
        return b""

assert_type(BadSignature() + Right(), str)
BadSignature() + Declines()  # E: `Declines` is not assignable to parameter `other` with type `Never`

def never_with_reflected() -> None:
    assert_type(Raises() + Right(), Never)

def never_without_reflected() -> None:
    assert_type(Raises() + object(), Never)

def no_return() -> None:
    assert_type(LegacyRaises() + Right(), Never)
    "#,
);

testcase!(
    test_not_implemented_operand_unions,
    r#"
from types import NotImplementedType
from typing import assert_type, overload

class Accepted:
    def __radd__(self, other: object) -> bytes:
        return b""

class Rejected:
    def __radd__(self, other: object) -> str:
        return ""

class Left:
    @overload
    def __add__(self, other: Accepted) -> int: ...
    @overload
    def __add__(self, other: Rejected) -> NotImplementedType: ...
    def __add__(self, other: Accepted | Rejected) -> int | NotImplementedType:
        return 1 if isinstance(other, Accepted) else NotImplemented

def right_union(other: Accepted | Rejected) -> None:
    assert_type(Left() + other, int | str)

class DecliningLeft:
    def __add__(self, other: object) -> NotImplementedType:
        return NotImplemented

def left_union(left: Left | DecliningLeft, right: Accepted | Rejected) -> None:
    assert_type(left + right, int | bytes | str)

class SuccessfulLeft:
    def __add__(self, other: object) -> int:
        return 1

class SelectiveRight:
    @overload
    def __radd__(self, other: SuccessfulLeft) -> bytes: ...
    @overload
    def __radd__(self, other: DecliningLeft) -> str: ...
    def __radd__(self, other: SuccessfulLeft | DecliningLeft) -> bytes | str:
        return ""

def only_declining_left(left: SuccessfulLeft | DecliningLeft) -> None:
    assert_type(left + SelectiveRight(), int | str)
    "#,
);

testcase!(
    test_not_implemented_inferred_wrappers,
    r#"
from types import NotImplementedType
from typing import Any, Self, assert_type, overload

class Base:
    def apply(self, other: object):
        return NotImplemented

    def __add__(self, other: object):
        return self.apply(other)

    def add_self(self, other: object) -> None:
        assert_type(self + other, Any)

    def add_explicit_self(self: Self, other: object) -> None:
        assert_type(self + other, Any)

class Day(Base):
    def apply(self, other: object):
        return Day()

assert_type(Day() + Day(), Any)

def bounded[T: Base](value: T) -> None:
    assert_type(value + object(), Any)

class Direct:
    def __add__(self, other: object):
        return NotImplemented

class DirectChild(Direct):
    pass

DirectChild() + object()  # E: Cannot find `__add__` or `__radd__`

class Declared(Base):
    def __add__(self, other: object) -> NotImplementedType:
        return self.apply(other)

Declared() + object()  # E: Cannot find `__add__` or `__radd__`

class Overloaded:
    def apply(self, other: object):
        return NotImplemented

    @overload
    def __add__(self, other: int) -> int: ...
    @overload
    def __add__(self, other: str) -> NotImplementedType: ...
    def __add__(self, other: int | str):
        return self.apply(other)

class OverloadedChild(Overloaded):
    pass

assert_type(OverloadedChild() + 1, int)
OverloadedChild() + "x"  # E: Cannot find `__add__` or `__radd__`

class Value:
    pass

class Guarded:
    def __add__(self, other: object):
        if isinstance(other, Value):
            return Value()
        return NotImplemented

class Reflected:
    def __radd__(self, other: object) -> str:
        return ""

assert_type(Guarded() + Reflected(), Value | str)
    "#,
);

testcase!(
    test_not_implemented_comparison_completion,
    r#"
from types import NotImplementedType
from typing import Literal, Never, assert_type

class Declines:
    def __eq__(self, other: object) -> NotImplementedType:
        return NotImplemented

    def __ne__(self, other: object) -> NotImplementedType:
        return NotImplemented

    def __lt__(self, other: object) -> NotImplementedType:
        return NotImplemented

    def __gt__(self, other: object) -> NotImplementedType:
        return NotImplemented

class Partial:
    def __eq__(self, other: object) -> Literal[True] | NotImplementedType:
        return NotImplemented

    def __ne__(self, other: object) -> Literal[False] | NotImplementedType:
        return NotImplemented

class Accepts:
    def __eq__(self, other: object) -> Literal[True]:
        return True

    def __ne__(self, other: object) -> Literal[False]:
        return False

def compare(left: Declines, right: Declines, partial: Partial, accepts: Accepts) -> None:
    assert_type(left == right, bool)
    assert_type(left != right, bool)
    left < right  # E: Cannot find `__lt__` or `__gt__`
    assert_type(partial == partial, bool)
    assert_type(partial != partial, bool)
    assert_type(left == accepts, Literal[True])
    assert_type(left != accepts, Literal[False])
    assert_type(partial == accepts, Literal[True])
    assert_type(partial != accepts, Literal[False])

class Raises:
    def __eq__(self, other: object) -> Never:
        raise RuntimeError

def no_identity_after_never(left: Raises, right: Raises) -> None:
    assert_type(left == right, Never)
    "#,
);

testcase!(
    test_not_implemented_inplace_stages,
    r#"
from types import NotImplementedType
from typing import Never, assert_type

class Right:
    def __radd__(self, other: object) -> str:
        return ""

class Left:
    def __iadd__(self, other: object) -> NotImplementedType:
        return NotImplemented

    def __add__(self, other: object) -> int:
        return 1

def ordinary_success() -> None:
    left = Left()
    left += Right()
    assert_type(left, int)

class PartialLeft(Left):
    def __iadd__(self, other: object) -> bytes | NotImplementedType:
        return NotImplemented

def partial_inplace() -> None:
    left = PartialLeft()
    left += Right()
    assert_type(left, bytes | int)

class DecliningLeft(Left):
    def __add__(self, other: object) -> NotImplementedType:
        return NotImplemented

def reflected_success() -> None:
    left = DecliningLeft()
    left += Right()
    assert_type(left, str)

class Raises:
    def __iadd__(self, other: object) -> Never:
        raise RuntimeError

def nonreturning() -> None:
    left = Raises()
    left += Right()
    assert_type(left, Never)

class BadSignature(Left):
    def __iadd__(self, other: Never) -> NotImplementedType:  # E: Class member `BadSignature.__iadd__` overrides parent class `Left` in an inconsistent manner
        return NotImplemented

def signature_recovery() -> None:
    left = BadSignature()
    left += Right()
    assert_type(left, int)
    "#,
);

testcase!(
    test_not_implemented_deprecation,
    r#"
from types import NotImplementedType
from typing import assert_type
from warnings import deprecated

class Left:
    @deprecated("Deprecated forward method")
    def __add__(self, other: object) -> NotImplementedType:
        return NotImplemented

class Right:
    @deprecated("Deprecated reflected method")
    def __radd__(self, other: object) -> int:
        return 1

assert_type(Left() + Right(), int)  # E: `Left.__add__` is deprecated  # E: `Right.__radd__` is deprecated
    "#,
);

// We try __iadd__ and some fallback dunders. When all fail, the least confusing option is to use __iadd__.
testcase!(
    test_iadd_error,
    r#"
from typing import Never, assert_type
class A:
    def __iadd__(self, other: Never):
        pass
class B:
    def __radd__(self, other: Never) -> "B":
        return self
a = A()
a += B()  # E: `B` is not assignable to parameter `other` with type `Never` in function `A.__iadd__`
    "#,
);

testcase!(
    test_iadd_ignores_getattr,
    r#"
from typing import assert_type
class A:
    def __add__(self, other: str) -> "A":
        return self
    def __getattr__(self, name: str) -> str:
        return "x"
a = A()
a += "hi"
assert_type(a, A)
    "#,
);

testcase!(
    test_custom_eq,
    r#"
from typing import assert_type
class A:
    def __eq__(self, other) -> bool:
        return True
assert_type(A() == 42, bool)
    "#,
);

testcase!(
    test_in_generator,
    r#"
'x' in (x for x in ['y'])
42 in (x for x in ['y'])  # E: `in` is not supported between `Literal[42]` and `Generator[str]`
    "#,
);

testcase!(
    test_unop_on_self,
    r#"
from typing import Self
class C:
    def __neg__(self) -> Self:
        return self
    def f(self) -> Self:
        return -self
    "#,
);

testcase!(
    test_dunder_bool_short_circuit_and_discard,
    r#"
from typing import assert_type, Literal
class Falsey:
    def __bool__(self) -> Literal[False]:
        return False
class Truthy:
    def __bool__(self) -> Literal[True]:
        return True
class NotBoolable:
    __bool__: int = 0

assert_type(Falsey() or int(), int)
# Note that although we evaluate `__bool__` and use the result for the boolean
# operation control flow, the resulting value is the actual `Truthy` instance
# and not a bool. This matches the runtime.
assert_type(Truthy() or int(), Truthy)
assert_type(int() if Truthy() else str(), int)
assert_type(int() if Falsey() else str(), str)

# Test the use of a non-boolean-convertible type in boolean operators.
#
# The runtime only uses truthiness in short-circuiting here, so it is actually
# legal to use a non-boolable value as the rightmost entry of a bool op.
assert_type(NotBoolable() or int(), int | NotBoolable)  # E: Expected `__bool__` to be a callable, got `int`
assert_type(int() or NotBoolable(), int | NotBoolable)
"#,
);

testcase!(
    test_generic_class_bool_context,
    r#"
from typing import Generic, TypeVar, assert_type

_T = TypeVar("_T")

class Foo(Generic[_T]):
    def __bool__(self) -> bool:
        return False

def f(cls: type[Foo] | None = None):
    x = cls or Foo
    assert_type(x, type[Foo])
    return x
"#,
);

testcase!(
    test_tensor_type_lambda,
    r#"
from typing import Callable, cast, assert_type

class Tensor:
    __pow__ = cast("Callable[[Tensor, int], Tensor]", lambda x, y: x)  # No redundant cast warning - types are not exactly equal

def f(x: Tensor, i: int):
    assert_type(x ** i, Tensor)

    "#,
);

testcase!(
    test_tensor_type_method,
    r#"
from typing import assert_type

def power(x: "Tensor", i: int) -> "Tensor":
    return x

class Tensor:
    __pow__ = power

def f(x: Tensor, i: int):
    assert_type(x ** i, Tensor)

    "#,
);

testcase!(
    test_magic_dunder_call_with_metaclass,
    r#"
from typing import assert_type

class Meta(type):
    def __add__(cls, other) -> int:
        return 0

class A(metaclass=Meta):
    @classmethod
    def __add__(cls, other) -> str:
        return ""

class B(A):
    pass

# The `+` operation uses `Meta.__add__`, unlike a direct `__add__` call, which uses `A.__add__`.
assert_type(B + B, int)
assert_type(B.__add__(B), str)
    "#,
);

testcase!(
    test_iter_var_annotation_with_getitem,
    r#"
class A:
    def __getitem__(self, i: int):
        return 0

x: int
for x in A():
    pass

y: str
for y in A():  # E: Cannot use variable `y` with type `str` to iterate over elements of type `Literal[0]`
    pass
    "#,
);

testcase!(
    test_contains_bad_getitem,
    r#"
class A:
    def __getitem__(self):
        return 0
0 in A()  # E: Expected 0 positional arguments, got 1 in function `A.__getitem__`
    "#,
);

testcase!(
    test_contains_getitem_wrong_type,
    r#"
class A:
    def __getitem__(self, i) -> int:
        return 0
"" in A()  # E: `Literal['']` is not assignable to contained type `int`
    "#,
);

testcase!(
    test_deprecated,
    r#"
from typing import assert_type, Self
from warnings import deprecated
class A:
    @deprecated("Super deprecated")
    def __add__(self, other) -> Self:
        return self
class B:
    def __radd__(self, other) -> Self:
        return self
assert_type(A() + B(), A)  # E: `A.__add__` is deprecated
    "#,
);

// https://github.com/facebook/pyrefly/issues/396
testcase!(
    test_bind_dunder_callable_simple,
    r#"
from typing import Callable

def foo_pow(base: "Foo", arg: "int | Foo") -> "Foo": return Foo()

class Foo:
    __pow__: Callable[["Foo", int], "Foo"] = foo_pow

def test(foo: Foo) -> None:
    foo ** 2
    "#,
);

// https://github.com/facebook/pyrefly/issues/396
testcase!(
    test_bind_dunder_callable_2,
    r#"
from typing import *
class ThisClassWorks:
    def __init__(self, flag: bool) -> None:
        self.flag = flag

    def __and__(self, other: Union["ThisClassWorks", bool]) -> "ThisClassWorks":
        if isinstance(other, bool):
            return ThisClassWorks(self.flag and other)
        return ThisClassWorks(self.flag and other.flag)

    def __rand__(self, other: Union["ThisClassWorks", bool]) -> "ThisClassWorks":
        if isinstance(other, bool):
            return ThisClassWorks(self.flag and other)
        return ThisClassWorks(self.flag and other.flag)

def _produce_and_func() -> Callable[
    ["ThisClassDoesNotWork", Union["ThisClassDoesNotWork", bool]],
    "ThisClassDoesNotWork",
]:
    def and_func(
        self: "ThisClassDoesNotWork", other: Union["ThisClassDoesNotWork", bool]
    ) -> "ThisClassDoesNotWork":
        if isinstance(other, bool):
            return ThisClassDoesNotWork(self.flag and other)
        return ThisClassDoesNotWork(self.flag and other.flag)

    return and_func

class ThisClassDoesNotWork:
    def __init__(self, flag: bool) -> None:
        self.flag = flag

    __and__ = _produce_and_func()
    __rand__ = _produce_and_func()

ThisClassWorks(True) & ThisClassWorks(False)
ThisClassWorks(True) & False
True & ThisClassWorks(False)
ThisClassDoesNotWork(True) & ThisClassDoesNotWork(False)
ThisClassDoesNotWork(True) & False
True & ThisClassDoesNotWork(False)
    "#,
);

// https://github.com/facebook/pyrefly/issues/3876
testcase!(
    test_reflected_dunder_subclass_priority,
    r#"
from enum import IntFlag
from typing import assert_type

class Color(IntFlag):
    RED = 1
    GREEN = 2

def f(x: int, c: Color) -> None:
    # `int & Color` invokes `Color.__rand__` at runtime because `Color` is a
    # proper subclass of `int` that overrides the reflected dunder, so the result
    # keeps the flag type instead of widening to `int`.
    assert_type(x & c, Color)
    assert_type(x | c, Color)
    assert_type(x ^ c, Color)
"#,
);

testcase!(
    test_type_of_typevar_equality,
    r#"
def f[S, T](x: type[S], y: type[T]):
    return x == y
    "#,
);

testcase!(
    test_chained_in,
    r#"
class Foo:
    def __contains__(self, x: int) -> bool:
        ...

class Bar:
    def __contains__(self, x: Foo) -> bool:
        ...

def test(x: int, foo: Foo, bar: Bar) -> None:
    x in foo in bar  # Should be OK
    x in bar # E: `in` is not supported between `int` and `Bar`
    "#,
);

testcase!(
    test_chained_lt,
    r#"
class A:
    def __lt__(self, other: "B") -> bool: ...
class B:
    def __lt__(self, other: "C") -> bool: ...
class C: pass

def test(a: A, b: B, c: C) -> None:
    a < b < c  # Should be OK: (a < b) and (b < c)
    a < c      # E: `<` is not supported between `A` and `C`
    "#,
);

// https://github.com/facebook/pyrefly/issues/4342
testcase!(
    test_date_datetime_comparison,
    r#"
from datetime import date, datetime

d = date.today()
dt = datetime.now()

dt < d  # E: `<` is not supported between `datetime` and `date`
dt <= d  # E: `<=` is not supported between `datetime` and `date`
dt > d  # E: `>` is not supported between `datetime` and `date`
dt >= d  # E: `>=` is not supported between `datetime` and `date`
d < dt  # E: `<` is not supported between `date` and `datetime`
d <= dt  # E: `<=` is not supported between `date` and `datetime`
d > dt  # E: `>` is not supported between `date` and `datetime`
d >= dt  # E: `>=` is not supported between `date` and `datetime`

dt == d
dt != d
d < date.today()
dt < datetime.now()
"#,
);

testcase!(
    test_bitor_unknown_operands,
    r#"
from typing import assert_type, Any

def f(x: int): ...

def test(x, y):
    z = x | y
    # When operands are unknown, the result should be Any, not type[Any]
    assert_type(z, Any)
    # This should not produce an error since z is Any
    f(z)
"#,
);

// https://github.com/facebook/pyrefly/issues/972
testcase!(
    test_int_pow_inference,
    r#"
from typing import assert_type, Any, Literal

# We special-case int ** int:
# exponent is 0 -> Literal[1]
# exponent is positive -> int
# exponent is negative -> float
assert_type(2 ** 0, Literal[1])
assert_type((-2) ** 0, Literal[1])
assert_type(2 ** 25, int)
assert_type(2 ** 26, int)
assert_type(2 ** 100, int)
assert_type((-2) ** 26, int)
assert_type(2 ** -25, float)
assert_type(2 ** -26, float)
assert_type(2 ** -100, float)

# When the exponent sign is unknown, fall back to __pow__ (-> Any).
def f(x: int, y: int) -> None:
    assert_type(x ** y, Any)
"#,
);

testcase!(
    test_augassign,
    r#"
from typing import Any, assert_type
def f1(x):
    y = 1
    y = y + x
    # `x` may be a non-`int` with an `__radd__` method with an unknown return type
    assert_type(y, Any)
def f2(x):
    y = 1
    y += x
    assert_type(y, Any)
    "#,
);

testcase!(
    test_division_by_zero_literal,
    r#"
from typing import Literal

x = 10 / 0  # E: Cannot divide by zero: `/` with a literal zero divisor
y = 10 // 0  # E: Cannot divide by zero: `//` with a literal zero divisor
z = 10 % 0  # E: Cannot divide by zero: `%` with a literal zero divisor

# Variable with Literal[0] type
zero: Literal[0] = 0
a = 10 / zero  # E: Cannot divide by zero: `/` with a literal zero divisor
b = 10 // zero  # E: Cannot divide by zero: `//` with a literal zero divisor
c = 10 % zero  # E: Cannot divide by zero: `%` with a literal zero divisor

# Union containing Literal[0] should not error (int includes non-zero values)
mixed: int | Literal[0] = 0
d = 10 / mixed
e = 10 // mixed
f = 10 % mixed

# Literal union containing zero should not error (could be non-zero at runtime)
maybe_zero: Literal[0, 1] = 0
g = 10 / maybe_zero
h = 10 // maybe_zero
i = 10 % maybe_zero
"#,
);

testcase!(
    test_unary_op_on_class_object,
    r#"
class Widget:
    def __pos__(self) -> int:
        return 1
    def __neg__(self) -> int:
        return -1
    def __invert__(self) -> int:
        return 0

# These are fine on instances:
w = Widget()
+w
-w
~w

# Instance operators don't apply to class objects:
+Widget  # E: Unary `+` is not supported on `type[Widget]`
-Widget  # E: Unary `-` is not supported on `type[Widget]`
~Widget  # E: Unary `~` is not supported on `type[Widget]`
"#,
);

testcase!(
    test_binop_on_class_object,
    r#"
class Vector:
    def __add__(self, other: "Vector") -> "Vector":
        return Vector()

# Fine on instances:
Vector() + Vector()

# Instance operators don't apply to class objects:
Vector + Vector  # E: `+` is not supported
"#,
);

testcase!(
    test_augmented_assign_unsupported,
    r#"
class Point:
    pass

p = Point()
p += 1  # E: `+=` is not supported
"#,
);

// https://github.com/facebook/pyrefly/issues/2914
testcase!(
    test_bool_conversion_non_callable,
    r#"
class BadBool:
    __bool__: int = 3

assert BadBool()  # E: The `__bool__` attribute of `BadBool` has type `int`, which is not callable
"#,
);

// https://github.com/facebook/pyrefly/issues/2913
testcase!(
    test_bool_conversion_in_contains,
    r#"
class BadBool:
    __bool__: int = 3

class Container:
    def __contains__(self, item: object) -> BadBool:
        return BadBool()

10 in Container()  # E: The `__bool__` attribute of `BadBool` has type `int`, which is not callable
"#,
);

testcase!(
    test_in_operator_returns_bool,
    r#"
from typing import assert_type

class Truthy:
    def __contains__(self, item: object) -> int:
        return 1

assert_type(10 in Truthy(), bool)
assert_type(10 not in Truthy(), bool)

x: bool = 10 in Truthy()
y: int = 10 in Truthy()
"#,
);

testcase!(
    test_list_mul_literal_int,
    r#"
from typing import assert_type
_ = [{"col": None}] * 1000
assert_type([1, 2, 3] * 5, list[int])
"#,
);

testcase!(
    test_add_after_narrow,
    r#"
def f[T: (bytes, str)](x: T) -> T:
    if isinstance(x, bytes):
        return x + b""
    else:
        return x + ""
    "#,
);

testcase!(
    test_containment_with_typevars,
    r#"
from typing import Iterable
def f1[T: (str, bytes)](x: T, y: Iterable[T]):
    return x in y
def f2[T: (str, bytes)](x: T, y: Iterable[T]):
    if isinstance(x, str):
        return x in y
def f3[T: (str, bytes)](x: T, y: T):
    return x in y
    "#,
);

testcase!(
    test_binop_on_unknown_types,
    r#"
from typing import Any, assert_type

def f(x: Oops | None, y: None | Oops):  # E:  # E:
    assert_type(x, Any | None)
    assert_type(y, Any | None)

UnknownType: Any
def g(x: UnknownType | None, y: None | UnknownType):
    assert_type(x, Any | None)
    assert_type(y, Any | None)

def h(x: Oops | UnknownType, y: UnknownType | Oops):  # E:  # E:
    assert_type(x, Any)
    assert_type(y, Any)
    "#,
);

// The operands share a constraint choice, not a value. Each of these calls can
// produce a nonzero result when the other operand is a nonzero int.
testcase!(
    test_constrained_typevar_independent_truthiness_narrow,
    r#"
from typing import Literal, reveal_type

def left[T: (int, str)](x: T, y: T) -> Literal[0]:
    if isinstance(x, int) and not x:
        reveal_type(x + y)  # E: revealed type: int
        return x + y  # E: Returned type `int` is not assignable to declared return type `Literal[0]`
    return 0

def right[T: (int, str)](x: T, y: T) -> Literal[0]:
    if isinstance(y, int) and not y:
        reveal_type(x + y)  # E: revealed type: int
        return x + y  # E: Returned type `int` is not assignable to declared return type `Literal[0]`
    return 0

def both[T: (int, str)](x: T, y: T) -> Literal[0]:
    if isinstance(x, int) and isinstance(y, int) and not x:
        reveal_type(x + y)  # E: revealed type: int
        return x + y  # E: Returned type `int` is not assignable to declared return type `Literal[0]`
    return 0

left(0, 1)
right(1, 0)
both(0, 1)
"#,
);

// ty's generics/legacy/functions.md preserves bare constrained TypeVars through
// arithmetic and the identity of a narrowed value returned unchanged.
testcase!(
    test_constrained_typevar_operator_constraint_correlation,
    r#"
from typing import Literal, reveal_type

def bare[T: (int, str)](x: T, y: T) -> T:
    return x + y

def literals[T: (Literal[0], Literal[1])](x: T, y: T) -> Literal[0]:
    if not x:
        reveal_type(x + y)  # E: revealed type: Literal[0]
        return x + y
    return 0

def unchanged[T: (int, str)](x: T) -> T:
    if isinstance(x, int) and not x:
        return x
    return x

bare(1, 2)
bare("a", "b")
literals(0, 0)
"#,
);

// ty also rejects returning literal arithmetic as a narrowed constrained T.
// Existing class-level constraint preservation in test_add_after_narrow is a
// Pyrefly compatibility behavior, not a rule to preserve T for every result.
testcase!(
    test_constrained_typevar_literal_arithmetic_result,
    r#"
from typing import reveal_type

def increment[T: (int, str)](x: T) -> T:
    if isinstance(x, int) and not x:
        reveal_type(x + 1)  # E: revealed type: Literal[1]
        return x + 1  # E: Returned type `Literal[1]` is not assignable to declared return type `T`
    return x

def bits[T: (bool, str)](x: T, y: T) -> T:
    if isinstance(x, bool) and isinstance(y, bool) and not x:
        return x | y  # E: Returned type `bool` is not assignable to declared return type `T`
    return x
"#,
);

// An otherwise empty nominal class may overlap a runtime-checkable protocol.
testcase!(
    test_constrained_typevar_protocol_overlap,
    r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Addable(Protocol):
    def __add__(self, other: object) -> int: ...

class Base:
    pass

def uncertain[T: (Addable, Base)](x: T, y: T) -> int:
    if isinstance(x, Addable):
        return x + y
    return 0
"#,
);

// One shared union-valued constraint still permits different members per value.
testcase!(
    test_constrained_typevar_shared_union_choice,
    r#"
from typing import Literal

def mixed[T: (int | str, bytes)](x: T, y: T) -> None:
    if isinstance(x, int) and not x:
        x + y  # E: not supported

mixed(0, "not an int")

def literal_union[T: (Literal[0, 1], Literal[2])](x: T, y: T) -> Literal[0]:
    if not x:
        return x + y  # E: is not assignable to declared return type `Literal[0]`
    return 0
"#,
);
