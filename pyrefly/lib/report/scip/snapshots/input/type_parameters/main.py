# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# The declaration of `T` and both annotations should share a defined symbol.
def identity[T](x: T) -> T:
    return x
