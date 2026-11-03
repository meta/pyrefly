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
) -> MappedColumn[Any]: ...
