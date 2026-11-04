/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::util::TestEnv;
use crate::testcase;

testcase!(
    test_sphinx_param_type,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

def f(x, y):
    """
    :param int x: the x
    :param str y: the y
    """
    assert_type(x, int)
    assert_type(y, str)

f(1, "a")
f("a", 1)  # E: `Literal['a']` is not assignable to parameter `x` with type `int` # E: `Literal[1]` is not assignable to parameter `y` with type `str`
"#,
);

testcase!(
    test_sphinx_type_field,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

def f(x, y):
    """
    :param x: the x
    :type x: int
    :type y: list[str]
    """
    assert_type(x, int)
    assert_type(y, list[str])
"#,
);

testcase!(
    test_sphinx_type_field_wins_over_param_field,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

def f(x, y):
    """
    :type x: int
    :param str x: the x
    :param str y: the y
    :type y: bytes
    """
    assert_type(x, int)
    assert_type(y, bytes)
"#,
);

testcase!(
    test_sphinx_param_type_none_default,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

def f(x=None, y="a"):
    """
    :param int x: the x
    :param str y: the y
    """
    assert_type(x, int | None)
    assert_type(y, str)

f()
f(None, "b")
f(1)
f(y=None)  # E: `None` is not assignable to parameter `y` with type `str`
"#,
);

testcase!(
    test_sphinx_param_type_does_not_check_default,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

def f(x=1):
    """
    :param str x: the x
    """
    assert_type(x, str)
"#,
);

testcase!(
    test_sphinx_param_type_union_and_attribute,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
import os
from typing import assert_type

def f(x, y, z):
    """
    :param os.PathLike[str]|str x: the x
    :param dict[str, int] y: the y
    :param tuple[int, ...] z: the z
    """
    assert_type(x, os.PathLike[str] | str)
    assert_type(y, dict[str, int])
    assert_type(z, tuple[int, ...])
"#,
);

testcase!(
    test_sphinx_param_type_bare_generic,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import Any, assert_type

def f(x):
    """
    :param list x: the x
    """
    assert_type(x, list[Any])
"#,
);

testcase!(
    test_sphinx_param_type_local_class,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

class A: ...

def f(x):
    """
    :param A x: the x
    """
    assert_type(x, A)

class B:
    class Inner: ...

    def m(self, x):
        """
        :param Inner x: the x
        """
        assert_type(x, B.Inner)
"#,
);

testcase!(
    test_sphinx_param_type_init_uses_class_docstring,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

class C:
    """
    :param int x: the x
    :param str y: the y
    """

    def __init__(self, x, y=None):
        assert_type(x, int)
        assert_type(y, str | None)

    def other(self, x):
        assert_type(x, int)  # E: assert_type(Unknown, int) failed

C(1)
C("a")  # E: `Literal['a']` is not assignable to parameter `x` with type `int`
"#,
);

testcase!(
    test_sphinx_param_type_init_docstring_wins_over_class_docstring,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

class C:
    """
    :param int x: the x
    :param int y: the y
    """

    def __init__(self, x, y):
        """
        :param str x: the x
        """
        assert_type(x, str)
        assert_type(y, int)
"#,
);

testcase!(
    test_sphinx_param_type_annotation_wins,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import assert_type

def f(x: bytes, y):
    """
    :param int x: the x
    :param int y: the y
    """
    assert_type(x, bytes)
    assert_type(y, int)
"#,
);

testcase!(
    test_sphinx_param_type_not_a_declared_type,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
def f(x):
    """
    :param int x: the x
    """
    x = "a"
"#,
);

testcase!(
    test_sphinx_param_type_ignores_what_it_cannot_understand,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import Any, assert_type

def f(a, b, c, d, e, f, g):
    """
    :param NoSuchName a: unknown name
    :param str or None b: prose
    :param int, optional c: not a type
    :param callable d: not a type form
    :param no.such.module.Thing e: unknown module
    :param int[str] f: invalid subscript
    :param "int" g: string
    """
    assert_type(a, Any)
    assert_type(b, Any)
    assert_type(c, Any)
    assert_type(d, Any)
    assert_type(e, Any)
    assert_type(f, Any)
    assert_type(g, Any)
"#,
);

testcase!(
    test_sphinx_param_type_ignores_untyped_and_variadic_fields,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import Any, assert_type

def f(x, *args, **kwargs):
    """
    :param x: untyped
    :param int *args: not supported
    :param str **kwargs: not supported
    """
    assert_type(x, Any)
    assert_type(args, tuple[Any, ...])
    assert_type(kwargs, dict[str, Any])
"#,
);

testcase!(
    test_param_types_from_docstring_is_opt_in,
    r#"
from typing import Any, assert_type

def f(x, y=None):
    """
    :param int x: the x
    :type y: str
    """
    assert_type(x, Any)
    assert_type(y, Any | None)

f("a", 1)

class C:
    """
    :param int x: the x
    """

    def __init__(self, x):
        assert_type(x, Any)

C("a")
"#,
);

testcase!(
    test_sphinx_legacy_type_variable,
    TestEnv::new().enable_infer_param_types_from_docstring(),
    r#"
from typing import TypeVar, assert_type

T = TypeVar("T")

def ident(x):
    """
    :param T x: the x
    """
    return x

assert_type(ident(1), int)
assert_type(ident("a"), str)
"#,
);
