from typing import Generic, TypeVar
from .base import _DeclarativeMapped

_T = TypeVar("_T")

class MappedColumn(_DeclarativeMapped[_T]):
    column: object

    def __init__(self) -> None:
        ...
