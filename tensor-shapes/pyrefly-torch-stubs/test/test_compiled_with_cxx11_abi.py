# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises


def test_compiled_with_cxx11_abi_returns_bool() -> None:
    result = torch.compiled_with_cxx11_abi()
    assert_type(result, bool)
    assert type(result) is bool
    assert result is torch._C._GLIBCXX_USE_CXX11_ABI
    with assert_raises(TypeError):
        torch.compiled_with_cxx11_abi(1)  # E: Expected 0 positional
