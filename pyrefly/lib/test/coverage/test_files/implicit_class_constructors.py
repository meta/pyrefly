# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import builtins
import collections
import pathlib
import xml.etree.ElementTree
from pathlib import Path as P


class Plain:
    pass


class Factory:
    def Plain(self) -> Plain:
        return Plain()


class Box[T]:
    def __init__(self, value: T) -> None:
        pass


class Other:
    def __new__(cls) -> int:
        return 0


def make_plain() -> Plain:
    return Plain()


# Shadows the builtin, so `list()` below doesn't name the class it returns.
def list() -> builtins.list[int]:
    return []


Alias = Plain

PLAIN = Plain()
ROOT = pathlib.Path(".")
ALIASED_ROOT = P(".")
FACTORY = Factory()
NUMBER = int("1")
SPAN = range(3)
ALIASED = Alias()

BOX = Box(1)
QUEUE = collections.deque()
OTHER = Other()
MADE = make_plain()
METHOD = FACTORY.Plain()
NESTED = xml.etree.ElementTree.Element("a")
SHADOWED = list()
