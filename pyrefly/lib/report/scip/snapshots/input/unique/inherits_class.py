# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.


class A:
    def x(self) -> int:
        raise NotImplementedError

    def matched_despite_different_type(self, x: int):
        pass


class B(A):
    def x(self) -> int:
        return 5

    def matched_despite_different_type(self, x: int, y: int):
        pass

    def unrelated(self):
        pass
