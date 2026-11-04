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
class Enum(String, TypeEngine[str | PythonEnum]):
    def __init__(self, *enums: str | type[PythonEnum]) -> None:
        ...
