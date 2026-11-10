# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# format-options: showRanges
def decorator(func):
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)

    return wrapper


@decorator
def func(x: float) -> float:
    test = x

    return test
