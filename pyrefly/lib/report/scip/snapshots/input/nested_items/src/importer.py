# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from foo.bar import InitClass
from foo.bar.baz.mod import AnotherNestedMuchWow, SuchNestedMuchWow

print(SuchNestedMuchWow().class_item)
print(AnotherNestedMuchWow().other_item)
print(InitClass().init_item)
