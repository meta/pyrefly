# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# All occurrences of `x` should share the same symbol, skipping `middle`.
def outer():
    x = 0

    def middle():
        def inner():
            nonlocal x
            x = 1
            return x

        return inner

    return x, middle
