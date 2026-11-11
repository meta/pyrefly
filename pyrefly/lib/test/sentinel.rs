/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_python::sys_info::PythonVersion;

use crate::test::util::TestEnv;
use crate::testcase;

testcase!(
    test_sentinel_construction_success,
    r#"
from typing_extensions import Sentinel

A = Sentinel("A")
    "#,
);

testcase!(
    test_sentinel_construction_second_positional_arg,
    r#"
from typing_extensions import Sentinel

A = Sentinel("A", 123)  # E: Sentinel only takes one positional argument
    "#,
);

testcase!(
    test_sentinel_construction_with_repr_success,
    r#"
from typing_extensions import Sentinel

A = Sentinel("A", repr="some text")
    "#,
);

testcase!(
    test_sentinel_construction_with_repr_str_success,
    r#"
from typing_extensions import Sentinel

text: str = "some other text"
A = Sentinel("A", repr=text)
    "#,
);

testcase!(
    test_sentinel_construction_with_repr_int_error,
    r#"
from typing_extensions import Sentinel

text = 5
A = Sentinel("A", repr=text)  # E: Invalid type for sentinel `repr` Literal[5]
    "#,
);

testcase!(
    test_sentinel_construction_name_kwarg_error,
    r#"
from typing_extensions import Sentinel

A = Sentinel(name="A")  # E: Sentinel requires a name as the first argument # E: Unexpected keyword argument `name` to sentinel
    "#,
);

testcase!(
    test_sentinel_construction_different_names_allowed,
    r#"
from typing_extensions import Sentinel

A = Sentinel("<A>")
    "#,
);

testcase!(
    test_sentinel_uses_sentinel_string_literal_name_in_error_messages,
    r#"
from typing_extensions import Sentinel

A = Sentinel("<A>")

def foo(a: A):
    b: int = a  # E: `<A>` is not assignable to `int`
    "#,
);

testcase!(
    test_sentinel_defaults_to_assignment_name_if_not_constructed_with_name,
    r#"
from typing_extensions import Sentinel

A = Sentinel()  # E: Sentinel requires a name as the first argument

def foo(a: A):
    b: int = a  # E: `A` is not assignable to `int`
    "#,
);

testcase!(
    test_sentinel_annotated_with_own_name,
    r#"
from typing_extensions import Sentinel

A: A = Sentinel("A")  # E: Circular reference to `A` in annotation
    "#,
);

testcase!(
    test_sentinel_typehint_different_sentinel,
    r#"
from typing_extensions import Sentinel

A = Sentinel("A")
B: A = Sentinel("B")  # E: `sentinel` is not assignable to `A`
    "#,
);

testcase!(
    test_sentinel_typehint_any,
    r#"
from typing import Any, assert_type
from typing_extensions import Sentinel

A: Any = Sentinel("A")
assert_type(A, Any)
    "#,
);

testcase!(
    test_sentinel_violates_annotation,
    r#"
from typing_extensions import Sentinel

MISSING: int = 0
MISSING = Sentinel('MISSING')  # E: `MISSING` is not assignable to variable `MISSING` with type `int`
    "#,
);

testcase!(
    test_sentinel_no_args_error,
    r#"
from typing_extensions import Sentinel

MISSING = Sentinel()  # E: Sentinel requires a name as the first argument
    "#,
);

testcase!(
    test_sentinel_typing_extensions_3_15,
    TestEnv::new().with_version(PythonVersion::new(3, 15, 0)),
    r#"
from typing import Literal, assert_type
from typing_extensions import Sentinel

A = Sentinel("A")
def foo(a: A | Literal[False]):
    if a:
        assert_type(a, A)
    else:
        assert_type(a, Literal[False])
    "#,
);

testcase!(
    test_sentinel_lowercase_3_15,
    TestEnv::new().with_version(PythonVersion::new(3, 15, 0)),
    r#"
from typing import Literal, assert_type

A = sentinel("A")
def foo(a: A | Literal[False]):
    if a:
        assert_type(a, A)
    else:
        assert_type(a, Literal[False])
    "#,
);

testcase!(
    test_sentinel_lowercase_3_14_error,
    TestEnv::new().with_version(PythonVersion::new(3, 14, 0)),
    r#"
A = sentinel("A")  # E: Could not find name `sentinel`
    "#,
);

testcase!(
    // typing_extensions backports the lowercase `sentinel` for pre-3.15, so
    // importing it must behave exactly like the uppercase `Sentinel` and the
    // builtin lowercase form, not fall back to the function's return type.
    test_sentinel_lowercase_from_typing_extensions,
    r#"
from typing import Literal, assert_type
from typing_extensions import sentinel

A = sentinel("A")
def foo(a: A | Literal[False]):
    if a:
        assert_type(a, A)
    else:
        assert_type(a, Literal[False])
    "#,
);

testcase!(
    test_sentinel_lowercase_from_typing_extensions_reveal,
    r#"
from typing import reveal_type
from typing_extensions import sentinel

Lower = sentinel("Lower")
reveal_type(Lower)  # E: Lower
    "#,
);

testcase!(
    test_sentinel_in_class_body,
    r#"
from typing import assert_type
from typing_extensions import Sentinel

class Cls:
    IN_CLASS = Sentinel("Cls.IN_CLASS")

def func3(x: int | Cls.IN_CLASS = Cls.IN_CLASS) -> None:
    if x is Cls.IN_CLASS:
        assert_type(x, Cls.IN_CLASS)
    else:
        assert_type(x, int)
    "#,
);

testcase!(
    test_builtin_sentinel_3_15_has_dunder_name,
    TestEnv::new().with_version(PythonVersion::new(3, 15, 0)),
    r#"
MISSING = sentinel("MISSING")
MISSING.__name__
    "#,
);

testcase!(
    test_typeshed_sentinel_3_14_has_dunder_name,
    TestEnv::new().with_version(PythonVersion::new(3, 14, 0)),
    r#"
from typing_extensions import Sentinel

MISSING = Sentinel("MISSING")
MISSING.__name__
    "#,
);

testcase!(
    sentinel_is_truthy,
    r#"
from typing_extensions import Sentinel
from typing import Literal, assert_type

MISSING = Sentinel("MISSING")
x: MISSING | None = MISSING
if x:
    assert_type(x, MISSING)
else:
    assert_type(x, None)
    "#,
);

testcase!(
    test_sentinel_in_class_body_reveal_qualified_name,
    r#"
from typing import reveal_type
from typing_extensions import Sentinel

class Cls:
    IN_CLASS = Sentinel("IN_CLASS")

reveal_type(Cls.IN_CLASS)  # E: Cls.IN_CLASS
    "#,
);

testcase!(
    test_nonliteral_sentinel_name_ok,
    r#"
from typing import reveal_type
from typing_extensions import Sentinel
def name() -> str: ...
X = Sentinel(name())
reveal_type(X)  # E: X
    "#,
);

testcase!(
    test_nonstring_sentinel_name_errors,
    r#"
from typing import reveal_type
from typing_extensions import Sentinel
def id() -> int: ...
X = Sentinel(id())  # E: `int` is not assignable to parameter `name` with type `str`
reveal_type(X)  # E: X
    "#,
);

testcase!(
    test_literal_name,
    r#"
from typing import Literal, reveal_type
from typing_extensions import Sentinel
def name() -> Literal["MyAwesomeSentinel"]: ...
X = Sentinel(name())
Y = Sentinel("JustAsAwesomeSentinel")
reveal_type(X)  # E: MyAwesomeSentinel
reveal_type(Y)  # E: JustAsAwesomeSentinel
    "#,
);

testcase!(
    test_sentinel_extra_positional_arg,
    r#"
from typing import Literal, assert_type
from typing_extensions import Sentinel

MISSING = Sentinel("MISSING", z := 1)  # E: Sentinel only takes one positional argument
assert_type(z, Literal[1])
"#,
);

testcase!(
    test_sentinel_with_type_qualifier,
    r#"
from typing import ClassVar, Final
from typing_extensions import Sentinel

class Bar:
    MARKER: ClassVar = Sentinel("MARKER")
    _value: int | MARKER

class Baz:
    MARKER: Final = Sentinel("MARKER")
    _value: int | MARKER
    "#,
);

testcase!(
    test_sentinel_qualifiers_enforced_on_class_fields,
    r#"
from typing import ClassVar, Final
from typing_extensions import Sentinel

class A:
    M: Final = Sentinel("M")

A.M = A.M  # E: Cannot set field `M`

class Sub(A):
    M = A.M  # E: `M` is declared as final in parent class `A`

class B:
    M: ClassVar = Sentinel("M")

B().M = B.M  # E: Cannot set field `M`
    "#,
);

testcase!(
    test_sentinel_with_explicit_any_named_class_var,
    r#"
from typing import Any as ClassVar, assert_type
from typing_extensions import Sentinel

S: ClassVar = Sentinel("S")
assert_type(S, ClassVar)
    "#,
);

testcase!(
    test_sentinel_final_initialized_in_body_is_read_only_in_init,
    r#"
from typing import Final
from typing_extensions import Sentinel

class C:
    S: Final = Sentinel("S")
    def __init__(self) -> None:
        self.S = self.S  # E: Cannot set field `S`
    "#,
);

testcase!(
    test_sentinel_with_qualifier_as_dataclass_field,
    r#"
from dataclasses import dataclass
from typing import ClassVar, Final
from typing_extensions import Sentinel

@dataclass
class A:
    S: Final = Sentinel("S")
    T: ClassVar = Sentinel("T")

A(S=A.S)
A(T=A.T)  # E: Unexpected keyword argument `T`
    "#,
);

testcase!(
    test_sentinel_final_in_named_tuple,
    r#"
from typing import Final, NamedTuple
from typing_extensions import Sentinel

class C(NamedTuple):
    S: Final = Sentinel("S")  # E: `Final` may not be used for NamedTuple members
    "#,
);

testcase!(
    test_sentinel_repr_none,
    r#"
from typing import Final
from typing_extensions import Sentinel

S: Final = Sentinel("S", repr=None)
    "#,
);
