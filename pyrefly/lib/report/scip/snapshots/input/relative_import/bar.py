# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from .foo import exported_function, this_class

if True:
    exported_function()

    this_class.exported_function()
