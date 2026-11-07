/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::pydantic::util::pydantic_env;
use crate::test::util::TestEnv;
use crate::testcase;

testcase!(
    test_uninitialized_instance_variable,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
class C:
    x: int  # E: Instance attribute `x` is declared but never initialized
"#,
);

// Cases that must not fire: initialized in `__init__`, given a class-body value,
// `ClassVar`, `Protocol` members, `NamedTuple` fields, and ordinary dataclass fields
// (whose synthesized `__init__` initializes them).
testcase!(
    test_uninitialized_instance_variable_no_error,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from typing import ClassVar, NamedTuple, Protocol
from dataclasses import dataclass
class InInit:
    a: int
    def __init__(self) -> None:
        self.a = 1
class ClassBodyValue:
    b: int = 0
class HasClassVar:
    c: ClassVar[int]
class Proto(Protocol):
    d: int
class NT(NamedTuple):
    e: int
@dataclass
class DC:
    f: int
"#,
);

testcase!(
    test_uninitialized_instance_variable_pydantic,
    pydantic_env().enable_uninitialized_instance_variable_error(),
    r#"
from pydantic import BaseModel
class M(BaseModel):
    x: int
"#,
);

// A `Final` field with no initializer is reported by the dedicated `Final` check, not by
// this one; asserting a single error confirms we do not emit a duplicate diagnostic.
testcase!(
    test_uninitialized_instance_variable_final,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from typing import Final
class C:
    x: Final[int]  # E: Final attribute declared in class body must be initialized with a value or in `__init__`
"#,
);

testcase!(
    test_uninitialized_instance_variable_disabled_by_default,
    r#"
class C:
    x: int
"#,
);

testcase!(
    test_uninitialized_instance_variable_dataclass_init_false,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from dataclasses import dataclass
@dataclass(init=False)
class C:
    x: int  # E: Instance attribute `x` is declared but never initialized
"#,
);

testcase!(
    test_uninitialized_instance_variable_dataclass_default_init,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from dataclasses import dataclass
@dataclass
class C:
    x: int
"#,
);

testcase!(
    test_uninitialized_instance_variable_dataclass_init_false_class_body_value,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from dataclasses import dataclass
@dataclass(init=False)
class C:
    x: int = 0
"#,
);

// https://github.com/facebook/pyrefly/issues/5280
testcase!(
    test_uninitialized_instance_variable_set_in_new,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from typing import Self
class Label(str):
    flag: bool
    def __new__(cls, text: str, flag: bool) -> Self:
        result = super().__new__(cls, text)
        result.flag = flag
        return result
class Partial:
    x: int
    y: int  # E: Instance attribute `y` is declared but never initialized
    def __new__(cls) -> Self:
        result = super().__new__(cls)
        result.x = 1
        return result
"#,
);

testcase!(
    test_uninitialized_instance_variable_not_set_in_new,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from typing import Self
class Other:
    x: int = 0
class NeverSet:
    x: int  # E: Instance attribute `x` is declared but never initialized
    def __new__(cls) -> Self:
        return super().__new__(cls)
class SetOnOtherObject:
    x: int  # E: Instance attribute `x` is declared but never initialized
    def __new__(cls) -> Self:
        obj = Other()
        obj.x = 1
        return super().__new__(cls)
"#,
);

// https://github.com/facebook/pyrefly/issues/5554
testcase!(
    test_uninitialized_instance_variable_argparse_namespace,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from argparse import Namespace

class Args(Namespace):
    name: str

class SubArgs(Args):
    verbose: bool
"#,
);

// https://github.com/facebook/pyrefly/issues/5278
// Metaclass annotations describe attributes supplied by classes that use the
// metaclass (via type.__new__), not fields that must be set in metaclass __init__.
testcase!(
    test_uninitialized_instance_variable_metaclass_attrs,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
class Metadata(type):
    label: str

class Example(metaclass=Metadata):
    label = "example"

def read_label(cls: Metadata) -> str:
    return cls.label
"#,
);

// https://github.com/facebook/pyrefly/issues/5277
testcase!(
    test_uninitialized_instance_variable_enum_value,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from enum import Enum

class Format(Enum):
    _value_: str
    _name_: str
    description: str  # E: Instance attribute `description` is declared but never initialized
    TEXT = "text"

    def render(self) -> str:
        return self._value_

class NotAnEnum:
    _value_: str  # E: Instance attribute `_value_` is declared but never initialized
"#,
);

// https://github.com/facebook/pyrefly/issues/5326
testcase!(
    test_uninitialized_instance_variable_frozen_dataclass_setattr,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
import builtins
from dataclasses import dataclass
from pathlib import PurePosixPath
@dataclass(frozen=True, slots=True, init=False)
class File:
    inner: tuple[PurePosixPath, int]
    def __init__(self, path: str, /, *, size: int) -> None:
        object.__setattr__(self, "inner", (PurePosixPath(path), size))
@dataclass(init=False)
class ViaBuiltin:
    x: int
    _protected: int
    def __init__(self, x: int) -> None:
        setattr(self, "x", x)
        object.__setattr__(self, "_protected", 1)
@dataclass(init=False)
class ViaImportedBuiltins:
    x: int
    y: int
    def __init__(self) -> None:
        builtins.object.__setattr__(self, "x", 1)
        builtins.setattr(self, "y", 2)
@dataclass(frozen=True, init=False)
class StillUninit:
    y: int  # E: Instance attribute `y` is declared but never initialized
    def __init__(self) -> None:
        pass
@dataclass(frozen=True, init=False)
class StarredArg:
    x: int  # E: Instance attribute `x` is declared but never initialized
    def __init__(self) -> None:
        object.__setattr__(self, "x", *[1])
class PlainStarred:
    def __init__(self) -> None:
        object.__setattr__(self, "x", *[1])
        setattr(self, "x", *[1])
@dataclass(frozen=True, init=False)
class StarredName:
    x: int  # E: Instance attribute `x` is declared but never initialized
    def __init__(self) -> None:
        object.__setattr__(self, *["x", 1])
@dataclass(frozen=True, init=False)
class ClassmethodReceiver:
    x: int  # E: Instance attribute `x` is declared but never initialized
    @classmethod
    def init_class(cls) -> None:
        setattr(cls, "x", 1)
@dataclass(frozen=True, init=False)
class MangledPrivate:
    __x: int  # E: Instance attribute `__x` is declared but never initialized
    __y: int  # E: Instance attribute `__y` is declared but never initialized
    def __init__(self) -> None:
        object.__setattr__(self, "__x", 1)
        setattr(self, "__y", 1)
"#,
);

testcase!(
    test_uninitialized_instance_variable_setattr_shadowed,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from dataclasses import dataclass
from typing import Any

def setattr(obj: object, name: str, value: object) -> None:
    pass

@dataclass(frozen=True, init=False)
class ShadowedSetattr:
    x: int  # E: Instance attribute `x` is declared but never initialized
    def __init__(self) -> None:
        setattr(self, "x", 1)

object: Any = None

@dataclass(frozen=True, init=False)
class ShadowedObject:
    y: int  # E: Instance attribute `y` is declared but never initialized
    def __init__(self) -> None:
        object.__setattr__(self, "y", 1)
"#,
);

testcase!(
    test_setattr_checked_unannotated_constructor,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
class Checked:
    x: int
    y: int
    def __init__(self):
        setattr(self, "x", 1)
        object.__setattr__(self, "y", 2)
"#,
);

// Skipped method bodies only discover ordinary attribute assignment targets.
// Setter recognition would also need lexical shadowing information from those bodies.
testcase!(
    test_setattr_skipped_constructor_boundary,
    TestEnv::new().enable_uninitialized_instance_variable_error(),
    r#"
from typing import no_type_check

class Skipped:
    x: int  # E: Instance attribute `x` is declared but never initialized
    y: int
    @no_type_check
    def __init__(self) -> None:
        setattr(self, "x", 1)
        self.y = 2
"#,
);
