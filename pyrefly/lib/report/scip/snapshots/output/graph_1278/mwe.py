# < definition scip-python python snapshot-util 0.1 mwe/__init__:

from typing import TypeVar, Generic, Callable, Iterator, ParamSpec
#    ^^^^^^ reference  python-stdlib 3.13 typing/__init__:
#                  ^^^^^^^ reference  python-stdlib 3.13 typing/TypeVar#
#                           ^^^^^^^ reference  python-stdlib 3.13 typing/Generic.
#                                    ^^^^^^^^ reference  python-stdlib 3.13 typing/Callable.
#                                              ^^^^^^^^ reference  python-stdlib 3.13 typing/Iterator#
#                                                        ^^^^^^^^^ reference  python-stdlib 3.13 typing/ParamSpec#

_T_co = TypeVar("_T_co")
#^^^^ definition  snapshot-util 0.1 mwe/_T_co.
#       ^^^^^^^ reference  python-stdlib 3.13 typing/TypeVar#
_P = ParamSpec("_P")
#^ definition  snapshot-util 0.1 mwe/_P.
#    ^^^^^^^^^ reference  python-stdlib 3.13 typing/ParamSpec#

class X(Generic[_T_co]):
#     ^ definition  snapshot-util 0.1 mwe/X#
#     relationship implementation scip-python python python-stdlib 3.13 typing/Generic.
#       ^^^^^^^ reference  python-stdlib 3.13 typing/Generic.
#               ^^^^^ reference  snapshot-util 0.1 mwe/_T_co.
    pass

def decorate(func: Callable[_P, Iterator[_T_co]]) -> Callable[_P, X[_T_co]]: ...
#   ^^^^^^^^ definition  snapshot-util 0.1 mwe/decorate().
#            ^^^^ definition  snapshot-util 0.1 mwe/decorate().(func)
#                  ^^^^^^^^ reference  python-stdlib 3.13 typing/Callable.
#                           ^^ reference  snapshot-util 0.1 mwe/_P.
#                               ^^^^^^^^ reference  python-stdlib 3.13 typing/Iterator#
#                                        ^^^^^ reference  snapshot-util 0.1 mwe/_T_co.
#                                                    ^^^^^^^^ reference  python-stdlib 3.13 typing/Callable.
#                                                             ^^ reference  snapshot-util 0.1 mwe/_P.
#                                                                 ^ reference  snapshot-util 0.1 mwe/X#
#                                                                   ^^^^^ reference  snapshot-util 0.1 mwe/_T_co.

class Foo:
#     ^^^ definition  snapshot-util 0.1 mwe/Foo#
    @decorate
#    ^^^^^^^^ reference  snapshot-util 0.1 mwe/decorate().
    def foo(self) -> Iterator[None]: ...
#       ^^^ definition  snapshot-util 0.1 mwe/Foo#foo().
#           ^^^^ definition  snapshot-util 0.1 mwe/Foo#foo().(self)
#                    ^^^^^^^^ reference  python-stdlib 3.13 typing/Iterator#

@decorate
#^^^^^^^^ reference  snapshot-util 0.1 mwe/decorate().
def noop():
#   ^^^^ definition  snapshot-util 0.1 mwe/noop().
    yield

class FooImpl(Foo):
#     ^^^^^^^ definition  snapshot-util 0.1 mwe/FooImpl#
#     relationship implementation scip-python python snapshot-util 0.1 mwe/Foo#
#             ^^^ reference  snapshot-util 0.1 mwe/Foo#
    def foo(self):
#       ^^^ definition  snapshot-util 0.1 mwe/FooImpl#foo().
#       relationship implementation scip-python python snapshot-util 0.1 mwe/Foo#foo().
#           ^^^^ definition  snapshot-util 0.1 mwe/FooImpl#foo().(self)
        return noop()
#              ^^^^ reference  snapshot-util 0.1 mwe/noop().
