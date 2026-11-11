# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# format-options: showDocs


class Example:
    # Note, only y has a type hint
    y: int

    def __init__(self, in_val):
        self.x = in_val
        self.x = self.x + 1
        self.y = in_val

    def something(self):
        print(self.x)
        print(self.y)
