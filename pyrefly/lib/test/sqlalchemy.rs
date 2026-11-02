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

fn sqlalchemy_mapped_env() -> TestEnv {
    let mut env = TestEnv::new();
    env.add(
        "sqlalchemy.orm._orm_constructors",
        r#"
from typing import Any

def mapped_column(*args: Any, **kw: Any) -> Any: ...
    "#,
    );
    env.add(
        "sqlalchemy.orm.base",
        r#"
class Mapped[T]:
    def __get__(self, instance, owner) -> T: ...
    def __set__(self, instance, value: T) -> None: ...
    def __delete__(self, instance) -> None: ...
    "#,
    );
    env.add(
        "sqlalchemy.sql.elements",
        r#"
class ColumnElement[T]: ...
    "#,
    );
    env.add(
        "sqlalchemy.sql.dml",
        r#"
class Update:
    def where(self, *criteria: object) -> Update: ...
    def values(self, **kwargs: object) -> Update: ...
    "#,
    );
    env.add(
        "sqlalchemy.sql.selectable",
        r#"
class Select:
    def where(self, *criteria: object) -> Select: ...
    def filter_by(self, **kwargs: object) -> Select: ...
    def join(self, target: object) -> Select: ...
    "#,
    );
    env.add_with_path(
        "sqlalchemy.orm.decl_api",
        "sqlalchemy/orm/decl_api.py",
        r#"
from typing import dataclass_transform

from ._orm_constructors import mapped_column

class DeclarativeBase: ...

@dataclass_transform(field_specifiers=(mapped_column,))
class DCTransformDeclarative(type): ...

class MappedAsDataclass(metaclass=DCTransformDeclarative): ...
        "#,
    );
    env.add_with_path(
        "sqlalchemy.orm",
        "sqlalchemy/orm/__init__.py",
        r#"
from .base import Mapped as Mapped
from .decl_api import DeclarativeBase as DeclarativeBase
from .decl_api import MappedAsDataclass as MappedAsDataclass
from ._orm_constructors import mapped_column as mapped_column
    "#,
    );
    env.add_with_path(
        "sqlalchemy",
        "sqlalchemy/__init__.py",
        r#"
from .sql.dml import Update as Update
from .sql.elements import ColumnElement as ColumnElement
from .sql.selectable import Select as Select
def select(*entities: object) -> Select: ...
def update(table: object) -> Update: ...
    "#,
    );
    env
}

testcase!(
    test_sqlalchemy_mapped_is_always_descriptor,
    sqlalchemy_mapped_env(),
    r#"
from sqlalchemy.orm import DeclarativeBase, Mapped
class Base(DeclarativeBase):
    pass
class User(Base):
    name: Mapped[str]
    def __init__(self, name: str):
        self.name = name
    "#,
);

// Regression test for https://github.com/facebook/pyrefly/issues/1610
testcase!(
    test_sqlalchemy_mapped_dataclass_mutable_default,
    sqlalchemy_mapped_env(),
    r#"
from sqlalchemy.orm import DeclarativeBase, Mapped, MappedAsDataclass, mapped_column

class Base(MappedAsDataclass, DeclarativeBase):
    pass

class Model(Base):
    client_params: Mapped[dict] = mapped_column(default={})  # E: Mutable default for field `client_params` is not allowed; use `default_factory`
    labels: Mapped[list[str]] = mapped_column(default_factory=list)
    "#,
);

testcase!(
    test_sqlalchemy_update_values_checks_mapped_fields,
    sqlalchemy_mapped_env(),
    r#"
import sqlalchemy as sa
from sqlalchemy.orm import DeclarativeBase, Mapped

class Base(DeclarativeBase):
    pass

class User(Base):
    id: Mapped[int]
    name: Mapped[str]

sa.update(User).where(User.id == 1).values(name="alice", id=1)
sa.update(User).where(User.id == 1).values(name=0)  # E: `Literal[0]` is not assignable to field `name` with type `str`
sa.update(User).where(User.id == 1).values(nam="alice")  # E: Unexpected SQLAlchemy update field `nam`

# SQLAlchemy accepts a SQL expression wherever a column value is expected.
def sql_expr() -> sa.ColumnElement[int]: ...
sa.update(User).values(name=sql_expr())

class CustomUpdate:
    def values(self, **kwargs: object) -> CustomUpdate: ...

def update(table: object) -> CustomUpdate: ...

# A same-named function outside SQLAlchemy must not trigger the special-case check.
update(User).values(nam="alice")
    "#,
);

testcase!(
    test_sqlalchemy_update_values_checks_inherited_mapped_fields,
    sqlalchemy_mapped_env(),
    r#"
import sqlalchemy as sa
from sqlalchemy.orm import DeclarativeBase, Mapped

class Base(DeclarativeBase):
    pass

class SoftDeleteMixin:
    deleted: Mapped[bool]

class User(Base, SoftDeleteMixin):
    id: Mapped[int]
    name: Mapped[str]

class AdminUser(User):
    role: Mapped[str]
    deleted: bool  # type: ignore

sa.update(AdminUser).where(AdminUser.id == 1).values(name="alice")
sa.update(User).where(User.id == 1).values(name="alice", deleted=False)
sa.update(AdminUser).where(AdminUser.id == 1).values(deleted=False)  # E: Unexpected SQLAlchemy update field `deleted`
    "#,
);

// Regression test for https://github.com/facebook/pyrefly/issues/1072
testcase!(
    test_sqlalchemy_select_filter_by_checks_mapped_fields,
    sqlalchemy_mapped_env(),
    r#"
import sqlalchemy as sa
from sqlalchemy import select
from sqlalchemy.orm import DeclarativeBase, Mapped

class Base(DeclarativeBase):
    pass

class User(Base):
    id: Mapped[int]
    name: Mapped[str]

select(User).filter_by(name="alice", id=1)
sa.select(User).where(User.id == 1).filter_by(name=0)  # E: `Literal[0]` is not assignable to field `name` with type `str`
select(User).filter_by(nam="alice")  # E: Unexpected SQLAlchemy filter_by field `nam`
select(User).filter_by(id=1).filter_by(name="alice")

# SQLAlchemy accepts a SQL expression wherever a column value is expected.
def sql_expr() -> sa.ColumnElement[int]: ...
select(User).filter_by(name=sql_expr())

class Address(Base):
    email: Mapped[str]

# A join changes the namespace that filter_by uses, so defer to SQLAlchemy's typing.
select(User).join(Address).filter_by(email="alice@example.com")

class CustomSelect:
    def filter_by(self, **kwargs: object) -> CustomSelect: ...

def select_custom(entity: object) -> CustomSelect: ...

# A same-named method outside SQLAlchemy must not trigger the special-case check.
select_custom(User).filter_by(nam="alice")
    "#,
);

fn sqlmodel_env() -> TestEnv {
    let mut env = sqlalchemy_mapped_env();
    env.add_with_path(
        "sqlmodel",
        "sqlmodel/__init__.py",
        r#"
from typing import Any

class SQLModel:
    ...

def Field(*args, **kwargs) -> Any:
    ...

def Relationship(*args, **kwargs) -> Any:
    ...
"#,
    );
    env
}

testcase!(
    test_sqlalchemy_update_values_checks_sqlmodel_fields,
    sqlmodel_env(),
    r#"
from uuid import UUID

from sqlalchemy import update
from sqlalchemy.orm import Mapped
from sqlmodel import Field, Relationship, SQLModel

class Node(SQLModel, table=True):
    id: UUID = Field(primary_key=True)
    name: str = Field(description="x")
    parent_id: UUID | None = Field(default=None, foreign_key="node.id")
    parent: Mapped["Node | None"] = Relationship()

update(Node).values(name="a")
update(Node).values(parent_id=None)
update(Node).values(nope="a")  # E: Unexpected SQLAlchemy update field `nope`
    "#,
);

testcase!(
    test_sqlalchemy_update_values_checks_sqlmodel_fields_no_mapped,
    sqlmodel_env(),
    r#"
from sqlalchemy import update
from sqlmodel import Field, SQLModel

class HeroBase(SQLModel):
    name: str = Field(index=True)
    secret_name: str
    age: int | None = Field(default=None, index=True)

class Hero(HeroBase, table=True):
    id: int | None = Field(default=None, primary_key=True)

update(Hero).values(name="a")
update(Hero).values(nope="a")  # E: Unexpected SQLAlchemy update field `nope`
    "#,
);

testcase!(
    test_sqlalchemy_update_values_checks_sqlmodel_fields_without_mapped,
    sqlmodel_env(),
    r#"
from uuid import UUID

from sqlalchemy import update
from sqlmodel import Field, SQLModel

class Node(SQLModel, table=True):
    id: UUID = Field(primary_key=True)
    name: str = Field(description="x")

update(Node).values(name="a")
update(Node).values(nope="a")  # E: Unexpected SQLAlchemy update field `nope`
    "#,
);

testcase!(
    test_sqlalchemy_update_values_checks_sqlmodel_fields_inherited_fields,
    sqlmodel_env(),
    r#"
from sqlalchemy import update
from sqlmodel import Field, SQLModel

class HeroBase(SQLModel):
    name: str = Field(index=True)
    secret_name: str
    age: int | None = Field(default=None, index=True)

class Hero(HeroBase, table=True):
    id: int | None = Field(default=None, primary_key=True)

update(Hero).values(secret_name="a")
update(Hero).values(nope="a")  # E: Unexpected SQLAlchemy update field `nope`
    "#,
);

testcase!(
    test_sqlalchemy_update_values_checks_sqlmodel_ignores_fields_past_table,
    sqlmodel_env(),
    r#"
from sqlalchemy import update
from sqlmodel import Field, SQLModel

class HeroBase(SQLModel):
    name: str = Field(index=True)
    secret_name: str
    age: int | None = Field(default=None, index=True)

class Hero(HeroBase, table=True):
    id: int | None = Field(default=None, primary_key=True)

class EvilHero(Hero):
    evil_amount: bool

update(EvilHero).values(secret_name="a")
update(EvilHero).values(evil_amount="a")  # E: Unexpected SQLAlchemy update field `evil_amount`
    "#,
);
