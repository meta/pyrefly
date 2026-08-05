/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::util::TestEnv;
use crate::testcase;

fn mock_env(source: &str) -> TestEnv {
    TestEnv::one("other", source).enable_invalid_mock_assertion_error()
}

testcase!(
    test_target_string_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.foo"):  # E: No attribute `foo` in module `other`
    pass
"#,
);

testcase!(
    test_current_module_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("main.foo"):  # E: No attribute `foo` in module `main`
    pass
"#,
);

// `mock.patch` adds `create=True` for us when patching a builtin name on a module, so these
// targets are legal despite `other` defining neither name.
testcase!(
    test_builtin_name_on_module_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.open"):
    pass

with patch("other.ValueError"):
    pass
"#,
);

testcase!(
    test_implicit_builtin_reexport_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.sys"):  # E: No attribute `sys` in module `other`
    pass
"#,
);

// Mirrors the shape seen in the wild (mkdocs, flake8): decorator form, with a replacement passed
// positionally.
testcase!(
    test_builtin_name_on_module_decorator_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import mock_open, patch

@patch("other.open", mock_open(read_data="x"))
def test() -> None:
    pass
"#,
);

testcase!(
    test_decorator_target_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

@patch("other.foo")  # E: No attribute `foo` in module `other`
def test() -> None:
    pass
"#,
);

// `create=True` is forced regardless of what the caller passed, so an explicit `create=False`
// does not bring the check back.
testcase!(
    test_builtin_name_with_create_false_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.open", create=False):
    pass
"#,
);

// The rule applies only when the patch target is a module, not a class.
testcase!(
    test_builtin_name_on_class_checked,
    TestEnv::one("other", "class C:\n    existing = 0"),
    r#"
from unittest.mock import patch

with patch("other.C.open"):  # E: Class `C` has no class attribute `open`
    pass
"#,
);

// Only the final component is fetched with `getattr`; intermediate ones are imported, so a
// builtin name in the middle of a target gets no exemption.
testcase!(
    test_builtin_name_as_intermediate_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.open.write"):  # E: No attribute `open` in module `other`
    pass
"#,
);

testcase!(
    test_create_true_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.foo", create=True):
    pass
"#,
);

testcase!(
    test_unknown_create_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

def test(create: bool) -> None:
    with patch("other.foo", create=create):
        pass
"#,
);

testcase!(
    test_positional_create_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import DEFAULT, patch

with patch("other.foo", DEFAULT, None, True):
    pass
"#,
);

testcase!(
    test_kwargs_create_skipped,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.foo", **{"create": True}):
    pass
"#,
);

testcase!(
    test_create_false_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other.foo", create=False):  # E: No attribute `foo` in module `other`
    pass
"#,
);

testcase!(
    test_private_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch("other._MAXLINE"):  # E: No attribute `_MAXLINE` in module `other`
    pass
"#,
);

testcase!(
    test_private_valid,
    TestEnv::one("other", "_private = 0"),
    r#"
from unittest.mock import patch

with patch("other._private"):
    pass
"#,
);

testcase!(
    test_nested_attributes_checked,
    TestEnv::one("other", "class C:\n    existing = 0\ninstance = C()"),
    r#"
from unittest.mock import patch

with patch("other.C.missing"):  # E: Class `C` has no class attribute `missing`
    pass

with patch("other.instance.missing"):  # E: Object of class `C` has no attribute `missing`
    pass
"#,
);

testcase!(
    test_nested_attributes_valid,
    TestEnv::one("other", "class C:\n    existing = 0\ninstance = C()"),
    r#"
from unittest.mock import patch

with patch("other.C.existing"):
    pass

with patch("other.instance.existing"):
    pass
"#,
);

testcase!(
    test_module_getattr,
    TestEnv::one("other", "def __getattr__(name: str) -> int: ..."),
    r#"
from unittest.mock import patch

with patch("other.dynamic"):
    pass
"#,
);

testcase!(
    test_keyword_checked,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch(target="other.foo"):  # E: No attribute `foo` in module `other`
    pass
"#,
);

testcase!(
    test_other_module_valid,
    TestEnv::one("other", "def foo() -> None: ..."),
    r#"
from unittest.mock import patch

with patch("other.foo"):
    pass
"#,
);

testcase!(
    test_patch_object_not_treated_as_patch,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import patch

with patch.object("other.missing", "attribute"):
    pass
"#,
);

testcase!(
    test_patch_multiple_not_treated_as_patch,
    TestEnv::one("other", ""),
    r#"
from unittest.mock import DEFAULT, patch

with patch.multiple("other.missing", attribute=DEFAULT):
    pass
"#,
);

// https://github.com/facebook/pyrefly/issues/3913
testcase!(
    test_assert_called_with_checks_target_signature,
    mock_env("def foo(x: int, *, y: str) -> None: ..."),
    r#"
from unittest.mock import patch

with patch("other.foo") as mock_foo:
    mock_foo.assert_called_with(1, y="ok")
    mock_foo.assert_called_once_with("bad", y="ok")  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int` in function `other.foo`
    mock_foo.assert_any_call(1, y=2)  # E: Argument `Literal[2]` is not assignable to parameter `y` with type `str` in function `other.foo`
"#,
);

// Mock calls and assertions must not apply the patched function's special typing behavior.
testcase!(
    test_patch_special_function_keeps_mock_behavior,
    TestEnv::new().enable_invalid_mock_assertion_error(),
    r#"
from unittest.mock import patch

with patch("typing.assert_type") as mock_assert_type:
    mock_assert_type("not an int", int)
    mock_assert_type.assert_called_once_with("not an int", int)
"#,
);

// https://github.com/facebook/pyrefly/issues/3913
testcase!(
    test_patch_start_checks_target_signature,
    mock_env("def foo(x: int) -> None: ..."),
    r#"
from unittest.mock import patch

patcher = patch("other.foo")
mock_foo = patcher.start()
mock_foo.assert_called_with("bad")  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int` in function `other.foo`
patcher.stop()
"#,
);

// https://github.com/facebook/pyrefly/issues/3913
testcase!(
    test_async_patch_checks_await_assertion,
    mock_env("async def foo(x: int) -> None: ..."),
    r#"
from unittest.mock import patch

with patch("other.foo") as mock_foo:
    mock_foo.assert_awaited_with(1)
    mock_foo.assert_awaited_once_with("bad")  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int` in function `other.foo`
"#,
);

// https://github.com/facebook/pyrefly/issues/3913
testcase!(
    test_patch_bound_method_checks_bound_signature,
    mock_env("class C:\n    def method(self, x: int) -> None: ...\ninstance = C()"),
    r#"
from unittest.mock import patch

with patch("other.instance.method") as mock_method:
    mock_method.assert_called_once_with("bad")  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int` in function `other.C.method`
"#,
);

testcase!(
    test_patch_new_callable_keeps_declared_result,
    TestEnv::one("other", "def foo(x: int) -> None: ..."),
    r#"
from unittest.mock import patch

class Replacement:
    marker: str

with patch("other.foo", new_callable=Replacement) as replacement:
    replacement.marker = "ok"
"#,
);

testcase!(
    test_patch_class_method_receiver,
    mock_env(
        "class C:\n    def method(self, x: int) -> None: ...\n    @staticmethod\n    def static(x: int) -> None: ...\n    @classmethod\n    def class_method(cls, x: int) -> None: ..."
    ),
    r#"
from other import C
from unittest.mock import patch

with patch("other.C.method") as mock_method:
    C().method(1)
    mock_method.assert_called_once_with(1)
    mock_method.assert_called_with("bad")  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int`

with patch("other.C.method", autospec=True) as mock_method:
    instance = C()
    instance.method(1)
    mock_method.assert_called_once_with(instance, 1)
    mock_method.assert_called_once_with(instance, "bad")  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int`

with patch("other.C.static") as mock_static:
    mock_static.assert_called_once_with(1)

with patch("other.C.class_method") as mock_class_method:
    mock_class_method.assert_called_once_with(1)
"#,
);

testcase!(
    test_patch_call_keeps_mock_behavior,
    mock_env("def foo(x: int) -> int: ..."),
    r#"
from typing import Any, assert_type
from unittest.mock import MagicMock, patch

with patch("other.foo", return_value="ok") as mock_foo:
    result: str = mock_foo(1)
    assert_type(mock_foo(1), Any)
    accepts_mock: MagicMock = mock_foo

with patch("other.foo") as mock_foo:
    mock_foo("any argument").child().assert_called_once_with()
    mock_foo.return_value = "configured later"
    result: str = mock_foo(1)
"#,
);

testcase!(
    test_patch_alternate_autospec_keeps_mock_behavior,
    mock_env("def foo(x: int) -> None: ..."),
    r#"
from unittest.mock import patch

def replacement() -> None: ...

with patch("other.foo", autospec=replacement) as mock_foo:
    mock_foo()
    mock_foo.assert_called_once_with()

with patch("other.foo", spec=replacement) as mock_foo:
    mock_foo.assert_called_once_with()

with patch("other.foo", spec_set=replacement) as mock_foo:
    mock_foo.assert_called_once_with()

def test_unknown_autospec(autospec: object) -> None:
    with patch("other.foo", autospec=autospec) as mock_foo:
        mock_foo()
        mock_foo.assert_called_once_with()
"#,
);

testcase!(
    test_patch_assertion_accepts_equality_matcher,
    mock_env("def foo(x: int, *, y: int) -> None: ..."),
    r#"
from unittest.mock import ANY, patch

class IntMatcher:
    def __eq__(self, other: object) -> bool:
        return isinstance(other, int)

with patch("other.foo") as mock_foo:
    mock_foo.assert_called_once_with(IntMatcher(), y=IntMatcher())
    mock_foo.assert_called_with(ANY, y=ANY)
    mock_foo.assert_called_with("bad", y=1)  # E: Argument `Literal['bad']` is not assignable to parameter `x` with type `int`
    mock_foo.assert_called_with(IntMatcher(), y="bad")  # E: Argument `Literal['bad']` is not assignable to parameter `y` with type `int`
"#,
);

testcase!(
    test_patch_async_generator_uses_magic_mock,
    mock_env(
        "from collections.abc import AsyncIterator\nasync def generate(x: int) -> AsyncIterator[int]:\n    yield x"
    ),
    r#"
from typing import assert_type
from unittest.mock import MagicMock, patch

with patch("other.generate") as mock_generate:
    accepts_mock: MagicMock = mock_generate
    assert_type(mock_generate, MagicMock)
    mock_generate.assert_called_once_with(1)
"#,
);

testcase!(
    test_patch_equality_matcher_does_not_create_any_diagnostics,
    mock_env("def foo(x: int) -> None: ...").enable_explicit_any_use_error(),
    r#"
from unittest.mock import patch

class IntMatcher:
    def __eq__(self, other: object) -> bool:
        return isinstance(other, int)

with patch("other.foo") as mock_foo:
    mock_foo.assert_called_once_with(IntMatcher())
"#,
);

testcase!(
    test_patch_overloaded_assertion,
    mock_env(
        "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n@overload\ndef foo(x: str) -> str: ...\ndef foo(x: int | str) -> int | str: ..."
    ),
    r#"
from typing import Any, assert_type
from unittest.mock import patch

with patch("other.foo") as mock_foo:
    assert_type(mock_foo(1), Any)
    mock_foo.assert_called_once_with(1)
    mock_foo.assert_called_once_with("one")
    mock_foo.assert_called_once_with([])  # E: No matching overload
"#,
);

testcase!(
    test_patch_assertion_check_disabled_by_default,
    TestEnv::one("other", "def foo(x: int) -> int: ..."),
    r#"
from unittest.mock import patch

with patch("other.foo") as mock_foo:
    mock_foo.assert_called_with("previously accepted")
    mock_foo.assert_called_once_with()
    result: str = mock_foo(1)
"#,
);

testcase!(
    test_patch_assertion_error_kind,
    mock_env("def foo(x: int) -> None: ..."),
    r#"
from unittest.mock import patch

with patch("other.foo") as mock_foo:
    mock_foo.assert_called_with("bad")  # pyrefly: ignore[invalid-mock-assertion]
    mock_foo.assert_called_once_with()  # pyrefly: ignore[invalid-mock-assertion]
    mock_foo.assert_called_with(unknown)  # E: Could not find name `unknown`
"#,
);
