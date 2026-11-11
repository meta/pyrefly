# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.


class A:
    def grandparent(self) -> bool:
        return True


class B(A): ...


class C(B):
    def grandparent(self) -> bool:
        return False
