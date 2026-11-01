/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::util::TestEnv;
use crate::testcase;

fn sqlalchemy_env() -> TestEnv {
    let mut env = TestEnv::new();
    env.add_with_path(
        "sqlalchemy",
        "sqlalchemy/__init__.py",
        "from . import orm\n",
    );
    env.add_with_path(
        "sqlalchemy.sql",
        "sqlalchemy/sql/__init__.py",
        "from . import sqltypes\nfrom . import type_api\n",
    );
    env.add_with_path(
        "sqlalchemy.sql.type_api",
        "sqlalchemy/sql/type_api.py",
        r#"
from typing import Generic, TypeVar

_T = TypeVar("_T")

class TypeEngine(Generic[_T]):
    ...

class TypeDecorator(TypeEngine[_T], Generic[_T]):
    ...
"#,
    );
    env.add_with_path(
        "sqlalchemy.sql.sqltypes",
        "sqlalchemy/sql/sqltypes.py",
        r#"
from enum import Enum as PythonEnum
from typing import Generic, TypeVar
from .type_api import TypeEngine

_T = TypeVar("_T")

class String(TypeEngine[str]):
    ...

class Integer(TypeEngine[int]):
    ...

class Float(TypeEngine[float]):
    ...

class GenericType(TypeEngine[_T], Generic[_T]):
    ...

# SQLAlchemy itself inherits these conflicting TypeEngine arguments.
class Enum(String, TypeEngine[str | PythonEnum]):  # E: Class `Enum` has inconsistent type arguments
    def __init__(self, *enums: str | type[PythonEnum]) -> None:
        ...
"#,
    );
    env.add_with_path(
        "sqlalchemy.orm.attributes",
        "sqlalchemy/orm/attributes.py",
        r#"
class InstrumentedAttribute[T]: ...
"#,
    );
    env.add_with_path(
        "sqlalchemy.orm.base",
        "sqlalchemy/orm/base.py",
        r#"
from typing import Any, Generic, TypeVar, overload
from .attributes import InstrumentedAttribute

_T = TypeVar("_T")

class _MappedAnnotationBase(Generic[_T]): ...

class Mapped(_MappedAnnotationBase[_T]):
    @overload
    def __get__(self, instance: None, owner: Any) -> InstrumentedAttribute[_T]: ...
    @overload
    def __get__(self, instance: object, owner: Any) -> _T: ...
    def __get__(self, instance: object | None, owner: Any) -> InstrumentedAttribute[_T] | _T: ...
    def __set__(self, instance: object, value: _T) -> None: ...

class _DeclarativeMapped(Mapped[_T]): ...
"#,
    );
    env.add_with_path(
        "sqlalchemy.orm.properties",
        "sqlalchemy/orm/properties.py",
        r#"
from typing import Generic, TypeVar
from .base import _DeclarativeMapped

_T = TypeVar("_T")

class MappedColumn(_DeclarativeMapped[_T]):
    column: object

    def __init__(self) -> None:
        ...
"#,
    );
    env.add_with_path(
        "sqlalchemy.orm._orm_constructors",
        "sqlalchemy/orm/_orm_constructors.py",
        r#"
from typing import Any
from sqlalchemy.sql.type_api import TypeEngine
from .properties import MappedColumn

def mapped_column(
    __name_pos: str | TypeEngine[Any] | type[TypeEngine[Any]] | None = None,
    __type_pos: TypeEngine[Any] | type[TypeEngine[Any]] | None = None,
    *args: Any,
    type_: TypeEngine[Any] | type[TypeEngine[Any]] | None = None,
    nullable: bool | None = None,
    primary_key: bool | None = False,
    **kw: Any,
) -> MappedColumn[Any]:
    return MappedColumn()
"#,
    );
    env.add_with_path(
        "sqlalchemy.orm",
        "sqlalchemy/orm/__init__.py",
        r#"
from ._orm_constructors import mapped_column as mapped_column
from .attributes import InstrumentedAttribute as InstrumentedAttribute
from .base import Mapped as Mapped
from .properties import MappedColumn as MappedColumn
"#,
    );
    env
}

testcase!(
    test_sqlalchemy_mapped_column_infers_type,
    sqlalchemy_env(),
    r#"
from typing import assert_type
from sqlalchemy.orm import InstrumentedAttribute, MappedColumn, mapped_column
from sqlalchemy.sql.sqltypes import Integer, String

class Model:
    name = mapped_column(String())
    quantity = mapped_column(Integer)
    sku = mapped_column(String(), nullable=False)
    pk = mapped_column(Integer, primary_key=True)
    nullable_pk = mapped_column(Integer, primary_key=True, nullable=True)

assert_type(Model.name, InstrumentedAttribute[str | None])
assert_type(Model.quantity, InstrumentedAttribute[int | None])
assert_type(Model.sku, InstrumentedAttribute[str])
assert_type(Model.pk, InstrumentedAttribute[int])
assert_type(Model.nullable_pk, InstrumentedAttribute[int | None])
assert_type(Model().name, str | None)
assert_type(Model().quantity, int | None)
assert_type(Model().sku, str)
assert_type(Model().pk, int)
assert_type(Model().nullable_pk, int | None)

assert_type(mapped_column("sku", String(), nullable=False), MappedColumn[str])
assert_type(mapped_column(type_=String(), nullable=False), MappedColumn[str])

def make_column() -> MappedColumn[str | None]:
    return mapped_column(String())
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_substitutes_generic_types,
    sqlalchemy_env(),
    r#"
from typing import assert_type, reveal_type
from sqlalchemy.orm import MappedColumn, mapped_column
from sqlalchemy.sql.sqltypes import GenericType
from sqlalchemy.sql.type_api import TypeDecorator

assert_type(mapped_column(GenericType[float]()), MappedColumn[float | None])
assert_type(mapped_column(TypeDecorator[str]()), MappedColumn[str | None])
reveal_type(mapped_column(GenericType))  # E: revealed type: MappedColumn[Unknown | None]
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_infers_enum_class,
    sqlalchemy_env(),
    r#"
from enum import Enum as PythonEnum
from typing import assert_type
from sqlalchemy.orm import MappedColumn, mapped_column
from sqlalchemy.sql.sqltypes import Enum

class Color(PythonEnum):
    RED = "red"

assert_type(mapped_column(Enum(Color)), MappedColumn[Color | None])
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_respects_explicit_annotation,
    sqlalchemy_env(),
    r#"
from typing import Literal, assert_type
from sqlalchemy.orm import InstrumentedAttribute, Mapped, mapped_column
from sqlalchemy.sql.sqltypes import Float, Integer, String

Key = Literal["api_key", "api_secret"]

class Model:
    key: Mapped[Key] = mapped_column(String(), nullable=False)
    quantity: Mapped[int] = mapped_column(Integer, primary_key=True)
    ratio: Mapped[float] = mapped_column(Float(), nullable=False)
    maybe_ratio: Mapped[float | None] = mapped_column(Float())

assert_type(Model.key, InstrumentedAttribute[Key])
assert_type(Model.quantity, InstrumentedAttribute[int])
assert_type(Model.ratio, InstrumentedAttribute[float])
assert_type(Model.maybe_ratio, InstrumentedAttribute[float | None])
assert_type(Model().key, Key)
assert_type(Model().quantity, int)
assert_type(Model().ratio, float)
assert_type(Model().maybe_ratio, float | None)
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_stored_enum,
    sqlalchemy_env(),
    r#"
from enum import Enum as PythonEnum
from typing import Any, assert_type
from sqlalchemy.orm import mapped_column
from sqlalchemy.sql.sqltypes import Enum

class Color(PythonEnum):
    RED = "red"

color_type = Enum(Color)
class Model:
    color = mapped_column(color_type, nullable=False)

assert_type(Model().color, Any)
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_nullable_flags,
    sqlalchemy_env(),
    r#"
from typing import Any, Literal, assert_type
from sqlalchemy.orm import MappedColumn, mapped_column
from sqlalchemy.sql.sqltypes import String

TRUE: Literal[True] = True
FALSE: Literal[False] = False
assert_type(mapped_column(String, primary_key=True, nullable=TRUE), MappedColumn[str | None])
assert_type(mapped_column(String, nullable=FALSE), MappedColumn[str])
assert_type(mapped_column(String, primary_key=TRUE), MappedColumn[str])
assert_type(mapped_column(String, primary_key=True, **{"nullable": True}), MappedColumn[Any])

def make(nullable: bool, primary_key: bool) -> None:
    assert_type(mapped_column(String, primary_key=True, nullable=nullable), MappedColumn[str | None])
    assert_type(mapped_column(String, primary_key=primary_key), MappedColumn[str | None])
    assert_type(mapped_column(String, nullable=False, primary_key=primary_key), MappedColumn[str])
    assert_type(mapped_column(String, primary_key=True, nullable=None), MappedColumn[str | None])
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_gradual_union,
    sqlalchemy_env(),
    r#"
from typing import Any, assert_type
from sqlalchemy.orm import MappedColumn, mapped_column
from sqlalchemy.sql.sqltypes import Integer, String

def make(type_: Integer | Any) -> None:
    assert_type(mapped_column(type_, nullable=False), MappedColumn[Any])

def known(type_: Integer | String) -> None:
    assert_type(mapped_column(type_, nullable=False), MappedColumn[int | str])
"#,
);

testcase!(
    test_sqlalchemy_mapped_column_call_boundaries,
    sqlalchemy_env(),
    r#"
from enum import Enum as PythonEnum
from typing import Annotated, Any, Literal, TypeAlias, assert_type
from sqlalchemy.orm import Mapped, MappedColumn, mapped_column
from sqlalchemy.sql.sqltypes import Enum, String
from sqlalchemy.sql.type_api import TypeEngine

class Color(PythonEnum):
    RED = "red"

class DialectEnum(Enum): ...

constructor = Enum
assert_type(mapped_column("color", constructor(Color)), MappedColumn[Color | None])
assert_type(mapped_column(type_=constructor(Color)), MappedColumn[Color | None])
assert_type(mapped_column(DialectEnum(Color)), MappedColumn[Any])
assert_type(mapped_column(Enum("red", "blue")), MappedColumn[Any])

def enum_factory(ignored: type[Color]) -> Enum:
    return Enum("red", "blue")

assert_type(mapped_column(enum_factory(Color)), MappedColumn[Any])

def abstract(sql_type: TypeEngine[str]) -> None:
    assert_type(mapped_column(sql_type, nullable=False), MappedColumn[Any])

def class_form(sql_type: type[String]) -> None:
    assert_type(mapped_column(sql_type, nullable=False), MappedColumn[str])

StringAlias: TypeAlias = String
assert_type(mapped_column(StringAlias, nullable=False), MappedColumn[str])

type KeyColumn = Mapped[Literal["red"]]
class Model:
    key: KeyColumn = mapped_column(String, nullable=False)
    annotated: Annotated[Mapped[Literal["red"]], "metadata"] = mapped_column(String)
assert_type(Model().key, Literal["red"])
assert_type(Model().annotated, Literal["red"])

mapped_column(42)  # E: Argument
"#,
);

testcase!(
    test_mapped_column_without_sqlalchemy,
    r#"
from typing import assert_type

def mapped_column(value: int) -> bytes:
    return bytes(value)

assert_type(mapped_column(3), bytes)
"#,
);
