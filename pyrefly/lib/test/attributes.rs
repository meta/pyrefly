/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// @lint-ignore-every SPELL deliberately testing bad spelling

use crate::state::require::Require;
use crate::test::util::TestEnv;
use crate::testcase;

testcase!(
    test_setattr_inferred_instance_attributes,
    r#"
from typing import Final, assert_type

class Plain:
    def __init__(self) -> None:
        object.__setattr__(self, "x", 1)
        setattr(self, "y", "hello")

assert_type(Plain().x, int)
assert_type(Plain().y, str)

class FinalField:
    x: Final[int]
    def __init__(self) -> None:
        object.__setattr__(self, "x", 1)

assert_type(FinalField().x, int)

class Starred:
    def __init__(self) -> None:
        object.__setattr__(self, "x", *[1])
        setattr(self, "y", *[1])

_ = Starred().x  # E: Object of class `Starred` has no attribute `x`
_ = Starred().y  # E: Object of class `Starred` has no attribute `y`
"#,
);

testcase!(
    test_setattr_classmethod_attribute_parity,
    r#"
from typing import assert_type

class TestDirect:
    @classmethod
    def setUpClass(cls) -> None:
        cls.x = 1

class TestSetattr:
    @classmethod
    def setUpClass(cls) -> None:
        setattr(cls, "x", 1)

assert_type(TestDirect.x, int)
assert_type(TestDirect().x, int)
assert_type(TestSetattr.x, int)
assert_type(TestSetattr().x, int)
"#,
);

testcase!(
    test_setattr_classmethod_attribute_boundaries,
    TestEnv::new().enable_implicitly_defined_attribute_error(),
    r#"
from typing import assert_type

class ClsSetattr:
    @classmethod
    def setup(cls) -> None:
        setattr(cls, "x", 1)  # E: Attribute `x` is implicitly defined by assignment in method `setup`, which is not a constructor
        object.__setattr__(cls, "y", 1)

def check(c: ClsSetattr) -> None:
    assert_type(c.x, int)
    _ = c.y  # E: Object of class `ClsSetattr` has no attribute `y`
"#,
);

testcase!(
    test_class_body_initialization_is_independent_of_methods,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def cond() -> bool: ...

class Constructor:
    if cond():
        x = 1  # E: Attribute `x` may be uninitialized

    def __init__(self) -> None:
        self.x = 2

class Helper:
    if cond():
        x = 1  # E: Attribute `x` may be uninitialized

    def initialize(self) -> None:
        self.x = 2
"#,
);

testcase!(
    test_attribute_initialization_inherited,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
class Base:
    timeout = 10
    def __init__(self) -> None:
        self.value = 0

class Child(Base):
    def __init__(self, t: int | None = None) -> None:
        super().__init__()
        if t is not None:
            self.timeout = t
            self.value = t
            self.own = t  # E: Attribute `own` may be uninitialized

class Grandchild(Child):
    def __init__(self, t: int | None = None) -> None:
        if t is not None:
            self.timeout = t
            self.value = t

class ClassBody(Base):
    if bool():
        timeout = 20
"#,
);

testcase!(
    test_attribute_initialization_trailing_noreturn,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import NoReturn

def stop() -> NoReturn:
    raise ValueError()

def proceed() -> None: ...

class TrailingCall:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        stop()

class TrailingRaise:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        raise ValueError()

class DeferredThenStop:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            proceed()
        stop()

class DeferredThenProceed:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            stop()
        proceed()

class EarlyReturn:
    def __init__(self, flag: bool) -> None:
        if flag:
            return
        self.value = 1  # E: Attribute `value` may be uninitialized
        stop()
"#,
);

// A path on which the attribute is unset reaches the end of the class body only if every branch
// it passes through completes.
testcase!(
    test_class_attribute_unset_path_cut_off_by_later_branch,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import NoReturn

def stop() -> NoReturn: ...
def coin() -> bool: ...

class CutOff:
    if coin():
        value = 1
    else:
        pass
    if coin():
        value = 2
    else:
        stop()

class NotCutOff:
    if coin():
        value = 1  # E: Attribute `value` may be uninitialized
    else:
        pass
    if coin():
        print(0)
    else:
        stop()
"#,
);

testcase!(
    test_attribute_initialization_while_true_return,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def connect() -> int: ...

class Retry:
    def __init__(self) -> None:
        while True:
            try:
                self.conn = connect()
                return
            except OSError:
                continue

class EarlyReturn:
    def __init__(self, flag: bool) -> None:
        while True:
            if flag:
                return
            self.value = 1  # E: Attribute `value` may be uninitialized
            return

class ReturnOrBreak:
    def __init__(self, flag: bool) -> None:
        while True:
            if flag:
                self.value = 1  # E: Attribute `value` may be uninitialized
                return
            break
"#,
);

testcase!(
    test_attribute_initialization_nested_function,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
class NestedOnly:
    def __init__(self) -> None:
        def later() -> None:
            self.value = 1  # E: Attribute `value` may be uninitialized

class ConditionalOuter:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1  # E: Attribute `value` may be uninitialized
        def later() -> None:
            self.value = 2

class InitializedOuter:
    def __init__(self) -> None:
        self.value = 1
        def later() -> None:
            del self.value

class NestedMethod:
    def __init__(self) -> None:
        class Inner:
            def later(inner) -> None:
                self.value = 1  # E: Attribute `value` may be uninitialized

class NestedReturn:
    def __init__(self) -> None:
        def later() -> None:
            return
        self.value = 1
"#,
);

testcase!(
    test_attribute_initialization_control_flow,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from contextlib import nullcontext
from typing import Literal

class TryExcept:
    def __init__(self, raw: str) -> None:
        try:
            self.value = int(raw)
        except ValueError:
            self.value = 0

class TryExceptMissing:
    def __init__(self, raw: str) -> None:
        try:
            self.value = int(raw)  # E: Attribute `value` may be uninitialized
        except ValueError:
            pass

class Finally:
    def __init__(self) -> None:
        try:
            int("1")
        finally:
            self.value = 1

class With:
    def __init__(self) -> None:
        with nullcontext():
            self.value = 1

class WithConditional:
    def __init__(self, flag: bool) -> None:
        with nullcontext():
            if flag:
                self.value = 1  # E: Attribute `value` may be uninitialized

class Match:
    def __init__(self, kind: Literal["a", "b"]) -> None:
        match kind:
            case "a":
                self.value = 1
            case "b":
                self.value = 2

class MatchMissing:
    def __init__(self, kind: str) -> None:
        match kind:
            case "a":
                self.value = 1  # E: Attribute `value` may be uninitialized

class Deleted:
    def __init__(self) -> None:
        self.value = 1  # E: Attribute `value` may be uninitialized
        del self.value

class DeletedConditionally:
    def __init__(self, flag: bool) -> None:
        self.value = 1  # E: Attribute `value` may be uninitialized
        if flag:
            del self.value

class Reassigned:
    def __init__(self) -> None:
        self.value = 1
        del self.value
        self.value = 2
"#,
);

testcase!(
    test_attribute_initialization_class_conditions_and_hooks,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
import sys
from dataclasses import dataclass
from typing import TYPE_CHECKING, assert_type

class Conditional:
    if TYPE_CHECKING:
        type_only = 1
    if sys.version_info >= (3, 10):
        versioned = 1

class VersionedLoop:
    while sys.version_info >= (3, 0):
        pass
    value = 1

@dataclass
class PostInit:
    flag: bool
    def __post_init__(self) -> None:
        self.value = 1

@dataclass
class ConditionalPostInit:
    flag: bool
    def __post_init__(self) -> None:
        if self.flag:
            self.value = 1  # E: Attribute `value` may be uninitialized

class SubclassHook:
    def __init_subclass__(cls) -> None:
        cls.value = 1

class ConditionalSubclassHook:
    def __init_subclass__(cls, flag: bool = False) -> None:
        if flag:
            cls.value = 1  # E: Attribute `value` may be uninitialized

class ClassLoop:
    for value in [1]:
        pass

assert_type(Conditional.type_only, int)
assert_type(Conditional.versioned, int)
assert_type(ClassLoop.value, int)
"#,
);

testcase!(
    bug = "Suppressed exceptions do not merge attribute initialization",
    test_attribute_initialization_suppressed_exception,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from contextlib import suppress

class C:
    def __init__(self, raw: str) -> None:
        with suppress(ValueError):
            self.value = int(raw)
"#,
);

testcase!(
    test_attribute_initialization_finally_after_return,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def enabled() -> bool: ...

class AssignedInFinally:
    def __init__(self) -> None:
        try:
            return
        finally:
            self.value = 1

class UnpackedInFinally:
    def __init__(self) -> None:
        try:
            return
        finally:
            self.first, [self.second, *self.rest] = 1, [2, 3]

class ReturnBeforeTry:
    def __init__(self) -> None:
        if enabled():
            return
        try:
            pass
        finally:
            self.value = 1  # E: Attribute `value` may be uninitialized

class ReturnInFinally:
    def __init__(self) -> None:
        try:
            return
        finally:
            if enabled():
                return
            self.value = 1  # E: Attribute `value` may be uninitialized

class NestedInTry:
    def __init__(self) -> None:
        try:
            try:
                return
            finally:
                self.inner = 1
        finally:
            self.outer = 1

class DeletedLaterInFinally:
    def __init__(self) -> None:
        try:
            return
        finally:
            self.value = 1  # E: Attribute `value` may be uninitialized
            if enabled():
                del self.value
"#,
);

// The outer `finally` may delete the attribute after the inner one assigns it.
testcase!(
    bug = "A `del` in an outer `finally` does not cancel an inner `finally` assignment",
    test_attribute_initialization_finally_deleted_by_outer_finally,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def enabled() -> bool: ...

class C:
    def __init__(self) -> None:
        try:
            try:
                return
            finally:
                self.value = 1  # Should report `value`
        finally:
            if enabled():
                del self.value
"#,
);

// The `finally` body only counts for `return` paths when it assigns the attribute directly.
testcase!(
    bug = "An assignment in a block inside `finally` does not count for `return` paths",
    test_attribute_initialization_finally_nested_assignment,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def enabled() -> bool: ...

class C:
    def __init__(self) -> None:
        try:
            return
        finally:
            if enabled():
                self.value = 1  # E: Attribute `value` may be uninitialized
            else:
                self.value = 2

class NestedTryInFinally:
    def __init__(self) -> None:
        try:
            return
        finally:
            try:
                pass
            finally:
                self.inner = 1  # E: Attribute `inner` may be uninitialized
            self.outer = 1
"#,
);

testcase!(
    bug = "For loop targets are treated as assigned before the first iteration",
    test_attribute_initialization_class_loop_target,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import assert_type

class C:
    for value in list[int]():
        pass

assert_type(C.value, int)
"#,
);

testcase!(
    test_possibly_uninitialized_attribute_default_disabled,
    r#"
def enabled() -> bool: ...

class C:
    if enabled():
        class_value = 1

    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1

C(True).missing  # E: Object of class `C` has no attribute `missing`
"#,
);

testcase!(
    test_possibly_uninitialized_attribute_suppression,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
class C:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.suppressed = 1  # pyrefly: ignore[possibly-uninitialized-attribute]
            self.reported = 1  # pyrefly: ignore[missing-attribute]  # E: Attribute `reported` may be uninitialized

C(True).missing  # pyrefly: ignore[possibly-uninitialized-attribute]  # E: Object of class `C` has no attribute `missing`
"#,
);

fn env_with_conditional_stub_attribute() -> TestEnv {
    TestEnv::one_with_path(
        "foo",
        "foo.pyi",
        r#"
def enabled() -> bool: ...

class Stub:
    if enabled():
        value = 1
        annotated: int = 1

    def __init__(self, flag: bool) -> None:
        if flag:
            self.instance_value = 1
"#,
    )
    .enable_possibly_uninitialized_attribute_error()
}

testcase!(
    test_possibly_uninitialized_attribute_stub,
    env_with_conditional_stub_attribute(),
    r#"
from typing import assert_type
from foo import Stub

assert_type(Stub.value, int)
assert_type(Stub.annotated, int)
assert_type(Stub(True).instance_value, int)

class Child(Stub): ...

assert_type(Child.value, int)

def enabled() -> bool: ...

class Implementation:
    if enabled():
        value = 1  # E: Attribute `value` may be uninitialized

    def __init__(self, flag: bool) -> None:
        if flag:
            self.instance_value = 1  # E: Attribute `instance_value` may be uninitialized
"#,
);

fn env_with_conditional_attribute() -> TestEnv {
    TestEnv::one(
        "foo",
        r#"
def coin() -> bool: ...
class Base:
    if coin():
        value = 1  # E: Attribute `value` may be uninitialized
"#,
    )
    .enable_possibly_uninitialized_attribute_error()
}

testcase!(
    test_conditionally_defined_imported_attribute,
    env_with_conditional_attribute(),
    r#"
from foo import Base
class Child(Base): ...
Base.value
Child.value
"#,
);

testcase!(
    test_conditionally_defined_methods,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import assert_type

def enabled() -> bool: ...
def fallback() -> None: ...

class ConditionalMethods:
    if enabled():
        def method(self) -> int:
            return 1

        async def async_method(self) -> int:
            return 1

        @staticmethod
        def static_method() -> int:
            return 1

        @classmethod
        def class_method(cls) -> int:
            return 1

        @property
        def prop(self) -> int:
            return 1

        if enabled():
            def nested(self) -> int:
                return 1
        else:
            def nested(self) -> int:
                return 2

    while enabled():
        def loop_method(self) -> int:
            return 1
        break

    if enabled():
        def deferred(self) -> int:
            return 1
    else:
        fallback()

    # A later merge must also preserve the method's definition source.
    if enabled():
        data = 1  # E: Attribute `data` may be uninitialized

assert_type(ConditionalMethods().method(), int)
assert_type(ConditionalMethods.static_method(), int)
assert_type(ConditionalMethods.class_method(), int)
assert_type(ConditionalMethods().prop, int)
assert_type(ConditionalMethods().nested(), int)
assert_type(ConditionalMethods().loop_method(), int)
assert_type(ConditionalMethods().deferred(), int)

async def use_async() -> None:
    assert_type(await ConditionalMethods().async_method(), int)
"#,
);

testcase!(
    test_conditionally_defined_nested_classes,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from enum import Enum

def enabled() -> bool: ...
class Other: ...

class ConditionalClass:
    if enabled():
        class Inner: ...

class ConditionalFunctionalClass:
    if enabled():
        Color = Enum("Color", "A B")  # E: Attribute `Color` may be uninitialized

class ReassignedClass:
    if enabled():
        class Inner: ...
        Inner = Other  # E: Attribute `Inner` may be uninitialized

class MixedDefinitions:
    if enabled():
        if enabled():
            class Inner: ...
        else:
            Inner = Other  # E: Attribute `Inner` may be uninitialized
"#,
);

testcase!(
    test_class_annotation_initialized_in_method,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from dataclasses import dataclass
from typing import ClassVar, NoReturn, Protocol, Self

def enabled() -> bool: ...
def stop() -> NoReturn: ...
def proceed() -> None: ...

class Conditional:
    x: int  # E: Attribute `x` may be uninitialized
    def __init__(self) -> None:
        if enabled():
            self.x = 1

class Complete:
    x: int
    def __init__(self) -> None:
        if enabled():
            self.x = 1
        else:
            self.x = 2

class InitializedByHelper:
    x: int
    def __init__(self) -> None:
        if enabled():
            self.x = 1
    def reset(self) -> None:
        self.x = 0

class NeverAssigned:
    x: int

@dataclass
class Data:
    x: int
    def __post_init__(self) -> None:
        if enabled():
            self.x = 1

class Proto(Protocol):
    x: int
    def __init__(self) -> None:
        if enabled():
            self.x = 1

@dataclass(init=False)
class DataWithoutInit:
    x: int  # E: Attribute `x` may be uninitialized
    def __init__(self) -> None:
        if enabled():
            self.x = 1

class Terminates:
    x: int
    def __init__(self) -> None:
        if enabled():
            self.x = 1
        else:
            stop()

class Proceeds:
    x: int  # E: Attribute `x` may be uninitialized
    def __init__(self) -> None:
        if enabled():
            self.x = 1
        else:
            proceed()

class WithGetattr:
    x: int
    def __init__(self) -> None:
        if enabled():
            self.x = 1
    def __getattr__(self, name: str) -> int: ...

class Base:
    x: int
    def __init__(self) -> None:
        self.x = 0

class Derived(Base):
    x: int
    def __init__(self) -> None:
        if enabled():
            self.x = 1

class ClassLevel:
    x: ClassVar[int]
    def __init__(self) -> None:
        if enabled():
            self.x = 1  # E: Cannot set field `x`

class InitializedInNew:
    x: int
    def __new__(cls) -> Self:
        instance = super().__new__(cls)
        instance.x = 0
        return instance
    def __init__(self) -> None:
        if enabled():
            self.x = 1

# Subclass bodies usually fill a metaclass's annotated attributes.
class Meta(type):
    key: str
    def __init__(cls, name: str, bases: tuple[type, ...], ns: dict[str, object]) -> None:
        if "key" not in ns:
            cls.key = name
"#,
);

testcase!(
    test_attribute_initialization_declared_only_in_base,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from dataclasses import dataclass
from typing import Protocol, Self

def enabled() -> bool: ...

class Base:
    x: int

class Derived(Base):
    def __init__(self) -> None:
        if enabled():
            self.x = 1  # E: Attribute `x` may be uninitialized

class DerivedRedeclare(Base):
    x: int  # E: Attribute `x` may be uninitialized
    def __init__(self) -> None:
        if enabled():
            self.x = 1

class P(Protocol):
    x: int

class Impl(P):
    def __init__(self) -> None:
        if enabled():
            self.x = 1  # E: Attribute `x` may be uninitialized

class InitializingBase:
    x: int
    def __init__(self) -> None:
        self.x = 0

class DerivedFromInitializing(InitializingBase):
    def __init__(self) -> None:
        super().__init__()
        if enabled():
            self.x = 1

class HelperBase:
    x: int
    def __init__(self) -> None:
        self.reset()
    def reset(self) -> None:
        self.x = 0

class DerivedFromHelper(HelperBase):
    def __init__(self) -> None:
        super().__init__()
        if enabled():
            self.x = 1

class AnnotatingBase:
    x: int  # E: Attribute `x` may be uninitialized
    def __init__(self) -> None:
        self.x: int

class DerivedFromAnnotating(AnnotatingBase):
    def __init__(self) -> None:
        if enabled():
            self.x = 1  # E: Attribute `x` may be uninitialized

@dataclass
class DataBase:
    x: int

class DerivedFromDataclass(DataBase):
    def __init__(self) -> None:
        if enabled():
            self.x = 1

class MetaBase(type):
    key: str

class MetaDerived(MetaBase):
    def __init__(cls, name: str, bases: tuple[type, ...], ns: dict[str, object]) -> None:
        if "key" not in ns:
            cls.key = name

class NewBase:
    x: int
    def __new__(cls) -> Self:
        instance = super().__new__(cls)
        instance.x = 0
        return instance

class DerivedFromNew(NewBase):
    def __init__(self) -> None:
        if enabled():
            self.x = 1
"#,
);

// A method whose body is skipped is scanned only for the attributes it writes.
testcase!(
    test_attribute_initialization_declared_only_in_base_skipped_body,
    TestEnv::new_skip_check_no_infer()
        .with_run_require(Require::Errors)
        .enable_possibly_uninitialized_attribute_error(),
    r#"
def enabled() -> bool: ...

class AnnotatingBase:
    x: int
    def setup(self):
        self.x: int

class DerivedFromAnnotating(AnnotatingBase):
    def __init__(self) -> None:
        if enabled():
            self.x = 1  # E: Attribute `x` may be uninitialized

class AugmentingBase:
    x: int
    def setup(self):
        self.x += 1

class DerivedFromAugmenting(AugmentingBase):
    def __init__(self) -> None:
        if enabled():
            self.x = 1  # E: Attribute `x` may be uninitialized

class AssigningBase:
    x: int
    def setup(self):
        self.x = 0

class DerivedFromAssigning(AssigningBase):
    def __init__(self) -> None:
        if enabled():
            self.x = 1
"#,
);

testcase!(
    test_class_body_initialization_termination,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def coin() -> bool: ...
def proceed() -> None: ...
def risky() -> int: ...
def mode() -> str: ...

class Proceeds:
    if coin():
        value = 1  # E: Attribute `value` may be uninitialized
    else:
        proceed()

class Raises:
    if coin():
        value = 1
    else:
        raise ValueError()

class TryAssignsOnBothPaths:
    try:
        value = risky()
    except ValueError:
        value = 0

class TryMissingInHandler:
    try:
        value = risky()  # E: Attribute `value` may be uninitialized
    except ValueError:
        pass

class MatchWithoutWildcard:
    match mode():
        case "a":
            value = 1  # E: Attribute `value` may be uninitialized
        case "b":
            value = 2

class MatchWithWildcard:
    match mode():
        case "a":
            value = 1
        case _:
            value = 2
"#,
);

testcase!(
    test_attribute_initialization_any_base,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import Any
from missing_module import Unknown  # E: Cannot find module `missing_module`

def coin() -> bool: ...

AnyBase: Any = object

class FromUnresolved(Unknown):
    if coin():
        value = 1

class FromAny(AnyBase):
    if coin():
        value = 1

    def __init__(self) -> None:
        if coin():
            self.instance_value = 1

class Known:
    if coin():
        value = 1  # E: Attribute `value` may be uninitialized
"#,
);

testcase!(
    test_attribute_initialization_inherited_from_object,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def coin() -> bool: ...
def custom_repr(self: object) -> str: ...

class Base: ...

class GrandBase:
    value = 0

class Middle(GrandBase): ...

class NoBase:
    if coin():
        __repr__ = custom_repr

class WithBase(Base):
    if coin():
        __repr__ = custom_repr

class Module:
    def __init__(self) -> None:
        if coin():
            self.__module__ = "other"

class NotInherited(Base):
    if coin():
        other = 1  # E: Attribute `other` may be uninitialized

class InheritedFromGrandparent(Middle):
    if coin():
        value = 1
"#,
);

testcase!(
    test_conditional_method_assignment_initialization,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def enabled() -> bool: ...
def callback() -> int:
    return 1

class CallableAttribute:
    if enabled():
        value = callback  # E: Attribute `value` may be uninitialized

class ReassignedMethod:
    if enabled():
        @staticmethod
        def value() -> int: ...
        value = callback  # E: Attribute `value` may be uninitialized

class MixedDefinitions:
    if enabled():
        if enabled():
            @staticmethod
            def value() -> int: ...
        else:
            value = callback  # E: Attribute `value` may be uninitialized

# Local function reads still require a definition on every reachable path.
def local() -> None:
    if enabled():
        def value() -> int:
            return 1
    value()  # E: `value` may be uninitialized
"#,
);

// Test case for various edge cases where a name isn't in the flow, and we might
// or might not decide an attribute has been defined.
testcase!(
    test_semantics_for_when_class_body_defines_attributes,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import assert_type, Any
def condition() -> bool: ...
class A:
    # Annotated with no value
    b: int
    # Annotated twice with no value
    c: str
    c: str
    # Defined in conditional control flow, with and without annotation
    if condition():
        d = 42  # E: Attribute `d` may be uninitialized
        e: int = 42  # E: Attribute `e` may be uninitialized
    # Defined (with or without annotation) but only in terminating control flow
    if condition():
        f = 42
        g: int = 42
        exit()
    h = 42
    i: int = 42
    del h
    del i
assert_type(A.b, int)
assert_type(A.c, str)
assert_type(A.d, int)
assert_type(A.e, int)
assert_type(A.f, Any)  # E: Class `A` has no class attribute `f`
assert_type(A.g, Any)  # E: Class `A` has no class attribute `g`
assert_type(A.h, int)
assert_type(A.i, int)
    "#,
);

testcase!(
    test_set_attribute,
    r#"
class A:
    x: int
def f(a: A):
    a.x = 1  # OK
    a.x = "oops"  # E: `Literal['oops']` is not assignable to attribute `x` with type `int`
    "#,
);

testcase!(
    test_set_attribute_in_unpacked_assign,
    r#"
class A:
    x: int
    y: str
def f(a: A):
    a.x, a.y = "x", "y"  # E: `Literal['x']` is not assignable to attribute `x` with type `int`
    "#,
);

testcase!(
    test_self_attribute_unannotated,
    r#"
from typing import assert_type
class A:
    def __init__(self, x: int):
        self.x = x
def f(a: A):
    assert_type(a.x, int)
    "#,
);

testcase!(
    test_unannotated_attribute_bad_assignment,
    r#"
class A:
    def __init__(self):
        self.x = 0
    def f(self):
        self.x = "oops"  # E: `Literal['oops']` is not assignable to attribute `x` with type `int`
    "#,
);

testcase!(
    test_super_object_bad_assignment,
    r#"
class A:
    a: int = 3

class B(A): pass

super(B, B()).a = 3  # E: Cannot set field `a`
    "#,
);

testcase!(
    test_super_object_delete_error,
    r#"
class A:
    a: int = 3

class B(A): pass
del super(B, B()).a # E: Cannot delete field `a`
    "#,
);

testcase!(
    test_super_retain_self,
    r#"
from typing import Self
class A:
    def m(self) -> Self:
        return self

    @classmethod
    def classm(cls) -> type[Self]:
        return cls

class B(A):
    def m(self) -> Self:
        return super().m()

    @classmethod
    def classm(cls) -> type[Self]:
        return super().classm()
    "#,
);

testcase!(
    test_self_attribute_assign_twice,
    r#"
from typing import assert_type
class A:
    def f(self, x: str):
        self.x = x  # E: `str` is not assignable to attribute `x` with type `int`
    def __init__(self, x: int):
        self.x = x
    "#,
);

testcase!(
    test_self_attribute_in_unrecognized_method_enabled,
    TestEnv::new().enable_implicitly_defined_attribute_error(),
    r#"
from typing import assert_type
class A:
    def f(self, x: int):
        self.x = x  # E: Attribute `x` is implicitly defined by assignment in method `f`, which is not a constructor
def f(a: A):
    assert_type(a.x, int)
    "#,
);

testcase!(
    test_conditional_attribute_initialized_in_helper,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import assert_type

class DelegatingInit:
    def __init__(self, cfg: int | None = None) -> None:
        if cfg is None:
            self._load_defaults()
        else:
            self.cfg = cfg

    def _load_defaults(self) -> None:
        self.cfg = 0

# An unconditional helper assignment suffices even without a call from __init__.
class HelperFirst:
    def _load_defaults(self) -> None:
        self.cfg = 0

    def __init__(self, cfg: int | None = None) -> None:
        if cfg is not None:
            self.cfg = cfg

class ConditionalHelper:
    def __init__(self, cfg: int | None = None) -> None:
        if cfg is not None:
            self.cfg = cfg  # E: Attribute `cfg` may be uninitialized

    def _load_defaults(self, enabled: bool) -> None:
        if enabled:
            self.cfg = 0

# The constructor still determines the attribute type.
class IncompatibleHelper:
    def __init__(self, cfg: int | None = None) -> None:
        if cfg is not None:
            self.cfg = cfg

    def _load_defaults(self) -> None:
        self.cfg = "bad"  # E: `Literal['bad']` is not assignable to attribute `cfg` with type `int`

assert_type(DelegatingInit().cfg, int)
assert_type(HelperFirst().cfg, int)
assert_type(ConditionalHelper().cfg, int)
assert_type(IncompatibleHelper().cfg, int)
"#,
);

testcase!(
    test_self_attribute_in_unrecognized_method_default_disabled,
    r#"
from typing import assert_type
class A:
    def f(self, x: int):
        self.x = x
def f(a: A):
    assert_type(a.x, int)
    "#,
);

testcase!(
    test_attribute_initialization_deferred_methods,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import Never

def stop() -> Never:
    raise RuntimeError()

def proceed() -> None: ...

class ConstructorTerminates:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            stop()

    def initialize(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            proceed()

class HelperTerminates:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            proceed()

    def initialize(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            stop()

class NeitherTerminates:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1  # E: Attribute `value` may be uninitialized
        else:
            proceed()

    def initialize(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            proceed()

class ConditionalHelper:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            stop()

    def initialize(self, flag: bool) -> None:
        if flag:
            self.value = 1

class ConditionalConstructor:
    def __init__(self, flag: bool) -> None:
        if flag:
            self.value = 1

    def initialize(self, flag: bool) -> None:
        if flag:
            self.value = 1
        else:
            stop()
"#,
);

testcase!(
    test_attribute_initialization_loops,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import Never, assert_type

def stop() -> Never:
    raise RuntimeError()

class BeforeLoop:
    def __init__(self, values: list[int]) -> None:
        self.value = 0
        for value in values:
            self.value = value

class PossiblyEmpty:
    def __init__(self, values: list[int]) -> None:
        for value in values:
            self.value = value  # E: Attribute `value` may be uninitialized

class NonEmpty:
    def __init__(self) -> None:
        for value in [1]:
            self.value = value

class NoReturningLoop:
    def __init__(self) -> None:
        self.value = 0
        for value in [1]:
            raise RuntimeError()

class ConditionalContinue:
    def __init__(self, flag: bool) -> None:
        for value in [1]:
            if flag:
                continue
            self.value = value  # E: Attribute `value` may be uninitialized

class ConditionalBreak:
    def __init__(self, flag: bool) -> None:
        while True:
            if flag:
                break
            self.value = 1  # E: Attribute `value` may be uninitialized
            break

class LoopElse:
    def __init__(self, values: list[int]) -> None:
        for value in values:
            self.value = value
        else:
            self.value = 0

class DeferredBeforeLoop:
    def __init__(self, flag: bool, values: list[int]) -> None:
        if flag:
            self.value = 1
        else:
            stop()
        for value in values:
            pass

class DeferredInLoop:
    def __init__(self, flag: bool) -> None:
        for value in [1]:
            if flag:
                self.value = value
            else:
                stop()

assert_type(BeforeLoop([]).value, int)
assert_type(NonEmpty().value, int)
assert_type(LoopElse([]).value, int)
"#,
);

testcase!(
    test_attribute_initialization_deferred_return_paths,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import Never

def stop() -> Never:
    raise RuntimeError()

def proceed() -> None: ...

class AllPathsTerminate:
    def __init__(self, early: bool, flag: bool) -> None:
        if early:
            if flag:
                self.value = 1
            else:
                stop()
            return
        if flag:
            self.value = 1
        else:
            stop()

class EarlyPathContinues:
    def __init__(self, early: bool, flag: bool) -> None:
        if early:
            if flag:
                self.value = 1  # E: Attribute `value` may be uninitialized
            else:
                proceed()
            return
        if flag:
            self.value = 1
        else:
            stop()

class FinalPathContinues:
    def __init__(self, early: bool, flag: bool) -> None:
        if early:
            if flag:
                self.value = 1  # E: Attribute `value` may be uninitialized
            else:
                stop()
            return
        if flag:
            self.value = 1
        else:
            proceed()
"#,
);

testcase!(
    test_inherited_attribute_in_unrecognized_method,
    r#"
from typing import assert_type
class A:
    x: int
class B(A):
    def f(self, x: int):
        self.x = x
    "#,
);

// Verify that we correctly pick up parant class type as context when there's a
// qualifier-only annotation.
testcase!(
    test_inherited_attribute_with_qualifier_only_annotation,
    r#"
from typing import ClassVar, assert_type
class A: pass
class B(A): pass
class Foo:
    x: ClassVar[list[A]] = []
    y: ClassVar[list[A]] = []
class Bar(Foo):
    x: ClassVar = [B()]
    y = [B()]
assert_type(Bar.x, list[A])
assert_type(Bar.y, list[A])
    "#,
);

// Ref https://github.com/facebook/pyrefly/issues/370
// Ref https://github.com/facebook/pyrefly/issues/522
testcase!(
    test_cls_attribute_in_constructor,
    r#"
from typing import ClassVar
class A:
    def __new__(cls, x: int):
        cls.x = x
class B:
    def __init_subclass__(cls, x: int):
        cls.x = x
class C:
    x: ClassVar[int]
    def __new__(cls, x: int):
        cls.x = x
class D:
    x: ClassVar[int]
    def __init_subclass__(cls, x: int):
        cls.x = x
    "#,
);

testcase!(
    test_self_attribute_in_test_setup,
    r#"
class MyTestCase:
    def setUp(self):
        self.x = 5
    def run(self):
        assert self.x == 5
    "#,
);

testcase!(
    bug = "Attributes assigned in TestCase.setUpClass should be available on the class",
    test_class_attribute_in_setup_class,
    r#"
from unittest import TestCase

class Base(TestCase):
    shared: int

class Child(Base):
    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls.shared = 1

    def test_shared(self) -> None:
        assert self.shared == 1

Child.shared
    "#,
);

// This demonstrates why allowing method assignment is unsound when method types are covariant.
testcase!(
    test_method_assign_disabled_by_default,
    r#"
from typing import Protocol
class X(Protocol):
    def foo(self) -> object:
        return 1
class Y:
    def foo(self) -> int:
        return 1
def func(x: X):
    x.foo = lambda: "hi"
y: Y = Y()
func(y)
y.foo()  # result is "hi"
    "#,
);

testcase!(
    test_method_assign,
    TestEnv::new().enable_method_assign_error(),
    r#"
from collections.abc import Callable
from typing import ClassVar

class A:
    def method(self) -> None: ...

    @staticmethod
    def static_method() -> None: ...

    @classmethod
    def class_method(cls) -> None: ...

    callback: Callable[[], None] = lambda: None
    class_var_method: ClassVar[Callable[["A"], None]] = lambda self: None

class B(A):
    pass

def replacement(self: A) -> None: ...

a = A()
A.method = replacement  # E: Cannot assign to method `method`
a.method = lambda: None  # E: Cannot assign to method `method`
A.static_method = lambda: None  # E: Cannot assign to method `static_method`
a.static_method = lambda: None  # E: Cannot assign to method `static_method`
A.class_method = lambda: None  # E: Cannot assign to method `class_method`
a.class_method = lambda: None  # E: Cannot assign to method `class_method`
B.method = replacement  # E: Cannot assign to method `method`
B().method = lambda: None  # E: Cannot assign to method `method`
A.callback = lambda: None
a.callback = lambda: None
a.class_var_method = lambda: None  # E: Cannot set field `class_var_method`
    "#,
);

testcase!(
    test_method_assign_lookup_precedence,
    TestEnv::new().enable_method_assign_error(),
    r#"
from collections.abc import Callable
from typing import TypedDict

class Meta(type):
    def f(cls) -> None: ...

class C(metaclass=Meta):
    pass

C.f = C.f  # E: Cannot assign to method `f`

class D(TypedDict):
    items: int

d: D = {"items": 1}
d.items = d.items  # E: Cannot assign to method `items`

class Base:
    def f(self) -> None: ...

class Child(Base):
    f: Callable[[], None] = lambda: None

    def test(self) -> None:
        super().f = lambda: None  # E: Cannot set field `f`

class Parent:
    f: Callable[[], None] = lambda: None

class Derived(Parent):
    def f(self) -> None: ...

    def test(self) -> None:
        super().f = lambda: None  # E: Cannot set field `f`
    "#,
);

testcase!(
    test_attribute_union,
    r#"
class A:
    x: int
class B:
    x: str
def test(x: A | B):
    del x.x
    x.x = 1  # E: `Literal[1]` is not assignable to attribute `x` with type `str`
    "#,
);

testcase!(
    test_callable_boundmethod_subset,
    r#"
from typing import Callable

class C:
    def f(self, x: int, /) -> str:
        return ""
class C2:
    @classmethod
    def f(cls, x: int, /) -> str:
        return ""
class C3:
    @staticmethod
    def f(x: int, /) -> str:
        return ""
def foo(x: Callable[[int], str], c: C, c2: C2, c3: C3):
    C.f = x  # E: `(int) -> str` is not assignable to attribute `f` with type `(self: C, x: int, /) -> str`
    c.f = x
    C2.f = x
    c2.f = x
    C3.f = x
    c3.f = x
    "#,
);

testcase!(
    test_bound_classmethod_explicit_targs,
    r#"
from typing import assert_type
class A[T]:
    x: T
    def __init__(self, x: T):
        self.x = x
    @classmethod
    def m(cls: 'type[A[T]]', x: T) -> 'A[T]':
        return cls(x)

assert_type(A[int].m(0), A[int])
assert_type(A.m(0), A[int])

def test_typevar_bounds[T: A[int]](x: type[T]):
    assert_type(x.m(0), A[int])
    "#,
);

testcase!(
    test_use_of_class_body_scope_in_class_body_statement,
    r#"
class A:
    x: int = 5
    y: int = x
    "#,
);

testcase!(
    test_annotating_non_self_attributes,
    r#"
class A:
    x: int

class B:
    def __init__(self, a: A):
        a.x: int = 1  # E: Cannot annotate non-self attribute `a.x`

a: A = A()
a.x: int = 5  # E: Cannot annotate non-self attribute `a.x`
    "#,
);

testcase!(
    test_self_attribute_annotated_in_class_body,
    r#"
from typing import assert_type
class A:
    x: str
    def __init__(self, x: int):
        self.x = x  # E: `int` is not assignable to attribute `x` with type `str`
    "#,
);

testcase!(
    test_self_attribute_annotated_assignment,
    r#"
from typing import assert_type

class A:
    def __init__(self, x: str):
        self.x: int = x  # E: `str` is not assignable to attribute `x` with type `int`
def f(a: A):
    assert_type(a.x, int)
    "#,
);

testcase!(
    test_self_attribute_redefinition_across_methods,
    r#"
class A:
    def __init__(self) -> None:
        self.foo: int | None = None
    def calc_foo(self) -> None:
        self.foo: int = 1  # E: `foo` cannot be annotated with `int`, it is already defined with type `int | None`
    "#,
);

testcase!(
    test_generic_classvar,
    r#"
from typing import ClassVar
class A[T]:
    x: ClassVar[T]  # E: `ClassVar` arguments may not contain any type variables
    y: ClassVar[list[T]]  # E: `ClassVar` arguments may not contain any type variables
    "#,
);

testcase!(
    test_self_attribute_annotated_twice,
    r#"
from typing import assert_type, Literal, Final
class A:
    x: int
    y: str
    def __init__(self):
        self.x: Literal[1] = 1
        self.y: Final = "y"
def f(a: A):
    assert_type(a.x, int)
    "#,
);

testcase!(
    test_final_attribute_assigned_in_init,
    r#"
from typing import assert_type, Final, Literal
class A:
    def __init__(self):
        self.x: Final = 0
def f(a: A):
    assert_type(a.x, Literal[0])
    "#,
);

testcase!(
    test_literal_attr_with_annotation,
    r#"
from typing import ClassVar, assert_type
class C:
    x0 = 0
    x1: ClassVar = 0
assert_type(C.x0, int)
assert_type(C.x1, int)
"#,
);

testcase!(
    test_final_annotated_override,
    r#"
from typing import Final
def f() -> int: ...
class Base:
    p: Final = f()
class Derived(Base):
    p = f()  # E: `p` is declared as final in parent class `Base`
"#,
);

testcase!(
    test_self_attribute_bare_annotation,
    r#"
from typing import assert_type
class A:
    def __init__(self, x: str):
        self.x: int
        self.x = x  # E: `str` is not assignable to attribute `x` with type `int`
def f(a: A):
    assert_type(a.x, int)
    "#,
);

testcase!(
    test_attribute_inference,
    r#"
class C:
    x: list[int | str]
def f(c: C):
    c.x = [5]
    "#,
);

testcase!(
    test_set_attribute_in_init_nested,
    r#"
from typing import assert_type
class C:
    def __init__(self):
        def f():
            self.x = 0
        f()
def f(c: C):
    assert_type(c.x, int)
    "#,
);

// TODO: Should we implement simple control-flow heuristics so `C.x` is recognized here?
testcase!(
    test_set_attribute_in_init_indirect,
    TestEnv::new().enable_implicitly_defined_attribute_error(),
    r#"
class C:
    def __init__(self):
        self.f()
    def f(self):
        self.x = 0  # E: Attribute `x` is implicitly defined by assignment in method `f`, which is not a constructor
def f(c: C) -> int:
    return c.x
    "#,
);

testcase!(
    test_missing_self_parameter,
    r#"
class C:
    def f():  # E: Instance method `f` must accept the instance as its first positional parameter
        pass
C().f()  # E: Expected 0 positional arguments, got 1 (including implicit `self`)
    "#,
);

// https://github.com/facebook/pyrefly/issues/5295
testcase!(
    test_instance_method_missing_receiver,
    r#"
class Foo:
    def fn() -> None: ...  # E: Instance method `fn` must accept the instance as its first positional parameter

    def kwargs_only(**kwargs: object) -> None: ...  # E: Instance method `kwargs_only` must accept the instance as its first positional parameter

    def keyword_only(*, x: object = None) -> None: ...  # E: Instance method `keyword_only` must accept the instance as its first positional parameter

    def conventional(self) -> None: ...

    def unconventional(this) -> None: ...

    def positional_only(this, /) -> None: ...

    def variadic(*args: object) -> None: ...

    @staticmethod
    def static() -> None: ...

    def outer(self) -> None:
        def inner() -> None: ...
        inner()

Foo().conventional()
Foo().unconventional()
Foo().positional_only()
Foo().variadic(1)
Foo.static()
    "#,
);

testcase!(
    test_new_missing_cls,
    r#"
class A:
    def __new__() -> "A": ...  # E: Class method `__new__` must accept the class as its first positional parameter

class B:
    def __new__(cls) -> "B": ...

class C:
    def __new__(*args: object) -> "C": ...
    "#,
);

testcase!(
    test_instance_method_missing_receiver_decorators,
    r#"
from typing import Any, Callable, final
from abc import ABC, abstractmethod

def add_receiver(f: Callable[[], None]) -> Callable[[object], None]:
    def wrapper(self: object) -> None:
        f()
    return wrapper

unknown: Any = lambda f: f

class C(ABC):
    @final
    def plain() -> None: ...  # E: Instance method `plain` must accept the instance as its first positional parameter

    @abstractmethod
    def abstract() -> None: ...  # E: Instance method `abstract` must accept the instance as its first positional parameter

    @add_receiver
    def wrapped() -> None: ...

    @unknown
    def uncertain() -> None: ...

    @property
    def prop() -> int: ...  # E: Instance method `prop` must accept the instance as its first positional parameter
    "#,
);

testcase!(
    test_generic_instance_method,
    r#"
class C:
    def f[T](self: T, x: T):
        pass
C().f(C())  # OK
C().f(0)    # E: Argument `Literal[0]` is not assignable to parameter `x` with type `C`
    "#,
);

testcase!(
    test_bad_bound_on_self,
    r#"
class C:
    def f[T: int](self: T) -> T:
        return self
C().f()  # E: `C` is not assignable to upper bound `int`
    "#,
);

// Make sure we treat `callable_attr` as a bare instance attribute, not a bound method.
testcase!(
    test_callable_instance_only_attribute,
    r#"
from typing import Callable, assert_type, Literal
class C:
    callable_attr: Callable[[int], int]
    def __init__(self):
       self.callable_attr = lambda x: x
c = C()
x = c.callable_attr(42)
assert_type(x, int)
    "#,
);

// To align with mypy & pyright, ClassVar[Callable] attributes should have method binding behavior
// See https://discuss.python.org/t/when-should-we-assume-callable-types-are-method-descriptors/92938
testcase!(
    test_callable_as_class_var,
    r#"
from typing import assert_type, Callable, ClassVar
def get_callback() -> Callable[[object, int], int]: ...
class C:
    f: ClassVar[Callable[[object, int], int]] = get_callback()
assert_type(C.f(None, 1), int)
assert_type(C().f(1), int)
"#,
);

// Mypy and Pyright treat `f` as not a method here; its actual behavior
// is ambiguous even if we assume the values are always functions or lambdas
// because the default value can be overridden by instance assignment.
//
// Our behavior is compatible, but the underlying implementation is not, we are
// behaving this way based on how we treat the Callable type rather than based
// on the absence of `ClassVar`.
//
// See https://discuss.python.org/t/when-should-we-assume-callable-types-are-method-descriptors/92938
testcase!(
    test_callable_with_ambiguous_binding,
    r#"
from typing import assert_type, Callable
def get_callback() -> Callable[[object, int], int]: ...
class C:
    f = get_callback()
assert_type(C.f(None, 1), int)
assert_type(C().f(None, 1), int)
# This is why the behavior is ambiguous - at runtime, the default `C.f` is a
# method but the instance-level shadow is not.
C().f = lambda _, x: x
"#,
);

// https://github.com/facebook/pyrefly/issues/3465
testcase!(
    test_callable_with_containing_class_first_param_is_method,
    r#"
from typing import assert_type, Callable
def _unary_op(name: str) -> Callable[["Column"], "Column"]: ...
def _bin_op(name: str) -> Callable[["Column", "Column | int"], "Column"]: ...
def _callback(name: str) -> Callable[[int], int]: ...
class Column:
    isNull = _unary_op("isNull")
    eqNullSafe = _bin_op("eqNullSafe")
    callback = _callback("callback")
c = Column()
assert_type(c.isNull(), Column)
assert_type(c.eqNullSafe(c), Column)
assert_type(c.eqNullSafe(1), Column)
assert_type(Column.isNull(c), Column)
assert_type(c.callback(1), int)
c.isNull(c)  # E: Expected 0 positional arguments
# An instance-level shadow is not bound at runtime, so it must accept the bound signature.
c.isNull = lambda: c
c.isNull = _unary_op("isNull")  # E: not assignable to attribute `isNull`
"#,
);

// An explicitly annotated Callable is an instance attribute, so it is not bound even when its
// first parameter is the containing class. This pattern appears in aiohttp's `ClientResponse`.
testcase!(
    test_annotated_callable_with_containing_class_first_param_is_not_method,
    r#"
from typing import assert_type, Callable
class Response:
    resolve: Callable[["Response", bytes], str] = lambda *_: "utf-8"
    def __init__(self, resolve: Callable[["Response", bytes], str]) -> None:
        self.resolve = resolve
    def encoding(self) -> str:
        return self.resolve(self, b"")
r = Response(lambda _, b: "utf-8")
assert_type(r.resolve(r, b""), str)
"#,
);

testcase!(
    test_class_access_of_instance_only_attribute,
    r#"
from typing import assert_type, Any
class C:
    x: int
    def __init__(self, y: str):
        self.x = 0
        self.y = y
assert_type(C.x, int)
assert_type(C.y, Any)  # E: Instance-only attribute `y` of class `C` is not visible on the class
c = C("y")
assert_type(c.x, int)
assert_type(c.y, str)
"#,
);

testcase!(
    test_match_method_against_callable,
    r#"
from typing import Callable
class C:
    def f(self, x: int) -> None:
        pass
def f1(c: Callable[[int], None]):
    pass
def f2(c: Callable[[C, int], None]):
    pass
f1(C.f)  # E: Argument `(self: C, x: int) -> None` is not assignable to parameter `c` with type `(int) -> None`
f1(C().f)
f2(C.f)
f2(C().f)  # E: Argument `(x: int) -> None` is not assignable to parameter `c` with type `(C, int) -> None`
    "#,
);

testcase!(
    test_simple_inheritance,
    r#"
from typing import assert_type
class B:
    x: int

class HasBase(B):
    y: str

assert_type(HasBase().x, int)
"#,
);

testcase!(
    test_generic_multiple_inheritance,
    r#"
from typing import assert_type
class A[T]:
    x: T

class B[T]:
    y: T

class C[T](A[int], B[T]):
    z: bool

c: C[str] = C()
assert_type(c.x, int)
assert_type(c.y, str)
assert_type(c.z, bool)
"#,
);

testcase!(
    test_generic_chained_inheritance,
    r#"
from typing import assert_type
class A[T]:
    x: T

class B[T](A[list[T]]):
    y: T

class C[T](B[T]):
    z: bool

c: C[str] = C()
assert_type(c.x, list[str])
assert_type(c.y, str)
assert_type(c.z, bool)
"#,
);

testcase!(
    test_nested_class_attribute_with_inheritance,
    r#"
from typing import assert_type

class B:
    class Nested:
        x: int

class C(B):
    pass

N0: B.Nested = C.Nested()
N1: C.Nested = B.Nested()
assert_type(N1.x, int)
"#,
);

testcase!(
    test_class_generic_attribute_lookup,
    r#"
class C[T]:
    x: list[T] = []

C.x  # E: Generic attribute `x` of class `C` is not visible on the class
"#,
);

testcase!(
    test_class_generic_attribute_lookup_on_self_type,
    r#"
from typing import assert_type

class C[T]:
    x: type[T]

    @classmethod
    def f(cls) -> type[T]:
        assert_type(cls.x, type[T])
        return cls.x
"#,
);

testcase!(
    test_var_attribute,
    r#"
from typing import assert_type
def f[T](x: T) -> T:
    return x
class C:
    def __init__(self):
        self.x = 42
assert_type(f(C()).x, int)
    "#,
);

testcase!(
    test_never_attr,
    r#"
from typing import Never, NoReturn, assert_type
def f() -> NoReturn: ...
def g():
    x = f().x
    assert_type(x, Never)
    "#,
);

testcase!(
    test_callable_attr,
    r#"
from typing import assert_type
from types import CodeType
def f():
    pass
def g():
    assert_type(f.__code__, CodeType)
    "#,
);

testcase!(
    test_boundmethod_attr,
    r#"
from typing import assert_type
class A:
    def f(self):
        pass
def g(a: A):
    assert_type(a.f.__self__, object)
    "#,
);

testcase!(
    test_ellipsis_attr,
    r#"
x = ...
x.x  # E: Object of class `EllipsisType` has no attribute `x`
    "#,
);

testcase!(
    test_forall_attr,
    r#"
from typing import assert_type
from types import CodeType
def f[T](x: T) -> T:
    return x
assert_type(f.__code__, CodeType)
    "#,
);

testcase!(
    test_metaclass_attr,
    r#"
from typing import assert_type

class A: ...
class B[T]: ...
assert_type(A.mro(), list[type])
assert_type(B[int].mro(), list[type])

class Meta(type):
    x: int
class C(metaclass=Meta):
    pass
assert_type(C.x, int)
    "#,
);

testcase!(
    test_metaclass_property_precedence,
    r#"
from typing import assert_type

class Meta(type):
    @property
    def f(cls) -> str:
        return "1"

class C(metaclass=Meta):
    @property
    def f(self) -> str:
        return type(self).f

# C.f should resolve to the metaclass property, returning str
assert_type(C.f, str)
    "#,
);

testcase!(
    test_metaclass_method_cls_typetype,
    r#"
from typing import assert_type

class Meta(type):
    def m[T](cls: type[T]) -> T: ...

class C(metaclass=Meta):
    pass

assert_type(C.m(), C)
"#,
);

fn env_with_stub() -> TestEnv {
    let mut t = TestEnv::new();
    t.add_with_path(
        "foo",
        "foo.pyi",
        r#"
class A:
    x: int = ...
    y: int
    "#,
    );
    t
}

testcase!(
    test_stub_initializes_attr,
    env_with_stub(),
    r#"
from typing import assert_type
from foo import A

assert_type(A.x, int)
assert_type(A.y, int)
    "#,
);

testcase!(
    test_object_getattribute,
    r#"
from typing import *
class A:
    def __getattribute__(self, name: str, /) -> int: ...
    def __setattr__(self, name: str, value: Any, /) -> None: ...
    def __delattr__(self, name: str, /) -> None: ...
class B:
    def __getattribute__(self, name: str, /) -> str: ...
a = A()
b = B()
assert_type(a.x, int)
assert_type(b.x, str)
a.x = 1
del a.x
b.x = 1  # E: Object of class `B` has no attribute `x`
del b.x  # E: Object of class `B` has no attribute `x`
    "#,
);

testcase!(
    test_object_getattr,
    r#"
from typing import assert_type

class Foo:
    def __getattr__(self, name: str) -> int: ...

def test(foo: Foo) -> None:
    assert_type(foo.x, int)
    assert_type(foo.y, int)
    foo.x = 1  # E: Object of class `Foo` has no attribute `x`
    del foo.y  # E: Object of class `Foo` has no attribute `y`
    "#,
);

testcase!(
    test_object_getattr_wrong_signature,
    r#"
from typing import assert_type

class Foo:
    def __getattr__(self, name: int) -> int: ...

def test(foo: Foo) -> None:
    assert_type(foo.x, int)  # E: Argument `Literal['x']` is not assignable to parameter `name`
    assert_type(foo.y, int)  # E: Argument `Literal['y']` is not assignable to parameter `name`
    foo.x = 1  # E: Object of class `Foo` has no attribute `x`
    del foo.y  # E: Object of class `Foo` has no attribute `y`
    "#,
);

testcase!(
    test_builtin_getattr_literal_attribute_name,
    r#"
from typing import Any, assert_type

class Foo:
    x: int

    def __init__(self) -> None:
        pass

def test(foo: Foo, attr_name: str) -> None:
    assert_type(getattr(foo, "x"), int)
    assert_type(getattr(foo, "x", None), int | None)
    assert_type(getattr(foo, "y"), Any)
    assert_type(getattr(foo, "y", None), Any | None)
    assert_type(getattr(foo, attr_name), Any)
    assert_type(getattr(foo, attr_name, None), Any | None)
    "#,
);

testcase!(
    test_builtin_getattr_mangled_attribute_name,
    r#"
from typing import Any, assert_type

class C:
    __x: int

    def __getattr__(self, name: str) -> str:
        return "fallback"

def test(c: C) -> None:
    assert_type(getattr(c, "__x"), Any)
    assert_type(getattr(c, "__x", None), Any | None)
    getattr(c, "__x").upper()
    assert_type(getattr(c, "__class__"), type[C])
    "#,
);

testcase!(
    test_builtin_getattr_hook_lookup_failure,
    r#"
from typing import Any, Literal, assert_type, overload

class C:
    @overload
    def __getattr__(self, name: Literal["x"]) -> int: ...
    @overload
    def __getattr__(self, name: Literal["y"]) -> str: ...
    def __getattr__(self, name: str) -> int | str:
        if name == "x":
            return 1
        if name == "y":
            return "value"
        raise AttributeError(name)

def test(c: C) -> None:
    assert_type(getattr(c, "x"), int)
    assert_type(getattr(c, "y", None), str | None)
    assert_type(getattr(c, "missing"), Any)
    assert_type(getattr(c, "missing", None), Any | None)
    "#,
);

testcase!(
    test_builtin_getattr_argument_errors,
    r#"
class C:
    x: int

def test(c: C) -> None:
    getattr(undefined_receiver, "x")  # E: Could not find name `undefined_receiver`
    getattr(c, "x", undefined_default)  # E: Could not find name `undefined_default`
    "#,
);

testcase!(
    test_builtin_getattr_container_defaults,
    TestEnv::new().enable_implicit_any_empty_container_error(),
    r#"
from typing import Any, assert_type

class C:
    x: int

def test(c: C) -> None:
    assert_type(getattr(c, "x", []), int | list[Any])
    assert_type(getattr(c, "x", {}), int | dict[Any, Any])
    assert_type(getattr(c, "missing", []), Any | list[Any])
    assert_type(getattr(c, "missing", {}), Any | dict[Any, Any])
    "#,
);

testcase!(
    test_builtin_getattr_contextual_container_defaults,
    r#"
class C:
    x: int

def list_default(c: C) -> int | list[object]:
    return getattr(c, "x", [1])

def dict_default(c: C) -> int | dict[str, object]:
    return getattr(c, "x", {"key": 1})

def missing_default(c: C) -> int | list[object]:
    return getattr(c, "missing", [1])

def nonliteral_default(c: C, name: str) -> int | list[object]:
    return getattr(c, name, [1])

def none_default(c: C) -> int | None:
    return getattr(c, "x", None)

def incompatible_default(c: C) -> int:
    return getattr(c, "x", "wrong")  # E: Returned type `Literal['wrong'] | int` is not assignable
    "#,
);

testcase!(
    test_builtin_getattr_contextual_lambda_defaults,
    TestEnv::new().enable_implicit_any_lambda_error(),
    r#"
from typing import Callable

class C:
    callback: Callable[[str], str]

def known_attribute(c: C) -> Callable[[str], str]:
    return getattr(c, "callback", lambda x: x.upper())

def missing_attribute(c: C) -> Callable[[str], str]:
    return getattr(c, "missing", lambda x: x.upper())

def nonclass_receiver(c: type[C]) -> Callable[[str], str]:
    return getattr(c, "callback", lambda x: x.upper())

def nonliteral_name(c: C, name: str) -> Callable[[str], str]:
    return getattr(c, name, lambda x: x.upper())

def incompatible_body(c: object) -> Callable[[str], str]:
    return getattr(c, "callback", lambda x: x + 1)  # E: `+` is not supported between `str` and `Literal[1]`

def incompatible_body_control(c: object, name: str) -> Callable[[str], str]:
    return getattr(c, name, lambda x: x + 1)  # E: `+` is not supported between `str` and `Literal[1]`
    "#,
);

testcase!(
    test_builtin_getattr_non_special_call_shapes,
    r#"
from typing import Any, assert_type

class Foo:
    x: int

def test(foo: Foo) -> None:
    assert_type(getattr(*(foo, "x")), Any)
    assert_type(getattr(*[foo], "x"), Any)
    assert_type(getattr(foo, "x", *(None,)), Any | None)
    getattr(foo, "x", None, None)  # E:
    "#,
);

testcase!(
    test_object_setattr,
    r#"
from typing import assert_type

class Foo:
    def __getattr__(self, name: str) -> int: ...
    def __setattr__(self, name: str, value: int) -> None: ...

def test(foo: Foo) -> None:
    foo.x = 1
    foo.x = ""  # E: Argument `Literal['']` is not assignable to parameter `value` with type `int`
    "#,
);

testcase!(
    test_object_delattr,
    r#"
from typing import assert_type

class Foo:
    def __getattr__(self, name: str) -> int: ...
    def __delattr__(self, name: str) -> None: ...

def test(foo: Foo) -> None:
    del foo.x
    "#,
);

testcase!(
    test_getattr_does_not_provide_dunder_bool,
    r#"
# Implicit dunders like `__bool__` are looked up on the type and are not
# resolved through `__getattr__`, so a `__getattr__` returning a non-callable
# type must not make an instance look like it has a non-callable `__bool__`.
# Regression test for https://github.com/facebook/pyrefly/issues/4467
class Tensor: ...
class M:
    def __getattr__(self, name: str) -> "Tensor | M": ...

def f(x: M | None) -> M:
    return x or M()
    "#,
);

testcase!(
    test_getattribute_does_not_provide_dunder_bool,
    r#"
# The same rule applies to a custom `__getattribute__`: implicit `__bool__`
# lookups go through the type, not the instance's `__getattribute__`, so a
# `__getattribute__` returning a non-callable type must not make an instance
# look like it has a non-callable `__bool__`.
class Tensor: ...
class M:
    def __getattribute__(self, name: str) -> "Tensor | M": ...

def f(x: M | None) -> M:
    return x or M()
    "#,
);

testcase!(
    test_inherited_getattr_does_not_provide_dunder_bool,
    r#"
# The `__getattr__` fallback must be disabled for implicit `__bool__` even when
# `__getattr__` is inherited from a base class rather than defined directly.
class Tensor: ...
class Base:
    def __getattr__(self, name: str) -> "Tensor | Base": ...
class Sub(Base): ...

def f(x: Sub | None) -> Sub:
    return x or Sub()
    "#,
);

testcase!(
    test_object_setattr_wrong_signature,
    r#"
from typing import assert_type

class Foo:
    def __getattr__(self, name: int) -> int: ...
    def __setattr__(self, name: int, value: int) -> None: ...  # E: `Foo.__setattr__` overrides parent class `object` in an inconsistent manner

def test(foo: Foo) -> None:
    foo.x = 1  # E: Argument `Literal['x']` is not assignable to parameter `name` with type `int`
    "#,
);

testcase!(
    test_argparse_namespace_setattr,
    r#"
from argparse import ArgumentParser, Namespace

ap: ArgumentParser = ArgumentParser()
ap.add_argument("-b", "--bool-flag", default=False, action='store_true')
ap.add_argument("-i", "--integer", default=1, type=int)
ap.add_argument("-s", "--string-arg", type=str, default="")
args: Namespace = ap.parse_args()
if not args.string_arg:
    args.string_arg = "string-goes-here"
    "#,
);

testcase!(
    test_module_getattr,
    TestEnv::one("foo", "def __getattr__(name: str) -> int: ..."),
    r#"
from typing import assert_type
import foo
assert_type(foo.x, int)
assert_type(foo.y, int)
foo.x = 1  # E: No attribute `x` in module `foo`
del foo.y  # E: No attribute `y` in module `foo`
    "#,
);

testcase!(
    test_module_getattr_from_import,
    TestEnv::one("foo", "def __getattr__(name: str) -> int: ..."),
    r#"
from typing import assert_type
from foo import x, y
assert_type(x, int)
assert_type(y, int)
    "#,
);

testcase!(
    test_conditional_import_dynamic_unknown_is_preserved,
    {
        let mut env = TestEnv::new();
        env.add_with_path(
            "modern",
            "modern.pyi",
            "class Meta(type):\n    value: int\n    def method(self) -> str: ...",
        );
        env.add_with_path(
            "dynamic_fallback",
            "dynamic_fallback.pyi",
            "from typing import Any\ndef __getattr__(name: str) -> Any: ...",
        );
        env
    },
    r#"
try:
    from modern import Meta
except ImportError:
    from dynamic_fallback import Meta
class C(Meta):
    pass
from typing import Any, assert_type
assert_type(C.value, int)
assert_type(C.method(C), str)
assert_type(C.dynamic_attribute, Any)

def accepts_meta(value: Meta) -> None: ...
accepts_meta(C)
    "#,
);

fn test_env_with_incomplete_module() -> TestEnv {
    TestEnv::one_with_path(
        "foo",
        "foo.pyi",
        r#"
from _typeshed import Incomplete
def __getattr__(name: str) -> Incomplete: ...
"#,
    )
}

testcase!(
    test_module_getattr_stub_incomplete,
    test_env_with_incomplete_module(),
    r#"
from typing import assert_type, Any
from foo import x, y
# Incomplete is essentially Any, so x and y should be Any
assert_type(x, Any)
assert_type(y, Any)
    "#,
);

fn test_env_with_getattr_and_other_attribute() -> TestEnv {
    TestEnv::one_with_path(
        "foo",
        "foo.pyi",
        r#"
x: str
def __getattr__(name: str) -> int: ...
"#,
    )
}

testcase!(
    test_module_getattr_explicit_export_priority,
    test_env_with_getattr_and_other_attribute(),
    r#"
from typing import assert_type
from foo import x, y
# x is explicitly defined as str, should not use __getattr__
assert_type(x, str)
# y is not defined, should use __getattr__ and be int
assert_type(y, int)
    "#,
);

fn test_env_with_getattr_and_submodule() -> TestEnv {
    let mut env = TestEnv::new();
    env.add_with_path(
        "foo",
        "foo/__init__.pyi",
        "def __getattr__(name: str) -> int: ...",
    );
    env.add_with_path("foo.bar", "foo/bar.pyi", "");
    env
}

testcase!(
    test_submodule_takes_precedence_over_module_getattr,
    test_env_with_getattr_and_submodule(),
    r#"
from foo import bar  # submodule
from foo import baz  # non-existent attribute, should fall back to __getattr__
from typing import assert_type, reveal_type
reveal_type(bar)  # E: Module[foo.bar]
assert_type(baz, int)
    "#,
);

testcase!(
    test_any_subclass,
    r#"
from typing import Any, assert_type

class A(Any):
    x: int

class B(A):
    y: str

def test0(a: A, b: B, ta: type[A]) -> None:
    assert_type(a.x, int)
    assert_type(b.x, int)
    assert_type(b.y, str)
    assert_type(ta.mro(), list[type])

    assert_type(a.z, Any)
    assert_type(b.z, Any)
    assert_type(ta.z, Any)

class Test(B):
    def m(self) -> None:
        assert_type(super().z, Any)
    @classmethod
    def m2(cls) -> None:
        assert_type(super().z, Any)
    "#,
);

testcase!(
    test_any_as_base_class_suppresses_missing_attribute_in_method,
    r#"
from typing import Any
class MyTest(Any):
    def foo(self):
        self.bar()  # should not error: Any is in base-class hierarchy
"#,
);

// https://github.com/facebook/pyrefly/issues/2204
testcase!(
    test_generic_function_assigned_to_attribute,
    r#"
from typing import reveal_type, assert_type
def f[T](x: T) -> T:
    return x

class C:
    def m[U](self, x: U) -> U:
        return x

class D:
    def __init__(self, c: C):
        self.f = f
        self.g = c.m
        self.h = C.m

def test(o: D):
    reveal_type(o.f) # E: [T](x: T) -> T
    assert_type(o.f(1), int)

    reveal_type(o.g) # E: [U](x: U) -> U
    assert_type(o.g(1), int)
"#,
);

// https://github.com/facebook/pyrefly/issues/2812
testcase!(
    test_bound_overload_assigned_to_attribute,
    r#"
from typing import assert_type

class ThemeStack:
    def __init__(self) -> None:
        self._entries: list[dict[str, str]] = [{}]
        self.get = self._entries[-1].get

def test(stack: ThemeStack) -> None:
    assert_type(stack.get("theme"), str | None)
    assert_type(stack.get("theme", "fallback"), str)
"#,
);

// https://github.com/facebook/pyrefly/issues/3958
testcase!(
    test_property_returning_bound_method_preserves_signature,
    r#"
from typing import reveal_type

class Foo:
    def bar(
        self,
        simple_union: int | str,
        complex_union: int | str | tuple[int, str] | None = None,
    ) -> None: ...

class FooManager:
    def __init__(self) -> None:
        self.foo = Foo()

    @property
    def bar(self):
        return self.foo.bar

reveal_type(FooManager().bar)  # E: revealed type: (simple_union: int | str, complex_union: int | str | tuple[int, str] | None = None) -> None
"#,
);

testcase!(
    test_generic_function_as_closure_default_arg,
    r#"
import bisect

class Worker:
    def __init__(self) -> None:
        self.heartbeats: list[float] = []
        self.event = self._create_event_handler()

    def _create_event_handler(self):
        heartbeats = self.heartbeats

        def event(
            timestamp: float | None = None,
            insort = bisect.insort,
        ) -> None:
            if timestamp is not None:
                insort(heartbeats, timestamp)

        return event
"#,
);

testcase!(
    test_attr_unknown,
    r#"
class Op:
    default: str
class Namespace:
    def __getattr__(self, op_name):
        if op_name == "__file__":
            return "test"
        return Op()
x = Namespace().some_op.default # E:  Object of class `str` has no attribute `default
"#,
);

testcase!(
    test_with,
    r#"
class C:
    def __init__(self) -> None:
        self.prev = False
    def __enter__(self) -> None:
        self.prev = False
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.prev = False
    def __new__(cls, orig_func=None):
        if orig_func is None:
            return super().__new__(cls)
def f():
    with C():
        pass
    "#,
);

testcase!(
    test_class_nested_inside_generic_class,
    r#"
from typing import Any, assert_type
class Outer[T]:
    class Inner:
        x: T | None = None  # E: not in scope
assert_type(Outer[int].Inner, type[Outer.Inner])
assert_type(Outer.Inner, type[Outer.Inner])
assert_type(Outer[int].Inner.x, Any | None)
assert_type(Outer.Inner.x, Any | None)
assert_type(Outer[int].Inner().x, Any | None)
assert_type(Outer.Inner().x, Any | None)
   "#,
);

testcase!(
    test_attr_base,
    r#"
def f(x, key, dict):
    for param in x:
        if key in dict:
            pass
    "#,
);

testcase!(
    test_classvar_no_value,
    r#"
from typing import ClassVar, assert_type
class C:
    x: ClassVar[int]
assert_type(C.x, int)
    "#,
);

testcase!(
    test_set_attribute_on_typevar_annotated_self,
    r#"
from typing import Self, TypeVar
Self2 = TypeVar('Self2', bound='A')
class A:
    x: int
    def f1(self: Self, x: int, y: float) -> Self:
        self.x = x
        self.x = y  # E: `float` is not assignable to attribute `x` with type `int`
        return self
    def f2(self: Self2, x: int, y: float) -> Self2:
        self.x = x
        self.x = y  # E: `float` is not assignable to attribute `x` with type `int`
        return self
    def f3[Self3: A](self: Self3, x: int, y: float) -> Self3:
        self.x = x
        self.x = y  # E: `float` is not assignable to attribute `x` with type `int`
        return self
    "#,
);

testcase!(
    test_typevar_attr_default_error,
    r#"
from typing import Any, assert_type
class A[T1 = int, T2]:  # E:
    x: T2
def f(a: A):
    assert_type(a.x, Any)
    "#,
);

testcase!(
    test_union_base_class,
    r#"
from typing import Any, assert_type
class A:
    x: int
class B:
    x: str
try:
    C = A
except:
    C = B
class D(C): # E: Invalid base class: `A | B`
    pass
def f(d: D):
    assert_type(d.x, Any)
    "#,
);

testcase!(
    test_getattr_dispatch_for_metaclass,
    r#"
from typing import assert_type
class EMeta(type):
    def __getattr__(self, attr: str) -> int: ...
class E(int, metaclass=EMeta):
    pass
assert_type(E.EXAMPLE_VALUE, int)
    "#,
);

testcase!(
    test_getattr_selection_for_class_object_w_metaclass,
    r#"
from typing import assert_type
class EMeta(type):
    def __getattr__(self, attr: str) -> int: ...
class E(metaclass=EMeta):
    def __getattr__(self, attr: str) -> str: ...
assert_type(E.EXAMPLE_VALUE, int)
    "#,
);

testcase!(
    test_getattr_selection_for_class_object_no_metaclass,
    r#"
from typing import assert_type
class E:
    def __getattr__(self, attr: str) -> str: ...
E.EXAMPLE_VALUE # E: Class `E` has no class attribute `EXAMPLE_VALUE`
    "#,
);

testcase!(
    test_attribute_access_on_type_var,
    r#"
from typing import assert_type, Any
class Foo:
    def m(self) -> int:
        return 0
def f[T: Foo](y: T, z: type[T]) -> T:
    assert_type(y.m(), int)
    assert_type(z.m(y), int)
    assert_type(T.m(y), Any) # E: Object of class `TypeVar` has no attribute `m`
    return y
    "#,
);

testcase!(
    test_classmethod_on_instance_typevar_bound,
    r#"
from typing import Self

class A:
    @classmethod
    def bar(cls) -> Self: ...

def test[T: A](a: T) -> T:
    return a.bar()
    "#,
);

testcase!(
    test_attribute_access_on_quantified_bound_by_union,
    r#"
from typing import assert_type
class Foo:
    x: int
class Bar:
    x: str
def f[T: Foo | Bar](y: T, z: Foo | Bar) -> T:
    assert_type(z.x, int | str)
    assert_type(y.x, int | str)
    return y
    "#,
);

testcase!(
    test_attribute_access_on_type_none,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

ty(None).__bool__(None)
"#,
);

testcase!(
    test_attribute_access_on_type_literal,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

ty(0).bit_length(0)
"#,
);

testcase!(
    test_attribute_access_on_type_literalstring,
    r#"
from typing import LiteralString

# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

def test(x: LiteralString):
    ty(x).upper(x)
"#,
);

testcase!(
    test_private_attribute_outside_class,
    r#"
class A:
    __secret: int = 0

exposed = A.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class

class B:
    leaked = A.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class

class C:
    __secret: int = 0
    def reveal(self, a: A):
        return a.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class

class Outer:
    __secret: int = 0

    class Inner:
        def reveal(self, outer: "Outer") -> int:
            return outer.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class
"#,
);

testcase!(
    test_protected_attribute_access,
    TestEnv::new().enable_protected_access_error(),
    r#"
class A:
    _protected: int = 0

    def reveal(self, other: "A") -> int:
        return self._protected + other._protected

outside_instance = A()._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass
outside_class = A._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass

class B(A):
    inherited = A._protected

    def reveal(self, other: A) -> int:
        return self._protected + other._protected

class Unrelated:
    def leak(self, a: A) -> int:
        return a._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass
"#,
);

testcase!(
    test_protected_specialized_class_def,
    TestEnv::new().enable_protected_access_error(),
    r#"
class C[T]:
    _p: int = 0

C[int]._p  # E: Protected attribute `_p` cannot be accessed
Alias = C[int]
Alias._p  # E: Protected attribute `_p` cannot be accessed

def f() -> None:
    X = C[int]
    X._p  # E: Protected attribute `_p` cannot be accessed

class Child(C[int]):
    value = C[int]._p
"#,
);

testcase!(
    test_generic_alias_runtime_args,
    r#"
from typing import Any, assert_type

assert_type(tuple[int, str].__args__, tuple[Any, ...])
assert_type(tuple[int, ...].__args__, tuple[Any, ...])

class C[T]: ...
assert_type(C[int].__args__, tuple[Any, ...])

def f(cls: type[C[int]]) -> None:
    cls.__args__  # E: has no class attribute `__args__`
"#,
);

testcase!(
    test_same_named_protected_attribute_from_unrelated_class,
    TestEnv::new().enable_protected_access_error(),
    r#"
class A:
    _protected: int = 0

class B(A):
    def leak(self, other: "Unrelated") -> int:
        return other._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass

class Unrelated:
    _protected: int = 0
"#,
);

testcase!(
    test_private_attribute_inside_class,
    r#"
class A:
    __secret: int = 0

    def reveal(self) -> int:
        return self.__secret

    @classmethod
    def reveal_cls(cls) -> int:
        return cls.__secret

    @staticmethod
    def reveal_static() -> int:
        return A.__secret
"#,
);

testcase!(
    test_private_attribute_on_peer_instance,
    r#"
class F1:
    __v: int

    def equals(self, other: "F1") -> bool:
        return self.__v == other.__v
"#,
);

testcase!(
    test_private_attribute_in_subclass_method,
    r#"
class A:
    __secret: int = 0

class B(A):
    def leak(self) -> int:
        return self.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class
"#,
);

testcase!(
    test_unknown_access_of_private_attribute,
    r#"
class A:
    __secret: int = 0
    def get_secret(self, other):
        # OK: `other` may be an instance of `A`, which has a `__secret` attribute
        return other.__secret
    "#,
);

testcase!(
    test_private_attribute_in_function_in_method,
    r#"
class A:
    __secret: int = 0
    def get_secret(self):
        def get():
            return self.__secret
        return get()
    "#,
);

testcase!(
    test_private_attribute_in_lambda_and_comprehension_in_class_body,
    r#"
class A:
    __secret: int = 0
    instances: list["A"] = []
    secrets = [a.__secret for a in instances]
    get_secret = lambda: A.__secret
    "#,
);

testcase!(
    test_nonexistent_private_attribute,
    r#"
class A:
    pass
class B:
    def oops1(self):
        return self.__secret  # E: Object of class `B` has no attribute `__secret`
    def oops2(self, other: A):
        return other.__secret  # E: Object of class `A` has no attribute `__secret`
    "#,
);

testcase!(
    test_private_attribute_with_getattr,
    r#"
from typing import assert_type

class Foo:
    def __getattr__(self, attr: str) -> str:
        return attr

def main(f: Foo) -> None:
    assert_type(f._foo, str)
    assert_type(f.__foo, str)
    "#,
);

testcase!(
    test_private_attribute_with_getattr_and_explicit_field,
    r#"
class A:
    __secret: int = 0
    def __getattr__(self, name: str) -> str: ...

class B:
    def leak(self, a: A):
        return a.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class
    "#,
);

testcase!(
    test_private_attribute_with_getattribute,
    r#"
from typing import assert_type

class Foo:
    def __getattribute__(self, name: str) -> str:
        return name

def main(f: Foo) -> None:
    assert_type(f.__bar, str)
    "#,
);

testcase!(
    test_private_attribute_with_inherited_getattr,
    r#"
from typing import assert_type

class Base:
    def __getattr__(self, name: str) -> str:
        return name

class Child(Base):
    pass

def main(c: Child) -> None:
    assert_type(c.__foo, str)
    "#,
);

testcase!(
    test_private_attribute_inherited_field_with_child_getattr,
    r#"
class Parent:
    __secret: int = 0

class Child(Parent):
    def __getattr__(self, name: str) -> str: ...

class External:
    def leak(self, c: Child):
        return c.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class
    "#,
);

testcase!(
    test_private_attribute_mixed_union_with_getattr,
    r#"
class A:
    __secret: int = 0

class Dyn:
    def __getattr__(self, name: str) -> str: ...

class External:
    def leak(self, x: A | Dyn):
        return x.__secret  # E: Private attribute `__secret` cannot be accessed outside of its defining class
    "#,
);

// We allow __attr access on modules, since name mangling only occurs on attributes of classes.
testcase!(
    test_module_attr_is_not_private,
    TestEnv::one("foo", "__x: int = 0"),
    r#"
import foo
import types
print(foo.__x)

def f(mod1, mod2: types.ModuleType):
    print(mod1.__x)
    print(mod2.__x)

class A:
    def f(self, mod: types.ModuleType):
        print(foo.__x)
        print(mod.__x)
    "#,
);

testcase!(
    test_attribute_access_on_type_callable,
    r#"
from typing import Callable

# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

def test_callable(x: Callable[[], None]):
    ty(x).__call__(x)
"#,
);

testcase!(
    test_attribute_access_on_type_function,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

def foo(): ...

ty(foo).__call__(foo)
"#,
);

testcase!(
    test_attribute_access_on_type_boundmethod,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

class X:
    def m(self): ...

ty(X().m).__call__(X().m)
"#,
);

testcase!(
    test_attribute_access_on_type_overload,
    r#"
from typing import overload

# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

@overload
def bar(x: int) -> int: ...
@overload
def bar(x: str) -> str: ...
def bar(x: int | str) -> int | str: ...

ty(bar).__call__(bar)
"#,
);

testcase!(
    test_attribute_access_on_type_union,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

class A:
    x = 0
class B:
    x = "foo"

def test_union(x: A  | B):
    ty(x).x
"#,
);

testcase!(
    test_attribute_access_on_type_class,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

class C:
    @staticmethod
    def m(x: int): ...

ty(C).m(0) # E: Class `type` has no class attribute `m`
"#,
);

testcase!(
    test_attribute_access_on_generic_alias,
    r#"
# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

class C[T]:
    @classmethod
    def m(cls, x: T): ...

ty(C[int]).m(0)  # E: Class `GenericAlias` has no class attribute `m`
ty(C[int]).__args__

from types import GenericAlias
from typing import assert_type
assert_type(ty(C[int])(C, (str,)), GenericAlias)
    "#,
);

testcase!(
    test_attribute_access_on_custom_metaclass,
    r#"
class Meta(type):
    def m(self): ...

class A(metaclass=Meta):
    def a(self): ...

def f[T](x: T) -> type[T]:
    return type(x)

def g(x: type[A]):
    meta1 = f(A)
    meta2 = x.__class__
    # Technically, the types allow `meta1` and `meta2` to be subclasses of `Meta` rather than
    # `Meta` itself, but it's more helpful to use `Meta` here than a conservative fallback.
    meta1.m(A)
    meta2.m(A)
    # Make sure we don't accidentally allow lookup of an attribute of A.
    meta1.a  # E: `Meta` has no class attribute `a`
    meta2.a  # E: `Meta` has no class attribute `a`
    "#,
);

testcase!(
    test_attribute_access_on_metaclass_union,
    r#"
from typing import assert_type
class M1(type):
    x: int
class M2(type):
    x: str
class A(metaclass=M1): ...
class B(metaclass=M2): ...
def f(x: type[A | B]):
    cls = x.__class__
    assert_type(cls.x, int | str)
    "#,
);

testcase!(
    test_attribute_access_on_classdef_metaclass,
    r#"
class Meta(type):
    def __setattr__(cls, name: str, value: int) -> None: ...
    def __getattribute__(cls, name: str) -> object: ...

class C(metaclass=Meta):
    pass

C.__class__.__setattr__(C, "x", 0)
C.__class__.__setattr__(C, "x", "bad") # E: Argument `Literal['bad']` is not assignable to parameter `value` with type `int`
C.__class__.__getattribute__(C, "x")

class DefaultMeta:
    pass

DefaultMeta.__class__.mro(DefaultMeta)
"#,
);

testcase!(
    bug = "type[TypedDict()] should be type",
    test_attribute_access_on_type_typeddict,
    r#"
from typing import TypedDict

# handy hack to get a type[X] for any X
def ty[T](x: T) -> type[T]: ...

class TD(TypedDict):
    x: int

ty(TD(x = 0))(x = 0)
ty(TD).mro() # E: Missing argument `self` in function `type.mro`
"#,
);

testcase!(
    test_attribute_access_on_type_bare_typeddict_special_form,
    r#"
from typing import TypedDict
# `type[TypedDict]` is invalid as an annotation. After the error is reported,
# the inner `TypedDict` should be recovered to `Any` so attribute access on
# `cls` does not trigger an internal error. Value-position access on bare
# `TypedDict` is unaffected and should still error.
def f(cls: type[TypedDict]) -> None:  # E: `TypedDict` is not allowed in this context
    cls.__annotations__
    cls.__name__
    cls.__bases__
"#,
);

testcase!(
    test_type_magic_dunder_compare,
    r#"
def test(x: type[int], y: type[int]) -> None:
    # These are OK because `type` inherits `__eq__` and `__ne__` from `object`.
    x == y
    x != y

    # These are always OK
    x is y
    x is not y

    # These are not OK because the corresponding dunder methods are not defined on `type`
    x < y       # E: `<` is not supported between `type[int]` and `type[int]`
    x <= y      # E: `<=` is not supported between `type[int]` and `type[int]`
    x > y       # E: `>` is not supported between `type[int]` and `type[int]`
    x >= y      # E: `>=` is not supported between `type[int]` and `type[int]`
    x in y      # E: `in` is not supported between `type[int]` and `type[int]`
    x not in y  # E: `not in` is not supported between `type[int]` and `type[int]`
    "#,
);

testcase!(
    test_access_method_using_class_param_on_class,
    r#"
from typing import assert_type, reveal_type, Any
class A[T]:
    def f(self) -> T: ...
reveal_type(A.f) # E: revealed type: [T](self: A[T]) -> T
assert_type(A.f(A[int]()), int)
    "#,
);

testcase!(
    test_access_generic_method_using_class_param_on_class,
    r#"
from typing import assert_type, reveal_type, Any
class A[T]:
    def f[S](self, x: S) -> tuple[S, T]: ...
reveal_type(A.f) # E: revealed type: [T, S](self: A[T], x: S) -> tuple[S, T]
assert_type(A.f(A[int](), ""), tuple[str, int])
    "#,
);

testcase!(
    test_access_overloaded_method_using_class_param_on_class,
    r#"
from typing import assert_type, reveal_type, overload, Any
class A[T]:
    @overload
    def f(self) -> T: ...
    @overload
    def f(self, x: T | None) -> T: ...
    def f(self, x=None) -> Any: ...
reveal_type(A.f) # E: revealed type: Overload[ [T](self: A[T]) -> T [T](self: A[T], x: T | None) -> T ]
assert_type(A.f(A[int]()), int)
    "#,
);

testcase!(
    test_access_overloaded_staticmethod_using_class_param_on_class,
    r#"
from typing import assert_type, reveal_type, overload, Any
class A[T]:
    @overload
    @staticmethod
    def f(x: None = ...) -> None: ...
    @overload
    @staticmethod
    def f(x: T) -> T: ...
    @staticmethod
    def f(x = None) -> Any: ...
reveal_type(A.f) # E: revealed type: Overload[ (x: None = ...) -> None [T](x: T) -> T ]
assert_type(A.f(), None)
assert_type(A.f(0), int)
    "#,
);

testcase!(
    test_parametrized_class_init_call,
    r#"
from typing import reveal_type

class Foo[T]:
    def __init__(self, /) -> None:
        pass

class Bar[S](Foo[S]):
    def __init__(self, /) -> None:
        Foo[S].__init__(self)

Foo[int].__init__(Foo[int]())
    "#,
);

testcase!(
    test_invalid_augmented_assign_in_init,
    r#"
class C:
    def __init__(self):
        self.x += 5  # E: Object of class `C` has no attribute `x`
    "#,
);

testcase!(
    test_attributes_when_raw_class_field_type_contains_var,
    r#"
from typing import assert_type, Any
# This test is making sure we don't leak a `Var` into a ClassField, which can lead to nondeterminism.
class A:
    x = []
    y = []
assert_type(A().x, list[Any])
A().x = [42]
A().y = [42]
assert_type(A().y, list[Any])
    "#,
);

testcase!(
    test_read_only_frozen_dataclass,
    r#"
import dataclasses

@dataclasses.dataclass(frozen=True)
class FrozenData:
    x: int
    y: str

def f(d: FrozenData):
    d.x = 42  # E: Cannot set field `x`
    d.y = "new"  # E: Cannot set field `y`
    "#,
);

testcase!(
    test_read_only_namedtuple,
    r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

def f(p: Point):
    p.x = 10  # E: Cannot set field `x`
    p.y = 20  # E: Cannot set field `y`
    "#,
);

testcase!(
    test_read_only_annotation_typeddict,
    r#"
from typing_extensions import TypedDict, ReadOnly

class Config(TypedDict):
    name: ReadOnly[str]
    value: int

def f(c: Config):
    c["value"] = 42  # OK
    c["name"] = "new"  # E: Key `name` in TypedDict `Config` is read-only
    "#,
);

testcase!(
    test_nested_class_mutability,
    r#"
class Backend:
    class Options:
        pass
class Options2(Backend.Options):
    pass
Backend.Options = Options2  # E: A class object initialized in the class body is considered read-only
    "#,
);

testcase!(
    test_nested_class_inheritance,
    r#"
class Backend:
    class Options:
        pass
class ProcessGroupGloo(Backend):
    class Options(Backend.Options):
        pass
    "#,
);

testcase!(
    test_nested_class_inheritance_via_assignment,
    r#"
class Backend:
    class Options:
        pass
class Options2(Backend.Options):
    pass
class ProcessGroupGloo(Backend):
    Options = Options2
    "#,
);

testcase!(
    test_read_only_class_var,
    r#"
from typing import ClassVar, Final
class C:
    x: ClassVar[Final[int]] = 42  # E: `Final` may not be nested inside `ClassVar`
C.x = 43  # E: This field is marked as Final
    "#,
);

testcase!(
    test_classvar_final_nesting,
    r#"
from typing import ClassVar, Final
class C:
    x: Final[ClassVar[int]] = 1  # E: `ClassVar` may not be nested inside `Final`
    y: ClassVar[Final[int]] = 2  # E: `Final` may not be nested inside `ClassVar`
    z: Final[int] = 3
    w: ClassVar[int] = 4
    "#,
);

testcase!(
    test_final_qualifier_with_inherited_type,
    r#"
from typing import Final
class Parent:
    x: float = 1
class Child(Parent):
    x: Final = 2  # E: `Child.x` is read-only, but `Parent.x` is read-write
child = Child()
child.x = 3.0  # E: Cannot set field `x`
    "#,
);

testcase!(
    test_inherited_annotation_with_tuple_unpacking,
    r#"
from typing import assert_type
class Parent:
    x: float
    y: float
class Child(Parent):
    x, y = 3, 4
child = Child()
assert_type(child.x, float)
assert_type(child.y, float)
    "#,
);

testcase!(
    test_attr_cast,
    r#"
from typing import Self, cast, Any, assert_type

class C:
    outputs: list[Any]
    def f(self, other):
        other = cast(Self, other)
        assert_type(other, Self)
        assert_type(other.outputs, list[Any])
        len(self.outputs) == len(other.outputs)
    "#,
);

testcase!(
    test_attr_tuple,
    r#"
from typing import Any, Tuple

def g(ann) -> None:
    if ann is Tuple: ...
    ann.__module__
    "#,
);

testcase!(
    test_tuple_attribute_example,
    r#"
def f(obj, g, field_type, my_type,):
    assert issubclass(obj, tuple) and hasattr(obj, "_fields")
    for f in obj._fields:
        if isinstance(field_type, my_type) and g is not None:
            if g is None:
                raise ValueError(
                    f"{obj.__name__}."
                )
    "#,
);

testcase!(
    test_set_attr_in_child_class,
    r#"
from typing import assert_type

class A:
    def __init__(self):
        self.x = 0

class B(A):
    def f(self):
        self.x = ""  # E: `Literal['']` is not assignable to attribute `x` with type `int`

class C(A):
    def f(self):
        self.x: str = ""  # E: Class member `C.x` overrides parent class `A` in an inconsistent manner
    "#,
);

testcase!(
    test_method_sets_inherited_generic_field,
    r#"
# Regression test for a bug tracked in https://github.com/facebook/pyrefly/issues/774
from typing import assert_type, Any
class A[T]:
    x: T
class B(A[int]):
    def __init__(self, x: int):
        # The test is primarily verifying that we handle this implicit definition
        # correctly in the class field logic, when this is actually an inherited field.
        self.x = x
assert_type(B(42).x, int)
    "#,
);

testcase!(
    test_private_attr_assignment_in_constructor,
    r#"
from typing import assert_type

class Config:
    pass

class DerivedConfig(Config):
    def foo(self) -> None:
        print("hello")

class B:
    def __init__(self, config: Config) -> None:
        self.__config = config

class C(B):
    def __init__(self, config: DerivedConfig) -> None:
        self.__config = config

    def bar(self) -> None:
        assert_type(self.__config, DerivedConfig)
        self.__config.foo()
    "#,
);

testcase!(
    test_crtp_example, // CRTP = Curiously recurring template pattern
    r#"
from typing import Any, assert_type
class Node[T: "Node[Any]"]:
    children : tuple[T, ...]
class Expr(Node["Expr"]):
    ...
class Singleton(Expr):
    def __init__(self, v: Expr):
        self.children = (v,)
assert_type(Singleton(Expr()).children, tuple[Expr, ...])
    "#,
);

testcase!(
    test_mro_method,
    r#"
().mro()  # E: no attribute `mro`
tuple.mro()
type.mro()  # E: Missing argument `self`
    "#,
);

// How special forms are represented in typing.py is an implementation detail, but in practice,
// some of the representations are stable across Python versions. In particular, user code
// sometimes relies on some special forms being classes and Type behaving like builtins.type.
testcase!(
    test_special_forms,
    r#"
from typing import Callable, Generic, Protocol, Tuple, Type
def f1(cls):
    if cls is Callable:
        return cls.mro()
def f2(cls):
    if cls is Generic:
        return cls.mro()
def f3(cls):
    if cls is Protocol:
        return cls.mro()
def f4(cls):
    if cls is Tuple:
        return cls.mro()
def f5(cls, x: type):
    if cls is Type:
        return cls.mro(x)
    "#,
);

testcase!(
    test_get_type_new,
    r#"
from typing import cast, reveal_type
def get_type_t[T]() -> type[T]:
    return cast(type[T], 0)
def foo[T](x: type[T]):
    # mypy reveals the same thing we do (the type of `type.__new__`), while pyright reveals `Unknown`.
    reveal_type(get_type_t().__new__)  # E: Overload[ [Self@type: type](cls: type[Self@type], o: object, /) -> type[Any] [Self](cls: type[Self], name: str, bases: tuple[type[Any], ...], namespace: dict[str, Any], /, **kwds: Any) -> Self ]
    "#,
);

testcase!(
    test_any_value_lookup,
    r#"
from typing import Any
Any.foo.bar # E: Class `Any` has no class attribute `foo`
    "#,
);

// T, P, and Ts are values of type TypeVar, ParamSpec, and TypeVarTuple respectively.
// They should behave like values when we try to access attributes on them.
testcase!(
    test_typevar_value_lookups,
    r#"
from typing import Callable, TypeVar, ParamSpec, TypeVarTuple

def ty[T](x: T) -> type[T]: ...

T = TypeVar("T")
P = ParamSpec("P")
Ts = TypeVarTuple("Ts")

T.__name__
P.__name__
P.args.__origin__
P.kwargs.__origin__
Ts.__name__

ty(T).__name__
ty(P).__name__
ty(P.args).__origin__
ty(P.args).__origin__
ty(Ts).__name__

def f(g: Callable[P, T], ts: tuple[*Ts], *args: P.args, **kwargs: P.kwargs):
    args.count(1)
    kwargs.keys()

    ty(args).count(args, 1)
    ty(kwargs).keys(kwargs)

    T.__name__
    P.__name__
    P.args.__origin__
    P.kwargs.__origin__
    Ts.__name__

    ty(T).__name__
    ty(P).__name__
    ty(P.args).__origin__
    ty(P.args).__origin__
    ty(Ts).__name__
"#,
);

testcase!(
    test_type_never,
    r#"
from typing import Never, assert_type, reveal_type
def f() -> type[Never]: ...
reveal_type(f().mro) # E: () -> list[type[Any]]
assert_type(f().wut, Never)
    "#,
);

testcase!(
    bug = "We should note when a classmethod creates an implicit attribute that captures a type parameter",
    test_implicit_class_attribute_captures_method_tparam,
    r#"
from typing import reveal_type
class A:
    @classmethod
    def f[T](cls, x: T):
        cls.x = x
reveal_type(A.x)  # E: revealed type: T
    "#,
);

testcase!(
    test_lazy_class_attribute_init,
    r#"
from typing import assert_type
class C:
    @classmethod
    def m(cls):
        if hasattr(cls, "foo"):
            return cls.foo
        retval = "foo"
        cls.foo = retval
        return retval
assert_type(C.foo, str)
    "#,
);

// See https://github.com/facebook/pyrefly/issues/1448 for what this tests
// and discussion of approaches to handling `@functools.wraps` with return
// type inference.
testcase!(
    test_inferred_returns_from_functools_wraps,
    r#"
from typing import assert_type, Any
from functools import wraps
def decorator(func):
    @wraps(func)
    def wrapper(self, *args, **kwargs):
        return func(self, *args, **kwargs)
    return wrapper
class C:
    @decorator
    def f(self) -> int: ...
assert_type(C().f(), Any)
    "#,
);

testcase!(
    test_missing_attribute_suggests_similar_name,
    r#"
class Foo:
    value = 1

def f(obj: Foo) -> None:
    obj.vaule  # E: Object of class `Foo` has no attribute `vaule`\n  Did you mean `value`?
"#,
);

testcase!(
    test_missing_attribute_suggests_builtin_str_method,
    r#"
"".lowerr  # E: Object of class `str` has no attribute `lowerr`\n  Did you mean `lower`?
"#,
);

testcase!(
    test_missing_attribute_suggests_inherited,
    r#"
class Base:
    field = 1

class Child(Base):
    pass

def f(x: Child) -> None:
    x.filed  # E: Object of class `Child` has no attribute `filed`\n  Did you mean `field`?
"#,
);

testcase!(
    test_missing_attribute_suggests_typed_dict_field,
    r#"
from typing import TypedDict
class TD(TypedDict):
    foo: int

def f(x: TD) -> int:
    return x.fo  # E: Object of class `TD` has no attribute `fo`\n  Did you mean `foo`?
"#,
);

testcase!(
    test_missing_attribute_suggests_enum_member,
    r#"
from enum import Enum
class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

def f(x: Color) -> Color:
    return x.BLU  # E: Object of class `Color` has no attribute `BLU`\n  Did you mean `BLUE`?
"#,
);

testcase!(
    test_union_attribute_missing_no_suggestion,
    r#"
# When an attribute exists on some union members but not others,
# we shouldn't suggest similar attributes from the types that have it
def f(x: str | None):
    return x.split()  # E: Object of class `NoneType` has no attribute `split` # !E: Did you mean
"#,
);

testcase!(
    test_union_attribute_missing_no_suggestion_three_types,
    r#"
# Partial union failure with 3 types: attribute exists on 1, missing on 2
def f(x: str | int | None):
    return x.split()  # E: Object of type `int | str | None` has no attribute `split` # !E: Did you mean
"#,
);

testcase!(
    test_union_attribute_missing_no_suggestion_mostly_have_it,
    r#"
# Even if most types have the attribute, if ANY don't, skip suggestion
class A:
    upper: int
    lower: int
class B:
    upper: int
    lower: int
class C:
    def upper(self) -> str: ...
def f(x: C | A | B):
    # C has "upper" method, A and B have "upper" attribute
    # But C doesn't have "lower" attribute, A and B do
    x.lowerr  # E: Object of class `C` has no attribute `lowerr` # !E: Did you mean
"#,
);

testcase!(
    test_union_both_missing_should_suggest,
    r#"
# When an attribute is missing on ALL union members, we should still suggest
class A:
    value: int
class B:
    value: str
def f(x: A | B):
    return x.vaule  # E: Object of type `A | B` has no attribute `vaule`\n  Object of class `A` has no attribute `vaule`\n  Object of class `B` has no attribute `vaule`\n  Did you mean `value`?
"#,
);

testcase!(
    test_union_all_have_attribute_no_error,
    r#"
# When all union members have the attribute, there should be no error
class A:
    value: int
class B:
    value: str
def f(x: A | B):
    return x.value  # No error - both A and B have 'value'
"#,
);

testcase!(
    test_class_toplevel_inherited_attr_name,
    r#"
# at the class top level, inherited attribute names should be considered in scope
from typing import assert_type

class Foo:
    assert_type(__qualname__, str)
    assert_type(__module__, str)
    attr: int

class Bar(Foo):
    assert_type(attr, int)
    "#,
);

testcase!(
    test_set_attr_to_none,
    r#"
from typing import Any, assert_type
class A:
    def __init__(self):
        self.x = None
        self.y: None = None
    def set_x(self, x: int):
        self.x = x
    def set_y(self, y: int):
        self.y = y  # E: `int` is not assignable to attribute `y` with type `None`
def f(a: A):
    assert_type(a.x, Any | None)
    assert_type(a.y, None)
    "#,
);

testcase!(
    test_do_not_promote_explicit_literal_param,
    r#"
from typing import Literal, assert_type
class A:
    def __init__(self, answer: Literal[42]):
        self.answer = answer
def f(a: A):
    assert_type(a.answer, Literal[42])
    "#,
);

testcase!(
    test_do_not_promote_explicit_literal_union,
    r#"
from typing import Literal, assert_type
class File:
    def __init__(self, mode: Literal["read", "write"]):
        self.mode = mode
def f(fi: File):
    assert_type(fi.mode, Literal["read", "write"])
    "#,
);

testcase!(
    test_unannotated_attribute_tuple_literal_promotion,
    r#"
from typing import assert_type
class A:
    def __init__(self):
        self.x = (42, 42)
def f(a: A):
    assert_type(a.x, tuple[int, int])
    a.x = (0, 0)
    "#,
);

testcase!(
    test_always_promote_inferred_literalstring,
    r#"
from typing import assert_type
class A:
    def __init__(self):
        greeting = "hello"
        self.x = f"{greeting} world"
        self.y = f"{greeting} world" if greeting == "hello" else 42
def f(a: A):
    assert_type(a.x, str)
    assert_type(a.y, int | str)
    "#,
);

testcase!(
    test_promote_literalstring_method_attribute,
    r#"
from typing import Literal, assert_type

class SessionMiddleware:
    def __init__(
        self, same_site: Literal["lax", "strict", "none"] = "lax", domain: str | None = None
    ) -> None:
        self.security_flags = "httponly; samesite=" + same_site
        if domain is not None:
            self.security_flags += f"; domain={domain}"

def f(middleware: SessionMiddleware):
    assert_type(middleware.security_flags, str)
    "#,
);

testcase!(
    test_do_not_promote_addition_of_explicit_literal_strings,
    r#"
from typing import LiteralString, assert_type

class C:
    def __init__(self, x: LiteralString, y: LiteralString) -> None:
        self.z = x + y

def f(c: C):
    assert_type(c.z, LiteralString)
    "#,
);

testcase!(
    test_do_not_promote_augmented_addition_of_explicit_literal_strings,
    r#"
from typing import LiteralString, assert_type

class C:
    def __init__(self, x: LiteralString, y: LiteralString) -> None:
        z = x
        z += y
        self.attr = z

def f(c: C):
    assert_type(c.attr, LiteralString)
    "#,
);

testcase!(
    test_top_level_anonymous_typeddict,
    r#"
from typing import NotRequired, TypedDict
class TD(TypedDict):
    x: NotRequired[int]
class A:
    def __init__(self, check: bool):
        self.x = {"x": 0}
        self.y = {"x": 0} if check else 42
def f(a: A):
    x: TD = a.x
    # anoynmous typed dicts are promoted away when unioned
    y: dict[str, int] | int = a.y
    "#,
);

testcase!(
    test_nested_anonymous_typeddict,
    r#"
from typing import Any

def f() -> list[dict[str, Any]]:
    pets = [{"name": "Carmen", "age": 3}]
    return pets
    "#,
);

// Regression test for https://github.com/facebook/pyrefly/issues/1341
testcase!(
    test_optional_type_truthiness,
    r#"
class A[T]:
    def __init__(self):
        self.foo: T | None = None

class B(A[None]):
    def m(self, x: int | None):
        foo = self.foo
        if not foo:
            pass
    "#,
);

// Regression test for https://github.com/facebook/pyrefly/issues/417
testcase!(
    test_classmethod_inherited_no_missing_attribute,
    r#"
class Base:
    @classmethod
    def from_pretrained(cls, name: str) -> "Base":
        return cls()

class Derived(Base):
    pass

Derived.from_pretrained("model")
"#,
);

testcase!(
    test_classmethod_vararg_does_not_bind_self,
    r#"
class C:
    @classmethod
    def create(*args, **kwargs): ...

C.create(42)
C.create(a=42)
"#,
);

testcase!(
    test_method_preserves_typevar,
    r#"
def f[T: (bytes, str)](x: T, y: T) -> T: ...
def g[T: (bytes, str)](x: T) -> T:
    y = x.lower()
    return f(y, y)
    "#,
);

testcase!(
    test_method_returning_self_preserves_typevar,
    r#"
from typing import reveal_type, Self

class A:
    def f(self) -> Self:
        return self

def f[T](x: T) -> T:
    if isinstance(x, A):
        return x.f()
    return x
    "#,
);

testcase!(
    test_getitem_on_constrained_typevar,
    r#"
from typing import assert_type

class A:
    def __getitem__(self, key) -> "A": ...

class B:
    def __getitem__(self, key: str) -> A: ...

class C[T: (A, B)]:
    def f(self) -> T: ...
    def g(self):
        return self.f()[""][0]
    "#,
);

testcase!(
    test_multiple_assignments_in_same_method,
    r#"
from typing import assert_type
class A:
    def __init__(self):
        self.val = "string"
        self.val = 1
def f(a: A):
    assert_type(a.val, str | int)

class B:
    def __init__(self, x: bool):
        if x:
            self.val = "string"
        else:
            self.val = 1
def f2(b: B):
    assert_type(b.val, str | int)
    "#,
);

testcase!(
    test_assignments_across_multiple_recognized_methods,
    r#"
from typing import assert_type
class A:
    def __init__(self):
        self.val = "string"
    def __post_init__(self):
        self.val = 1
def f(a: A):
    assert_type(a.val, str | int)
    "#,
);

testcase!(
    test_class_level_attribute_priority,
    r#"
from typing import assert_type
class A:
    val: int = 1
    def __init__(self):
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
def f(a: A):
    assert_type(a.val, int)

class B:
    val = 1
    def __init__(self):
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
def f2(b: B):
    assert_type(b.val, int)
    "#,
);

testcase!(
    test_recognized_vs_unrecognized_methods,
    r#"
from typing import assert_type, reveal_type
class A:
    def __init__(self):
        self.val = 1
    def do_work(self):
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
def f(a: A):
    assert_type(a.val, int)

class B:
    def __init__(self):
        self.val = None  # W: This expression is implicitly inferred to be `Any | None`. Please provide an explicit type annotation.
    def do_work(self):
        self.val = "string"
def f2(b: B):
    reveal_type(b.val)  # E: revealed type: Unknown | None
    "#,
);

testcase!(
    test_explicit_annotation_override,
    r#"
from typing import assert_type

class A:
    def __init__(self):
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
        self.val: int = 1
def f(a: A):
    assert_type(a.val, int)

class B:
    def __init__(self):
        self.val: int = 1
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
def f2(b: B):
    assert_type(b.val, int)

class C:
    def __init__(self, cond: bool):
        self.val: int = 1
        if cond:
            self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
def f3(c: C):
    assert_type(c.val, int)

class D:
    def __init__(self, cond: bool):
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
        if cond:
            self.val: int = 1
def f4(d: D):
    assert_type(d.val, int)

class E:
    def __init__(self, cond: bool):
        self.val = None  # E: `None` is not assignable to attribute `val` with type `int`
        if cond:
            self.val: int = 1
def f5(e: E):
    assert_type(e.val, int)
    "#,
);

testcase!(
    test_none_union_conditional_initialization,
    r#"
from typing import assert_type, reveal_type

class A:
    def __init__(self, x: None | int):
        self._x = None  # W: This expression is implicitly inferred to be `Any | None`. Please provide an explicit type annotation.
        if x:
            self._x = x
def f(a: A):
    reveal_type(a._x)  # E: revealed type: int | Unknown | None

class B:
    def __init__(self, x: None | int):
        if x:
            self._x = x
        else:
            self._x = x
def f2(b: B):
    assert_type(b._x, int | None)

class C:
    def __init__(self, x: None | int):
        if not x:
            self._y = x
        else:
            self._y = x
def f3(c: C):
    assert_type(c._y, int | None)
    "#,
);

testcase!(
    test_class_method_receiver_union,
    r#"
from typing import reveal_type, assert_type, Literal

class A:
    def __new__(cls):
        cls.val = 1
        return super().__new__(cls)
    def __init__(self):
        self.val = "string"
def f(a: A):
    assert_type(a.val, int | str)
    assert_type(A.val, int | str)

class B:
    def __init__(self):
        self.val = "string"
    def __new__(cls):
        cls.val = 1
        return super().__new__(cls)
def g(b: B):
    assert_type(b.val, int | str)
    assert_type(B.val, int | str)
    "#,
);

testcase!(
    test_class_body_with_method_definition,
    r#"
from typing import Literal, assert_type

class A:
    val = 0
    def __new__(cls):
        cls.val = 1
        return super().__new__(cls)
    def __init__(self):
        self.val = "string"  # E: `Literal['string']` is not assignable to attribute `val` with type `int`
def f(a: A):
    assert_type(a.val, int)
    assert_type(A.val, int)

class B:
    val: int | str = 0
    def __new__(cls):
        cls.val = 1
        return super().__new__(cls)
    def __init__(self):
        self.val = "string"
        assert_type(self.val, Literal["string"])
def g(b: B):
    assert_type(b.val, int | str)
    assert_type(B.val, int | str)
    "#,
);

testcase!(
    test_none_first_assignment_adds_any,
    r#"
from typing import assert_type, Any, Literal
class C:
    def __init__(self, x: bool):
        self.foo = None
        if x:
            self.foo = 0
        else:
            self.foo = 1
        assert_type(self.foo, Literal[0, 1])
def f(c: C):
    # If the first assignment to a field is `None`, we infer `Any | None`
    assert_type(c.foo, int | Any | None)
    "#,
);

testcase!(
    test_recognized_methods_contribute_helpers_excluded,
    r#"
from typing import assert_type
class A:
    def __init__(self):
        self.val = 1
    def __post_init__(self):
        self.val = "string"
    def helper(self):
        self.val = b"bytes"  # E: `Literal[b'bytes']` is not assignable to attribute `val` with type `int | str`
def f(a: A):
    assert_type(a.val, int | str)
    "#,
);

testcase!(
    test_self_referential_attribute,
    r#"
from typing import Any, assert_type
class C:
    def m(self) -> None:
        # `self.x = [self.x]` is self-referential and never converges (each fixpoint
        # iteration nests another `list[...]`).
        self.x = [self.x]  # E: Fixpoint iteration did not converge
def f(c: C):
    assert_type(c.x, list[list[list[list[list[Any]]]]])
    "#,
);

testcase!(
    test_unknown_attribute_type_untyped_call,
    TestEnv::new().enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

class C:
    def __init__(self) -> None:
        self.x = untyped(1)  # E: implicitly inferred to be `Any`
"#,
);

testcase!(
    test_unknown_attribute_type_class_body_untyped_call,
    TestEnv::new().enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

class C:
    x = untyped(1)  # E: implicitly inferred to be `Any`
"#,
);

testcase!(
    test_unknown_attribute_type_annotated_no_error,
    TestEnv::new().enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

class C:
    def __init__(self) -> None:
        self.x: int = untyped(1)
"#,
);

testcase!(
    test_unknown_attribute_type_known_no_error,
    TestEnv::new().enable_unknown_attribute_type_error(),
    r#"
class C:
    def __init__(self) -> None:
        self.x = 1
        self.s = "hello"
"#,
);

testcase!(
    test_unknown_attribute_type_not_suppressed_by_implicit_any,
    TestEnv::new().enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

class C:
    def __init__(self) -> None:
        # pyrefly: ignore[implicit-any]
        self.x = untyped(1)  # E: implicitly inferred to be `Any`
"#,
);

testcase!(
    test_unknown_attribute_type_explicit_any_no_error,
    TestEnv::new().enable_unknown_attribute_type_error(),
    r#"
from typing import Any, cast
class C:
    def __init__(self) -> None:
        # An explicit `Any` is intentional, so only implicit `Any` should trigger.
        self.x = cast(Any, 1)
"#,
);

testcase!(
    test_implicit_any_attribute_class_body_no_double_report,
    TestEnv::new()
        .enable_unknown_variable_type_error()
        .enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

# With both rules enabled, a class-body attribute is reported ONLY by
# unknown-attribute-type, not unknown-variable-type (the class-body guard on
# unknown-variable-type prevents a double report). Exactly one error must fire here.
class C:
    x = untyped(1)  # E: implicitly inferred to be `Any`
"#,
);

// Class attributes bound by unpacking, a `for` loop or `with` are reported the same way as a
// plain class-body assignment. A comprehension has its own scope, so its target is a variable.
testcase!(
    test_unknown_attribute_type_class_body_other_targets,
    TestEnv::new()
        .enable_unknown_variable_type_error()
        .enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

class C:
    a, b = untyped(1)  # E: implicitly inferred to be `Any`  # E: implicitly inferred to be `Any`
    for loop_var in untyped(1):  # E: implicitly inferred to be `Any`
        pass
    with untyped(1) as ctx:  # E: implicitly inferred to be `Any`
        pass
    comp = [h for h in untyped(1)]  # E: The type of `h` is unknown
    rebound = 1
    for rebound in untyped(1):  # E: implicitly inferred to be `Any`
        pass
"#,
);

// Imports, walrus targets and match captures in a class body are not reported, as at other scopes.
testcase!(
    test_unknown_attribute_type_class_body_other_bindings_no_error,
    TestEnv::one(
        "m",
        "def untyped(x):\n    return x\nvalue = untyped(1)  # E: The type of `value` is unknown\n",
    )
    .enable_unknown_variable_type_error()
    .enable_unknown_attribute_type_error(),
    r#"
from m import untyped

class C:
    from m import value
    (walrus := untyped(1))
    match untyped(1):
        case captured:
            pass
"#,
);

// In a function, each of these bindings is reported as `unknown-variable-type`. A class attribute
// is only checked through its final value, which merges these bindings, so none are reported.
testcase!(
    bug = "class attributes merged from several bindings are not reported",
    test_unknown_attribute_type_class_body_merged_bindings,
    TestEnv::new()
        .enable_unknown_variable_type_error()
        .enable_unknown_attribute_type_error(),
    r#"
def untyped(x):
    return x

class C:
    try:
        merged = untyped(1)
    except Exception:
        merged = untyped(2)
    augmented = untyped(1)
    augmented += 1
    if untyped(1):
        c, d = untyped(1)
    else:
        c, d = untyped(2)
    for o in untyped(1):
        o = untyped(3)
"#,
);

testcase!(
    test_bound_method_no_arbitrary_attr_set,
    r#"
class Foo:
    def real_method(self) -> None: ...
f: Foo = Foo()
f.real_method.test = None  # E: has no attribute `test`
del f.real_method.test  # E: has no attribute `test`
# Known method attributes are still accessible.
name: str = f.real_method.__name__
"#,
);

testcase!(
    test_getattr_self_awaitable_recursion,
    r#"
from collections.abc import Awaitable

class C:
    def __getattr__(self: Awaitable, name: str):
        pass

C().a  # E: Argument `C` is not assignable to parameter `self` with type `Awaitable[Unknown]` in function `C.__getattr__`
"#,
);

testcase!(
    test_getattr_self_protocol_generic_cache_order,
    r#"
from typing import Protocol, TypeVar

T = TypeVar("T", covariant=True)

class P(Protocol[T]):
    @property
    def x(self) -> T: ...

class C:
    def __getattr__(self: "P[str]", name: str) -> int:
        return 0

def f_int(x: P[int]) -> None: ...
def f_str(x: P[str]) -> None: ...

f_int(C())
f_str(C())  # E: Argument `C` is not assignable to parameter `x` with type `P[str]` in function `f_str`
"#,
);
// A field defined by assignment in several methods takes the union of the assigned types, built
// through the solver, so complementary bool literals collapse to `bool`.
testcase!(
    test_deferred_field_unions_bool_literals,
    r#"
from typing import Literal, assert_type
def rt() -> Literal[True]: ...
def rf() -> Literal[False]: ...
class K:
    def a(self) -> None:
        self.x = rt()
    def b(self) -> None:
        self.x = rf()
def probe(k: K) -> None:
    assert_type(k.x, bool)
"#,
);

// A literal is absorbed by its own class when both are assigned. The two are equivalent, so
// only the rendered form differs and `reveal_type` is what can observe it.
testcase!(
    test_deferred_field_unions_literal_with_class,
    r#"
from typing import Literal, reveal_type
def r1() -> Literal[1]: ...
def ri() -> int: ...
class K:
    def a(self) -> None:
        self.x = r1()
    def b(self) -> None:
        self.x = ri()
def probe(k: K) -> None:
    reveal_type(k.x)  # E: revealed type: int
"#,
);

// Assigning every member of an enum gives the enum class rather than a union of its members.
testcase!(
    test_deferred_field_promotes_exhaustive_enum,
    r#"
from enum import Enum
from typing import Literal, assert_type
class E(Enum):
    A = 1
    B = 2
def ra() -> Literal[E.A]: ...
def rb() -> Literal[E.B]: ...
class K:
    def a(self) -> None:
        self.x = ra()
    def b(self) -> None:
        self.x = rb()
def probe(k: K) -> None:
    assert_type(k.x, E)
"#,
);

// A promoted string literal and an explicitly annotated `LiteralString` are the same type once
// the wider one absorbs the narrower, so the field's natural type is `str`.
testcase!(
    test_deferred_field_absorbs_literal_string,
    r#"
from typing import LiteralString, reveal_type
def fs() -> LiteralString: ...
class K:
    def a(self) -> None:
        self.x = "a"
    def b(self) -> None:
        self.x = fs()
def use(k: K, s: str) -> None:
    reveal_type(k.x)  # E: revealed type: str
    k.x = s
"#,
);

// Unioning the assigned values merges the submodules reachable through them, so the field's type
// is a module value equal to neither assignment, and assigning either one back is rejected. A
// module value carries the submodules reachable through it, and two such values are compared
// structurally, so a merged one matches neither input. The same defect is reachable without any
// class field, through a function that returns a module from two branches.
testcase!(
    bug = "A merged module value is not assignable to the field it was inferred from",
    test_deferred_field_unions_module_values,
    r#"
class C:
    def a(self) -> None:
        import os
        self.x = os  # E: `Module[os]` is not assignable to attribute `x` with type `Module[os]`
    def b(self) -> None:
        import os.path
        self.x = os  # E: `Module[os]` is not assignable to attribute `x` with type `Module[os]`
"#,
);

// A field assigned an explicit `Any` in one method and an un-inferable value in another unions the
// two. The solver's union collapses a union of `Any`s to a single member, keeping the highest
// `AnyStyle`, and `Implicit` outranks `Explicit`, so the field is `Unknown` where the heap-only
// union left `Any | Unknown`. The difference reaches users: `Unknown` trips lints such as
// `unknown-argument-type` that an explicit `Any` suppresses.
testcase!(
    test_field_unions_explicit_any_with_unknown,
    r#"
from typing import Any, reveal_type

class C:
    def m(self, x: Any) -> None:
        self.f = x
    def n(self, y) -> None:
        self.f = y

def use(c: C) -> None:
    reveal_type(c.f)  # E: revealed type: Unknown
"#,
);

testcase!(
    test_unknown_attribute_access,
    TestEnv::new().enable_unknown_attribute_access_error(),
    r#"
class C:
    def __init__(self, v) -> None:
        self.known: int = 1
        self.unknown = v

def use(obj, c: C) -> None:
    obj.attr  # E: The type of attribute `attr` is unknown
    obj.method(1)  # E: The type of attribute `method` is unknown
    obj.attr.deeper  # E: The type of attribute `attr` is unknown  # E: The type of attribute `deeper` is unknown
    c.unknown  # E: The type of attribute `unknown` is unknown
    c.known
"#,
);

testcase!(
    test_unknown_attribute_access_no_error,
    TestEnv::new().enable_unknown_attribute_access_error(),
    r#"
from typing import Any

class C:
    x: int = 1

def use(obj, a: Any, c: C) -> None:
    a.attr  # An explicit `Any` is intentional.
    c.missing  # E: Object of class `C` has no attribute `missing`
    obj.attr = 1  # Assignment targets are not accesses.
    c.x
"#,
);

testcase!(
    test_protected_attribute_ignored_by_default,
    r#"
class A:
    _protected: int = 0

value = A()._protected
"#,
);

testcase!(
    test_protected_attribute_dynamic_values,
    TestEnv::new().enable_protected_access_error(),
    r#"
from typing import Any

class Dynamic:
    def __getattr__(self, name: str) -> Any: ...

def outside(value: Any, cls: type[Any], dynamic: Dynamic):
    value._protected
    cls._protected
    dynamic._protected

class A:
    _protected: int = 0

    def inside(self, value: Any, cls: type[Any], dynamic: Dynamic):
        value._protected
        cls._protected
        dynamic._protected
"#,
);

testcase!(
    test_protected_attribute_from_stub,
    {
        let mut env = TestEnv::new().enable_protected_access_error();
        env.add("source", "class SourceBase:\n    _protected: int = 0\n");
        env.add_with_path(
            "lib",
            "lib.pyi",
            "from source import SourceBase\nclass A:\n    _protected: int\n    __private: int\nclass StubDerived(SourceBase): ...\n",
        );
        env
    },
    r#"
from lib import A, StubDerived

value = A()._protected
A()._protected = 1
A().__private  # E: Private attribute `__private` cannot be accessed outside of its defining class
StubDerived()._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass

class B:
    def method(self, value: A):
        return value._protected

class Derived(A):
    pass

inherited = Derived()._protected

class Override(A):
    _protected: int = 1

Override()._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass

class Source:
    _protected: int = 1

def read(value: A | Source) -> int:
    return value._protected  # E: Protected attribute `_protected` cannot be accessed outside of its defining class or a subclass
"#,
);

testcase!(
    test_protected_attribute_on_bounded_type_variable,
    TestEnv::new().enable_protected_access_error(),
    r#"
class A:
    _protected: int = 0

    def read[T: A](self, other: T) -> int:
        return other._protected
"#,
);

testcase!(
    test_protected_attribute_quoted_cast_default,
    r#"
from typing import Any, assert_type, cast

class Owner:
    _value: int = 1

def read(value: Any) -> None:
    assert_type(cast("Owner", value)._value, int)
    assert_type(cast(Owner, value)._value, int)
"#,
);

testcase!(
    test_protected_attribute_quoted_cast_enabled,
    TestEnv::new().enable_protected_access_error(),
    r#"
from typing import Any, cast

class Owner:
    _value: int = 1

def read(value: Any) -> None:
    cast("Owner", value)._value  # E: Protected attribute `_value` cannot be accessed outside of its defining class or a subclass
    cast(Owner, value)._value  # E: Protected attribute `_value` cannot be accessed outside of its defining class or a subclass
"#,
);

testcase!(
    test_protected_attribute_named_tuple_public_api,
    TestEnv::new().enable_protected_access_error(),
    r#"
from collections import namedtuple
from typing import NamedTuple
from urllib.parse import urlsplit

class Record(NamedTuple):
    value: int

    def _internal(self) -> int:
        return self.value

record = Record(1)
record._replace(value=2)
record._asdict()
Record._make([1])
record._fields
Record._field_defaults
record._internal()  # E: Protected attribute `_internal` cannot be accessed outside of its defining class or a subclass

Row = namedtuple("Row", ["value"])
Row(1)._replace(value=2)
Row(1)._asdict()
urlsplit("https://example.com/a")._replace(scheme="file")

class Other:
    def _replace(self) -> None: ...

Other()._replace()  # E: Protected attribute `_replace` cannot be accessed outside of its defining class or a subclass
"#,
);

testcase!(
    test_protected_attribute_enclosing_class_scopes,
    TestEnv::new().enable_protected_access_error(),
    r#"
from typing import assert_type

class _CodeWriter:
    _indent: int = 1

    def indent(self):
        class Indenter:
            def __exit__(_, *args):
                assert_type(self._indent, int)
        return Indenter()

class Base:
    _value: int = 1

class HeaderBase:
    def __init_subclass__(cls, flag: int) -> None: ...

class InHeader(Base, HeaderBase, flag=Base._value):  # E: Protected attribute `_value` cannot be accessed outside of its defining class or a subclass
    pass

class Derived(Base):
    class Inner:
        value = Base._value

        def read(self, other: Base) -> int:
            def nested() -> int:
                return other._value
            return nested()

class Container:
    class First:
        _value: int = 1

    class Second:
        def read(self, other: "Container.First") -> int:
            return other._value  # E: Protected attribute `_value` cannot be accessed outside of its defining class or a subclass

class Unrelated:
    class Inner:
        def read(self, other: Base) -> int:
            return other._value  # E: Protected attribute `_value` cannot be accessed outside of its defining class or a subclass

def outside(other: Base) -> int:
    def nested() -> int:
        return other._value  # E: Protected attribute `_value` cannot be accessed outside of its defining class or a subclass
    return nested()
"#,
);

// The exemption for conditional methods requires every binding of the name in the class body
// to be a `def`, even when a later `def` replaces the other binding.
testcase!(
    test_conditional_method_with_other_binding,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def coin() -> bool: ...

class AssignedThenDefined:
    if coin():
        value = 1  # E: Attribute `value` may be uninitialized
        def value(self) -> int: ...

class DeletedThenDefined:
    if coin():
        def value(self) -> int: ...  # E: Attribute `value` may be uninitialized
        del value
        def value(self) -> int: ...
"#,
);

testcase!(
    test_instance_var_assign_protocol_policy_default,
    r#"
from typing import Protocol

class P(Protocol):
    x: int

class C:
    x: int = 0

def f(p: P) -> None:
    pass

f(C)  # E: Argument `type[C]` is not assignable to parameter `p` with type `P`
f(C())
"#,
);

testcase!(
    test_instance_var_assign_protocol_policy_enabled,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import Protocol

class P(Protocol):
    x: int

class C:
    x: int = 0

def f(p: P) -> None:
    pass

f(C)  # E: Argument `type[C]` is not assignable to parameter `p` with type `P`
f(C())
"#,
);

// `instance-var-assign` (issue #3750): assigning to a class-body-declared
// instance variable (annotated, not `ClassVar`) via the class object is flagged
// when the kind is enabled. Inherited fields and fields annotated with a default
// are still instance variables, so they are flagged too. The existing
// bad-assignment check still runs (both fire on separate paths).
testcase!(
    test_instance_var_assign_diagnostic,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import ClassVar

class C:
    instance_var: int
C.instance_var = 3  # E: Cannot set instance variable `instance_var` from the class

# Annotated with a default: still an instance variable per the typing spec.
class WithDefault:
    x: int = 3
WithDefault.x = 4  # E: Cannot set instance variable `x` from the class

# Inherited fields keep their provenance through the MRO lookup.
class Sub(C):
    pass
Sub.instance_var = 5  # E: Cannot set instance variable `instance_var` from the class

# A type-incompatible value yields BOTH instance-var-assign and bad-assignment.
C.instance_var = "oops"  # E: Cannot set instance variable `instance_var` from the class  # E: `Literal['oops']` is not assignable to attribute `instance_var` with type `int`

# Suppressible via the standard directive.
C.instance_var = 7  # pyrefly: ignore[instance-var-assign]
    "#,
);

testcase!(
    test_instance_var_assign_typed_dict,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import NotRequired, TypedDict, assert_type

class TD(TypedDict):
    x: int
    optional: NotRequired[int]

class Child(TD):
    y: str

class Grandchild(Child):
    pass

assert_type(TD.x, int)
TD.x = 1  # E: Cannot set instance variable `x` from the class
Child.x = 2  # E: Cannot set instance variable `x` from the class
Grandchild.x = 3  # E: Cannot set instance variable `x` from the class
Child.y = "value"  # E: Cannot set instance variable `y` from the class
TD.optional = 4  # E: Cannot set instance variable `optional` from the class
TD.x = "oops"  # E: Cannot set instance variable `x` from the class  # E: `Literal['oops']` is not assignable to attribute `x` with type `int`
TD.x = 5  # pyrefly: ignore[instance-var-assign]

class Partial(TypedDict, total=False):
    x: int

Partial.x = 6  # E: Cannot set instance variable `x` from the class

# Reads and dictionary-item writes are unaffected.
td: TD = {"x": 1}
assert_type(td["x"], int)
td["x"] = 2

class Metadata(TypedDict):
    __module__: str
    __doc__: str | None
    __annotations__: dict[str, object]

Metadata.__module__ = "renamed"
Metadata.__doc__ = "Documentation"
Metadata.__annotations__ = {}

class MetadataChild(Metadata):
    pass

MetadataChild.__module__ = "renamed"
MetadataChild.__doc__ = None
MetadataChild.__annotations__ = {}
"#,
);

// Non-instance-variable fields must NOT trigger the new kind even when it is
// enabled: unannotated assignments and ClassVars are class variables,
// staticmethods become unannotated ClassAttributes, instance-only attributes
// keep their existing no-access error, and instance writes are unaffected.
testcase!(
    test_instance_var_assign_non_instance_fields_not_flagged,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import ClassVar, Generic, TypeVar

class C:
    unannotated = 1
    cv: ClassVar[int] = 2

    @staticmethod
    def stat() -> int:
        return 3

def f() -> int:
    return 4

T = TypeVar("T")


class G(Generic[T]):
    item: T


C.unannotated = 5
C.cv = 6
C.stat = f

# Writing a ClassVar via an instance keeps firing the pre-existing mirror rule.
C().cv = 7  # E: Cannot set field `cv`

# Instance-only attributes keep their pre-existing error on class access.
class D:
    def __init__(self) -> None:
        self.only = 1
D.only = 56  # E: Instance-only attribute `only` of class `D` is not visible on the class
D().only = 57

# Generic fields are unreachable via the class object (pre-existing no-access).
G.item = 6  # E: Generic attribute `item` of class `G` is not visible on the class

# Reads and instance writes never trigger this diagnostic.
class E:
    read_only_use: int = 1
print(E.read_only_use)
print(E().read_only_use)
E().read_only_use = 2
    "#,
);

// `instance-var-assign` is off by default: assignments that WOULD be flagged in
// strict mode emit nothing under the default configuration.
testcase!(
    test_instance_var_assign_off_by_default,
    r#"
from typing import Literal, TypedDict, assert_type

class C:
    instance_var: int
C.instance_var = 3

class TD(TypedDict):
    x: int

class Child(TD):
    pass

TD.x = 1
Child.x = 2
assert_type(TD.x, Literal[1])
assert_type(Child.x, Literal[2])
    "#,
);

// Writes through a class object that resolve via the metaclass are metaclass-
// instance writes and are never flagged: a class is an instance of its
// metaclass, so assigning metaclass-declared fields (including `__name__`)
// through the class object is valid.
testcase!(
    test_instance_var_assign_metaclass_writes_not_flagged,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
class Meta(type):
    meta_field: int

class UsesMeta(metaclass=Meta):
    instance_var: int

UsesMeta.meta_field = 1
UsesMeta.__name__ = "Renamed"

# A plain class reads `__name__` from `type` through the metaclass fallback.
class Plain:
    instance_var: int

Plain.__name__ = "Renamed"

# Inherited metaclass fields stay metaclass-instance writes.
class SubUsesMeta(UsesMeta):
    pass

SubUsesMeta.meta_field = 2
    "#,
);

// Attributes created by assigning through `cls` in a classmethod are set on the
// class itself, so they are class attributes, not class-body instance variables.
testcase!(
    test_instance_var_assign_classmethod_created_attr_not_flagged,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
class C:
    @classmethod
    def make(cls) -> None:
        cls.extra: int = 0

C.extra = 1
    "#,
);

// Class-body instance variables whose annotation is a union containing
// descriptors keep their provenance through the descriptor-read conversion, so
// writes via the class are still flagged.
testcase!(
    test_instance_var_assign_descriptor_union_flagged,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
class Desc:
    def __get__(self, instance: object, owner: type) -> int:
        return 0
    def __set__(self, instance: object, value: int) -> None:
        pass

class C:
    x: "int | Desc"

C.x = 1  # E: Cannot set instance variable `x` from the class
C().x = 2
    "#,
);

testcase!(
    test_instance_var_assign_class_metadata,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
class C:
    pass

C.__module__ = "renamed"
C.__doc__ = "Documentation"
C.__annotations__ = {}
C.__name__ = "Renamed"
C.__qualname__ = "Renamed"

class Meta(type):
    pass

class Custom(metaclass=Meta):
    pass

Custom.__module__ = "renamed"
Custom.__doc__ = "Documentation"
Custom.__annotations__ = {}

class Explicit:
    __module__: str = "original"
    __doc__: str | None = "Original documentation"
    __annotations__: dict[str, object] = {}
    ordinary: int = 0

Explicit.__module__ = "renamed"
Explicit.__doc__ = "Documentation"
Explicit.__annotations__ = {}
Explicit.ordinary = 1  # E: Cannot set instance variable `ordinary` from the class
"#,
);

testcase!(
    test_instance_var_assign_inherited_classmethod,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import ClassVar

class Base:
    x: int = 0
    cv: ClassVar[int] = 0

class Child(Base):
    @classmethod
    def make(cls) -> None:
        cls.x = 1  # E: Cannot set instance variable `x` from the class
        cls.cv = 1
        cls.fresh: int = 1

Child.x = 2  # E: Cannot set instance variable `x` from the class
Child.cv = 2
Child.fresh = 2
Base.x = 3  # E: Cannot set instance variable `x` from the class

class Grandchild(Child):
    @classmethod
    def make_again(cls) -> None:
        cls.x = 3  # E: Cannot set instance variable `x` from the class
        cls.fresh = 3

Grandchild.x = 4  # E: Cannot set instance variable `x` from the class
Grandchild.fresh = 4

class Direct(Base):
    x: int = 0
    @classmethod
    def make(cls) -> None:
        cls.x = 1  # E: Cannot set instance variable `x` from the class

Direct.x = 2  # E: Cannot set instance variable `x` from the class
"#,
);

testcase!(
    bug = "Metaclass data descriptor reads still use class fields",
    test_instance_var_assign_metaclass_descriptor_collision,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import Literal, assert_type

class Meta(type):
    @property
    def x(cls) -> int:
        return 0
    @x.setter
    def x(cls, value: int) -> None:
        pass

class C(metaclass=Meta):
    x: int = 0

C.x = 1
assert_type(C.x, Literal[1])

class Child(C):
    pass

Child.x = 2
assert_type(Child.x, Literal[2])
"#,
);

testcase!(
    test_instance_var_assign_narrowing_default,
    r#"
from typing import Literal, assert_type

class C:
    x: int | None = None

C.x = 1
assert_type(C.x, Literal[1])

def f() -> int:
    if C.x is None:
        C.x = 2
    return C.x
"#,
);

testcase!(
    test_instance_var_assign_narrowing_enabled,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from typing import Literal, assert_type

class C:
    x: int | None = None

C.x = 1  # E: Cannot set instance variable `x` from the class
assert_type(C.x, Literal[1])
C.x = 2  # pyrefly: ignore[instance-var-assign]
assert_type(C.x, Literal[2])

def f() -> int:
    if C.x is None:
        C.x = 3  # pyrefly: ignore[instance-var-assign]
    return C.x
"#,
);

testcase!(
    bug = "Metaclass data descriptor reads still use class fields",
    test_metaclass_descriptor_kinds_precedence,
    TestEnv::new().enable_instance_var_assign_error(),
    r#"
from functools import cached_property
from typing import Literal, assert_type

class Data:
    def __get__(self, instance: object, owner: type) -> str:
        return "data"
    def __set__(self, instance: object, value: int) -> None:
        pass

class NonData:
    def __get__(self, instance: object, owner: type) -> str:
        return "non-data"

class DeleteOnly:
    def __get__(self, instance: object, owner: type) -> str:
        return "delete-only"
    def __delete__(self, instance: object) -> None:
        pass

class Meta(type):
    data = Data()
    non_data = NonData()
    delete_only = DeleteOnly()
    @property
    def readonly(cls) -> str:
        return "readonly"
    @cached_property
    def cached(cls) -> str:
        return "cached"

class C(metaclass=Meta):
    data: int = 0
    non_data: int = 0
    delete_only: int = 0
    readonly: int = 0
    cached: int = 0

assert_type(C.data, int)
assert_type(C.non_data, int)
assert_type(C.delete_only, int)
assert_type(C.readonly, int)
C.data = 1
C.non_data = 1  # E: Cannot set instance variable `non_data` from the class
C.delete_only = 1
C.readonly = 1
assert_type(C.cached, int)
C.cached = 1  # E: Cannot set instance variable `cached` from the class
assert_type(C.cached, Literal[1])
"#,
);

testcase!(
    test_self_attribute_repeated_compatible_annotation,
    r#"
class A:
    x: int
    def __init__(self) -> None:
        self.x: int = 0
    def reset(self) -> None:
        self.x: int = 1
class B:
    def __init__(self) -> None:
        self.y: int = 0
    def reset(self) -> None:
        self.y: int = 1
    "#,
);

testcase!(
    test_self_attribute_redefinition_ignores_other_receivers,
    r#"
class Other:
    x: str
class A:
    def __init__(self) -> None:
        self.x: int = 0
    @staticmethod
    def build(other: Other) -> None:
        other.x: str = ""  # E: Cannot annotate non-self attribute `other.x`
    def nested(self) -> None:
        def inner(self: Other) -> None:
            self.x: str = ""  # E: Cannot annotate non-self attribute `self.x`
    "#,
);

testcase!(
    test_self_attribute_redefinition_constructor_ordering,
    r#"
class A:
    def reset(self) -> None:
        self.x: int = 1  # E: `x` cannot be annotated with `int`, it is already defined with type `int | None`
    def __init__(self) -> None:
        self.x: int | None = None
"#,
);

testcase!(
    test_self_attribute_compatible_unions_and_aliases,
    r#"
type AliasInt = int
class A:
    def __init__(self) -> None:
        self.x: int | str = 0
        self.y: AliasInt = 0
    def reset(self) -> None:
        self.x: str | int = ""
        self.y: int = 1
"#,
);

testcase!(
    test_self_attribute_scoped_type_alias_redefinition,
    r#"
class T1: ...
class T2: ...
class A:
    def __init__(self) -> None:
        T = T1
        self.x: T
    def reset(self) -> None:
        T = T2
        self.x: T  # E: `x` cannot be annotated with `T2`, it is already defined with type `T1`
"#,
);

testcase!(
    test_self_attribute_class_body_reannotation_and_literal_narrowing,
    r#"
from typing import Literal
class A:
    x: int
    y: Literal[0] | Literal[1]
    def __init__(self) -> None:
        self.x: int = 0
        self.y: Literal[1] = 1
    def reset(self) -> None:
        self.x: int = 1
        self.y: Literal[0] = 0
"#,
);

testcase!(
    test_nested_method_receiver_ownership,
    r#"
class Outer:
    def __init__(self) -> None:
        self.x: int = 0
        class Inner:
            @staticmethod
            def static_helper(self: str) -> None:
                self.x = ""  # E: Object of class `str` has no attribute `x`
            def second_param(other, self: str) -> None:
                self.x = ""  # E: Object of class `str` has no attribute `x`
            def captured(other) -> None:
                self.x = 1
        def local_capture() -> None:
            nonlocal self
            self.x = 2
"#,
);

testcase!(
    test_receiver_ownership_global_comprehension_and_nonlocal_method,
    r#"
from typing import Final
class Other:
    x: int
    y: int
self = Other()
class A:
    z: Final[int]
    def __init__(self) -> None:
        self.z = 0
        def uses_global() -> None:
            global self
            self.x: int = 1  # E: Cannot annotate non-self attribute `self.x`
        _ = [0 for self in [Other()] for self.y in [1]]
        class Inner:
            def method(other) -> None:
                nonlocal self
                self.z = 2  # E: Cannot set field `z`
_ = A().y  # E: Object of class `A` has no attribute `y`
"#,
);

testcase!(
    test_final_attribute_nested_permissions,
    r#"
from typing import Final
class A:
    x: Final[int]
    def __init__(self) -> None:
        self.x = 0
        def helper() -> None:
            self.x = 1
        def helper_nonlocal() -> None:
            nonlocal self
            self.x = 2
        class Inner:
            def method(inner_self) -> None:
                def f() -> None:
                    self.x = 4  # E: Cannot set field `x`
                f()
            @staticmethod
            def static_fn(self: A) -> None:
                self.x = 3  # E: Cannot set field `x`
"#,
);

testcase!(
    test_no_type_check_staticmethod_skipped_body,
    r#"
from typing import no_type_check
class A:
    @no_type_check
    @staticmethod
    def static_func(self) -> None:
        self.phantom = 1
    @no_type_check
    def normal_func(self) -> None:
        self.real = 2

a = A()
_ = a.phantom  # E: Object of class `A` has no attribute `phantom`
_ = a.real
"#,
);

testcase!(
    test_explicit_staticmethod_new_receiver,
    r#"
class C:
    @staticmethod
    def __new__(cls) -> "C":
        cls.x: int = 0
        return super().__new__(cls)

_ = C().x
"#,
);

testcase!(
    test_explicit_staticmethod_new_no_type_check,
    r#"
from typing import no_type_check

class C:
    @no_type_check
    @staticmethod
    def __new__(cls) -> "C":
        cls.x = 1
        return super().__new__(cls)

_ = C().x
"#,
);

testcase!(
    test_explicit_staticmethod_new_batch_skipped,
    TestEnv::new_skip_check_no_infer().with_run_require(Require::Errors),
    r#"
class C:
    @staticmethod
    def __new__(cls):
        cls.x = 1
        return super().__new__(cls)

_ = C().x
"#,
);

testcase!(
    test_attribute_initialization_unreachable_loop_exit,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
class LoopReturns:
    def __init__(self) -> None:
        for _ in [1]:
            self.value = 1
            return

class LoopReturnsInBranch:
    def __init__(self, flag: bool) -> None:
        if flag:
            for _ in [1]:
                self.value = 1
                return
        else:
            self.value = 2
"#,
);

testcase!(
    test_attribute_initialization_combines_every_return,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import NoReturn

def stop() -> NoReturn: ...

class LaterReturnMisses:
    def __init__(self, a: bool, b: bool) -> None:
        if a:
            self.value = 1  # E: Attribute `value` may be uninitialized
            return
        if b:
            return
        self.value = 2

class EveryReturnCutOff:
    def __init__(self, a: bool, b: bool) -> None:
        if a:
            if b:
                self.value = 1
            else:
                stop()
            return
        if b:
            self.value = 2
        else:
            stop()
"#,
);

testcase!(
    test_attribute_initialization_never_call_before_return,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
from typing import NoReturn

def stop() -> NoReturn: ...
def proceed() -> None: ...

class StopsBeforeReturn:
    def __init__(self, a: bool, b: bool) -> None:
        if a:
            stop()
            return  # E: This code is unreachable
        if b:
            self.value = 1
            return
        self.value = 2

class StopsInEveryBranchBeforeReturn:
    def __init__(self, a: bool, b: bool) -> None:
        if a:
            if b:
                stop()
            else:
                stop()
            return  # E: This code is unreachable
        self.value = 1

class ProceedsBeforeReturn:
    def __init__(self, a: bool) -> None:
        if a:
            proceed()
            return
        self.value = 1  # E: Attribute `value` may be uninitialized

class DeletedBeforeStop:
    def __init__(self, a: bool) -> None:
        self.value = 1
        if a:
            del self.value
            stop()
            return  # E: This code is unreachable

class DeferredBeforeStop:
    def __init__(self, a: bool, b: bool) -> None:
        if a:
            if b:
                proceed()
            else:
                self.value = 1
            stop()
            return  # E: This code is unreachable
        self.value = 2

class ReturnInFinallyAfterStop:
    def __init__(self, a: bool) -> None:
        if a:
            try:
                stop()
            finally:
                return
        self.value = 1  # E: Attribute `value` may be uninitialized

class LaterReturnMisses:
    def __init__(self, a: bool, b: bool) -> None:
        if a:
            stop()
            return  # E: This code is unreachable
        if b:
            return
        self.value = 1  # E: Attribute `value` may be uninitialized
"#,
);

testcase!(
    test_attribute_initialization_inherited_getattr_fallback,
    TestEnv::new().enable_possibly_uninitialized_attribute_error(),
    r#"
def coin() -> bool: ...

class Fallback:
    def __getattr__(self, name: str) -> int: ...

class Derived(Fallback):
    def __init__(self) -> None:
        if coin():
            self.value = 1

class NoFallback:
    def __init__(self) -> None:
        if coin():
            self.value = 1  # E: Attribute `value` may be uninitialized
"#,
);
