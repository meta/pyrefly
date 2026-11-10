# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.


def exported_function():
    return "function"


class MyClass:
    """This is a class and it is cool"""

    def __init__(self):
        pass

    def exported_function(self):
        return "exported"


this_class = MyClass()
