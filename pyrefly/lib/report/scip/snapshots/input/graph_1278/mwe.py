from typing import TypeVar, Generic, Callable, Iterator, ParamSpec

_T_co = TypeVar("_T_co")
_P = ParamSpec("_P")

class X(Generic[_T_co]):
    pass

def decorate(func: Callable[_P, Iterator[_T_co]]) -> Callable[_P, X[_T_co]]: ...

class Foo:
    @decorate
    def foo(self) -> Iterator[None]: ...

@decorate
def noop():
    yield

class FooImpl(Foo):
    def foo(self):
        return noop()