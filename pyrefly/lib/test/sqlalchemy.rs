/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::env;

use crate::test::util::TestEnv;
use crate::testcase;

fn sqlalchemy_env() -> TestEnv {
    let path = env::var("SQLALCHEMY_TEST_PATH").expect("SQLALCHEMY_TEST_PATH must be set");
    TestEnv::new_with_site_package_paths(&[&path])
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

testcase!(
    test_sqlalchemy_mapped_is_always_descriptor,
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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

testcase!(
    test_sqlalchemy_update_values_checks_sqlmodel_fields,
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
    sqlalchemy_env(),
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
