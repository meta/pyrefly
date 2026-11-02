from typing import Generic, TypeVar

_T = TypeVar("_T")

class TypeEngine(Generic[_T]):
    ...

class TypeDecorator(TypeEngine[_T], Generic[_T]):
    ...
