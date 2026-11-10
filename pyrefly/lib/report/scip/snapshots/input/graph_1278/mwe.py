# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Callable, Generic, Iterator, ParamSpec, TypeVar

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
