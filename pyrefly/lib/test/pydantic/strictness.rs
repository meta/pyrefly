/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::pydantic_testcase;

pydantic_testcase!(
    test_pyrefly_strict,
    r#"
from pydantic import BaseModel, Field
class Model(BaseModel):
    x: int = Field(strict=False)  # this is the default
    y: int = Field(strict=True)
Model(x='0', y=1) 
Model(x='0', y='1') # E: Argument `Literal['1']` is not assignable to parameter `y` with type `int` in function `Model.__init__` 
"#,
);

// Note: mypy does not support strict=false. Everything is strict.
pydantic_testcase!(
    test_pyrefly_strict_default,
    r#"
from pydantic import BaseModel, Field
class Model(BaseModel):
    x: int = Field()
    y: int = Field(strict=True)
Model(x='0', y=1) 
Model(x='0', y='1') # E: Argument `Literal['1']` is not assignable to parameter `y` with type `int` in function `Model.__init__` 
"#,
);

pydantic_testcase!(
    test_strict_annotated_types,
    r#"
from typing import Annotated
from pydantic import BaseModel, Field, StrictInt

class Model(BaseModel):
    x: StrictInt = 1
    z: Annotated[StrictInt, Field(description="d")] = 1

Model(x=1, z=1)
Model(x='1', z=1)  # E: Argument `Literal['1']` is not assignable to parameter `x`
Model(x=1, z='1')  # E: Argument `Literal['1']` is not assignable to parameter `z`
    "#,
);

pydantic_testcase!(
    test_strict_annotated_false,
    r#"
from typing import Annotated
from pydantic import BaseModel, Strict

class Model(BaseModel):
    x: Annotated[int, Strict(False)]

Model(x="1")
    "#,
);

pydantic_testcase!(
    test_strict_annotated_does_not_merge_other_field_metadata,
    r#"
from typing import Annotated
from pydantic import BaseModel, Field

class Model(BaseModel):
    x: Annotated[int, Field(gt=10)] = 5
    y: Annotated[int, Field(gt=10)] = Field(default=5)

Model(x=5, y=5)
    "#,
);

pydantic_testcase!(
    test_class_keyword,
    r#"
from pydantic import BaseModel, Field

class Model1(BaseModel, strict=True):
    x: int = Field(0)
    y: int = Field(0, strict=False)
# `x` is strict
Model1(x=0)
Model1(x='0')  # E: `Literal['0']` is not assignable to parameter `x`
# `y` is lax
Model1(y=0)
Model1(y='0')

class Model2(BaseModel, strict=False):
    x: int = Field(0)
    y: int = Field(0, strict=True)
# `x` is lax
Model2(x=0)
Model2(x='0')
# `y` is strict
Model2(y=0)
Model2(y='0')  # E: `Literal['0']` is not assignable to parameter `y`
    "#,
);

pydantic_testcase!(
    test_configdict,
    r#"
from pydantic import BaseModel, ConfigDict
class Model(BaseModel):
    x: int
    model_config = ConfigDict(strict=True)
Model(x=0)
Model(x='0')  # E: `Literal['0']` is not assignable to parameter `x`
    "#,
);

pydantic_testcase!(
    test_multiple_strict_values,
    r#"
from pydantic import BaseModel, ConfigDict

# When `strict` appears in both the class keywords and model_config, the keyword wins
class Model1(BaseModel, strict=True):
    x: int
    model_config = ConfigDict(strict=False)
Model1(x=0)
Model1(x='0')  # E: `Literal['0']` is not assignable to parameter `x`
class Model2(BaseModel, strict=False):
    x: int
    model_config = ConfigDict(strict=True)
Model2(x=0)
Model2(x='0')
    "#,
);

pydantic_testcase!(
    test_inherit,
    r#"
from pydantic import BaseModel
class Model1(BaseModel, strict=True):
    pass
class Model2(Model1):
    x: int
Model2(x=0)
Model2(x='0')  # E: `Literal['0']` is not assignable to parameter `x`
    "#,
);

pydantic_testcase!(
    test_lax_mode_coercion,
    r#"
from pydantic import BaseModel
from typing import Callable, reveal_type
from decimal import Decimal
from datetime import date, datetime, time, timedelta
from pathlib import Path
from uuid import UUID

class Model(BaseModel):
    x: int = 0

reveal_type(Model.__init__)  # E: revealed type: (self: Model, *, x: LaxInt = ..., **Unknown) -> None

# int field accepts: int, bool, float, str, bytes, Decimal
Model(x=1)
Model(x=True)
Model(x=1.0)
Model(x='123')
Model(x=b'123')
Model(x=Decimal('123'))

class Model2(BaseModel):
    x: bytes

reveal_type(Model2.__init__)  # E: revealed type: (self: Model2, *, x: LaxBytes, **Unknown) -> None

class Model3(BaseModel):
    func: Callable[[int], str]

reveal_type(Model3.__init__)  # E: revealed type: (self: Model3, *, func: Any, **Unknown) -> None

class Model4(BaseModel):
    d: date

reveal_type(Model4.__init__)  # E: revealed type: (self: Model4, *, d: LaxDate, **Unknown) -> None

class Model5(BaseModel):
    dt: datetime

reveal_type(Model5.__init__)  # E: revealed type: (self: Model5, *, dt: LaxDatetime, **Unknown) -> None

class Model6(BaseModel):
    t: time

reveal_type(Model6.__init__)  # E: revealed type: (self: Model6, *, t: LaxTime, **Unknown) -> None

class Model7(BaseModel):
    td: timedelta

reveal_type(Model7.__init__)  # E: revealed type: (self: Model7, *, td: LaxTimedelta, **Unknown) -> None

class Model8(BaseModel):
    dec: Decimal

reveal_type(Model8.__init__)  # E: revealed type: (self: Model8, *, dec: LaxDecimal, **Unknown) -> None

class Model9(BaseModel):
    p: Path

reveal_type(Model9.__init__)  # E: revealed type: (self: Model9, *, p: LaxPath, **Unknown) -> None

class Model10(BaseModel):
    u: UUID

reveal_type(Model10.__init__)  # E: revealed type: (self: Model10, *, u: LaxUUID, **Unknown) -> None
    "#,
);

pydantic_testcase!(
    test_lax_mode_coercion_literals,
    r#"
from typing import Literal
from pydantic import BaseModel

class Model1(BaseModel):
    status: Literal[1]

m = Model1(status="1")  # E: Argument `Literal['1']` is not assignable to parameter `status`

class Model2(BaseModel):
    value: Literal["MyLiteral"]

Model2(value=2)  # E: Argument `Literal[2]` is not assignable to parameter `value`
    "#,
);

pydantic_testcase!(
    test_lax_mode_coercion_container,
    r#"
from typing import List, Sequence, Iterable, reveal_type
from collections import deque

from pydantic import BaseModel

class Model(BaseModel):
    x: List[int] = [0, 1]

reveal_type(Model.__init__) # E: revealed type: (self: Model, *, x: Iterable[LaxInt] = ..., **Unknown) -> None

class Model2(BaseModel):
    q: deque[int]

reveal_type(Model2.__init__) # E: revealed type: (self: Model2, *, q: Iterable[LaxInt], **Unknown) -> None

class Model3(BaseModel):
    d: dict[str, int]

reveal_type(Model3.__init__) # E: revealed type: (self: Model3, *, d: Mapping[str, LaxInt], **Unknown) -> None

class Model4(BaseModel):
    f: frozenset[int]

reveal_type(Model4.__init__) # E: revealed type: (self: Model4, *, f: Iterable[LaxInt], **Unknown) -> None

class Model5(BaseModel):
    s: set[int]

reveal_type(Model5.__init__) # E: revealed type: (self: Model5, *, s: Iterable[LaxInt], **Unknown) -> None

class Model6(BaseModel):
    t: Iterable[int]

reveal_type(Model6.__init__) # E: revealed type: (self: Model6, *, t: Iterable[LaxInt], **Unknown) -> None

class Model7(BaseModel):
    seq: Sequence[int]

reveal_type(Model7.__init__) # E: revealed type: (self: Model7, *, seq: Iterable[LaxInt], **Unknown) -> None

class Model8(BaseModel):
    t: tuple[int, ...]

reveal_type(Model8.__init__) # E: revealed type: (self: Model8, *, t: Iterable[LaxInt], **Unknown) -> None

class Model9(BaseModel):
    fixed: tuple[int, str, bool]

reveal_type(Model9.__init__) # E: revealed type: (self: Model9, *, fixed: Iterable[Decimal | bool | bytearray | bytes | float | int | str], **Unknown) -> None
    "#,
);

pydantic_testcase!(
    test_lax_mode_coercion_union,
    r#"
from typing import List, reveal_type
from decimal import Decimal

from pydantic import BaseModel

class Model(BaseModel):
    y: int | bool

reveal_type(Model.__init__)  # E: revealed type: (self: Model, *, y: Decimal | bool | bytes | float | int | str, **Unknown) -> None
    "#,
);

pydantic_testcase!(
    test_lax_mode_coercion_union_rejects_invalid_values,
    r#"
from typing import Literal
from pydantic import BaseModel

class Something(BaseModel):
    pass

class OtherThing(BaseModel):
    pass

class Model(BaseModel):
    scalar: str | bool
    model: Something | OtherThing

Model(scalar=5, model=Something())  # E: Argument value `Literal[5]` is not valid for Pydantic field `scalar` with type `bool | str`
Model(scalar="value", model=5)  # E: Argument `Literal[5]` is not assignable to parameter `model`

class BoolModel(BaseModel):
    value: bool

BoolModel(value=0)
BoolModel(value="yes")
BoolModel(value=2)  # E: Argument value `Literal[2]` is not valid for Pydantic field `value` with type `bool`
BoolModel(value="maybe")  # E: Argument value `Literal['maybe']` is not valid for Pydantic field `value` with type `bool`

class IntOrBoolModel(BaseModel):
    value: int | bool

IntOrBoolModel(value=5)
IntOrBoolModel(value="123")

def union_input(value: Literal[0, 5]) -> None:
    BoolModel(value=value)  # E: Argument value `Literal[5]` is not valid
    "#,
);

pydantic_testcase!(
    test_lax_mode_nested_model_and_dataclass_inputs,
    r#"
from dataclasses import dataclass
from typing import Any, Mapping, assert_type
from pydantic import BaseModel

class Link(BaseModel):
    href: str

@dataclass
class Record:
    value: int

class Publication(BaseModel):
    links: list[Link]
    data: Link | Record

links: list[dict[str, str]] = [{"href": "example"}]
record: Mapping[str, Any] = {"value": 1}
publication = Publication(links=links, data=record)
Publication(links=[Link(href="example")], data=Record(1))
Publication(links=links, data=Link(href="example"))
Publication(links=[5], data=5)  # E: is not assignable to parameter `links` # E: is not assignable to parameter `data`
assert_type(publication.links, list[Link])
assert_type(publication.data, Link | Record)
    "#,
);

pydantic_testcase!(
    test_lax_mode_custom_schema_inputs,
    r#"
from typing import Any, assert_type
from pydantic import BaseModel, ConfigDict

class CustomSchema:
    @classmethod
    def __get_pydantic_core_schema__(cls, source_type: Any, handler: Any) -> Any: ...

class InheritedSchema(CustomSchema):
    pass

class CustomModel(BaseModel):
    @classmethod
    def __get_pydantic_core_schema__(cls, source_type: Any, handler: Any) -> Any: ...

class Plain:
    pass

class Model(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True)
    pa_schema: InheritedSchema | bool
    custom_model: CustomModel
    plain: Plain | None

m = Model(pa_schema=InheritedSchema, custom_model=5, plain=Plain())
Model(pa_schema=5, custom_model="input", plain=None)
Model(pa_schema=InheritedSchema, custom_model=5, plain=5)  # E: is not assignable to parameter `plain`
assert_type(m.pa_schema, InheritedSchema | bool)
assert_type(m.custom_model, CustomModel)
    "#,
);

// https://github.com/meta/pyrefly/issues/5545
pydantic_testcase!(
    test_lax_mode_new_type_inputs,
    r#"
from typing import NewType
from pydantic import BaseModel

CommitId = NewType("CommitId", str)
Count = NewType("Count", int)
Ids = NewType("Ids", list[int])

class Model(BaseModel):
    commit_id: CommitId
    count: Count
    parents: tuple[CommitId, ...]
    ids: Ids

Model(commit_id=CommitId("abc"), count=Count(1), parents=(CommitId("def"),), ids=Ids([1]))
Model(commit_id="abc", count="1", parents=("def",), ids=["1"])
Model(
    commit_id=1,  # E: is not assignable to parameter `commit_id`
    count=None,  # E: is not assignable to parameter `count`
    parents=(1,),  # E: is not assignable to parameter `parents`
    ids=[None],  # E: is not assignable to parameter `ids`
)
    "#,
);

// https://github.com/meta/pyrefly/issues/5543
pydantic_testcase!(
    test_lax_mode_nested_model_with_model_validator,
    r#"
from typing import Any, Self
from pydantic import BaseModel, model_validator

class MenuRule(BaseModel):
    id: str

    @model_validator(mode="before")
    @classmethod
    def from_str(cls, value: Any) -> Any:
        return {"id": value} if isinstance(value, str) else value

class InheritedMenuRule(MenuRule):
    pass

class CheckedRule(BaseModel):
    id: str

    @model_validator(mode="after")
    def check(self) -> Self:
        return self

class Action(BaseModel):
    menus: list[MenuRule] = []
    inherited: InheritedMenuRule | None = None
    checked: CheckedRule | None = None

Action(menus=["edit"])
Action(inherited="edit")
Action(checked="edit")  # E: is not assignable to parameter `checked`
    "#,
);

// https://github.com/meta/pyrefly/issues/5543
pydantic_testcase!(
    test_lax_mode_nested_model_from_attributes,
    r#"
from pydantic import BaseModel, ConfigDict

class Row(BaseModel):
    state_id: int

class RowResponse(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    state_id: int

class InheritedRowResponse(RowResponse):
    pass

class KeywordRowResponse(BaseModel, from_attributes=True):
    state_id: int

SHARED_CONFIG = ConfigDict(from_attributes=True)

class SharedConfigRowResponse(BaseModel):
    model_config = SHARED_CONFIG
    state_id: int

class Response(BaseModel):
    current: RowResponse | None = None
    rows: list[RowResponse] = []
    inherited: InheritedRowResponse | None = None
    keyword: KeywordRowResponse | None = None
    shared: SharedConfigRowResponse | None = None
    plain: Row | None = None

row = Row(state_id=1)
Response(current=row, rows=[row], inherited=row, keyword=row, shared=row)
Response(plain=RowResponse(state_id=1))  # E: is not assignable to parameter `plain`
    "#,
);

pydantic_testcase!(
    test_lax_mode_builtin_schema_inputs,
    r#"
from enum import Enum, IntEnum
from ipaddress import IPv4Address
from typing import NamedTuple, assert_type
from pydantic import BaseModel

class Color(Enum):
    RED = "red"

class Level(IntEnum):
    LOW = 1

class Point(NamedTuple):
    x: int

class Model(BaseModel):
    color: Color
    level: Level
    point: Point
    ip: IPv4Address

m = Model(color="red", level=1, point=(1,), ip="1.2.3.4")
assert_type(m.color, Color)
assert_type(m.point, Point)
    "#,
);

pydantic_testcase!(
    test_lax_mode_generic_class_inputs,
    r#"
from collections import defaultdict
from pydantic import BaseModel, ConfigDict

class Box[T]:
    pass

class Model(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True)
    box: Box[int]
    counts: defaultdict[str, int]

Model(box=Box[str](), counts={})
Model(box=5, counts={})  # E: is not assignable to parameter `box`
    "#,
);

pydantic_testcase!(
    test_lax_mode_list_and_set_invariance,
    r#"
from pydantic import BaseModel
from collections import deque
from typing import reveal_type

class TestModel(BaseModel):
    name: str
    things: list[str]
    tags: set[str]

list_of_things = ["thing1", "thing2"]
set_of_tags = {"tag1", "tag2"}
a = TestModel(name="test", things=list_of_things, tags=set_of_tags)

deque_of_bytes: deque[bytes] = deque([b"thing1", b"thing2"])
b = TestModel(name="test", things=deque_of_bytes, tags=set_of_tags)

# When reading the field back, you get the original declared type (list[str]), not Iterable[LaxStr]
reveal_type(a.things)  # E: revealed type: list[str]
reveal_type(a.tags)    # E: revealed type: set[str]
    "#,
);

pydantic_testcase!(
    test_lax_mode_dict_invariance,
    r#"
from pydantic import BaseModel
from typing import reveal_type

class TestModel(BaseModel):
    name: str
    metadata: dict[str, str]

my_dict = {"key1": "value1", "key2": "value2"}
a = TestModel(name="test", metadata=my_dict)
    "#,
);

pydantic_testcase!(
    test_lax_mode_frozenset_invariance,
    r#"
from pydantic import BaseModel

class TestModel(BaseModel):
    items: frozenset[str]

my_frozenset = frozenset({"a", "b"})
a = TestModel(items=my_frozenset)

my_list = ["a", "b"]
b = TestModel(items=my_list)

my_set = {"a", "b"}
c = TestModel(items=my_set)
    "#,
);

pydantic_testcase!(
    test_lax_mode_mapping_field,
    r#"
from typing import Any, Mapping, reveal_type
from pydantic import BaseModel

class Model(BaseModel):
    parameters: Mapping[str, Any] | None = None

# The key type should not be expanded for Mapping, since Mapping is invariant in its key type.
# This means dict[str, Any] | None should be assignable to the init parameter.
reveal_type(Model.__init__)  # E: revealed type: (self: Model, *, parameters: Mapping[str, Any] | None = ..., **Unknown) -> None

d: dict[str, Any] = {}
Model(parameters=d)
    "#,
);

pydantic_testcase!(
    test_lax_mode_pattern_field,
    r#"
import re
from typing import reveal_type
from pydantic import BaseModel

class RegexSignature(BaseModel):
    signature: re.Pattern[str]

reveal_type(RegexSignature.__init__)  # E: revealed type: (self: RegexSignature, *, signature: Pattern[str] | str, **Unknown) -> None
RegexSignature(signature=re.compile(r"needle"))
RegexSignature(signature="needle")
RegexSignature(signature=b"needle")  # E: Argument `Literal[b'needle']` is not assignable to parameter `signature`
    "#,
);

pydantic_testcase!(
    test_lax_mode_other,
    r#"
from pydantic import BaseModel
from typing import Any, reveal_type

class Model1(BaseModel):
    x: None

reveal_type(Model1.__init__)  # E: revealed type: (self: Model1, *, x: None, **Unknown) -> None

class Model2(BaseModel):
    y: Any

reveal_type(Model2.__init__)  # E: revealed type: (self: Model2, *, y: Any, **Unknown) -> None
    "#,
);

pydantic_testcase!(
    test_lax_mode_type_expansion,
    r#"
from pydantic import BaseModel
from typing import reveal_type

class Model1(BaseModel):
    t: type[int]

reveal_type(Model1.__init__)  # E: revealed type: (self: Model1, *, t: type[LaxInt], **Unknown) -> None

    "#,
);
