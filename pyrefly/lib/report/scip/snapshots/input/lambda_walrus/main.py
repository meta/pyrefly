# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Each `hidden` should have a distinct local symbol.
first = lambda: (hidden := 1)  # noqa: E731, F841
second = lambda: (hidden := 2)  # noqa: E731, F841
