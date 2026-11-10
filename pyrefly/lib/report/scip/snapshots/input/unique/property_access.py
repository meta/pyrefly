# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Sequence


class PropertyClass:
    def __init__(self):
        pass

    @property
    def prop_ref(self):
        return 5


xs = [PropertyClass()]


def usage(xs: Sequence[PropertyClass]):
    def nested():
        for x in xs:
            print(x.prop_ref)
