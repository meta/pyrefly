# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Only the first binding of a symbol is its definition, except that every overload defines its
# parameters.
from typing import overload

x = 1
x = 2


def f():
    y = 1
    y = 2
    return y


class C:
    a = 1
    a = 2

    def __init__(self):
        self.b = 1
        self.b = 2

    @overload
    def m(self, v: int) -> int: ...
    @overload
    def m(self, v: str) -> str: ...
    def m(self, v):
        return v


def g(): ...
def g(): ...  # noqa: F811


def h(p):
    p = 1
    return p


for x in range(3):  # noqa: B007
    pass
