from typing import dataclass_transform

from ._orm_constructors import mapped_column

class DeclarativeBase: ...

@dataclass_transform(field_specifiers=(mapped_column,))
class DCTransformDeclarative(type): ...

class MappedAsDataclass(metaclass=DCTransformDeclarative): ...
