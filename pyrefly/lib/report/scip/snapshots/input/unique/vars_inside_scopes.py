# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import List


class X:
    items: List[int]

    def my_func(self):
        for x in self.items:
            y = x + 1  # noqa: F841

        if 5 in self.items:
            z = "oh ya"  # noqa: F841
