# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_view_dtype_reinterprets_last_dimension() -> None:
    tensor = torch.ones((2, 3))
    assert_shape(tensor.shape, (2, 3))
    result = tensor.view(torch.int16)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (2, 6)
    result = tensor.view(dtype=torch.int16)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (2, 6)
    result = tensor.view(torch.float8_e4m3fn)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (2, 12)


def test_view_typed_dtype_variable() -> None:
    tensor = torch.ones((2, 4))
    assert_shape(tensor.shape, (2, 4))
    dtype: torch.dtype = torch.float64
    result = tensor.view(dtype)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (2, 2)


def test_view_shape_controls() -> None:
    tensor = torch.ones((2, 3))
    assert_shape(tensor.view(3, 2).shape, (3, 2))
    assert_shape(tensor.view((3, 2)).shape, (3, 2))
    sized = tensor.view(size=(3, 2))
    assert_shape(sized.shape, (3, 2))
    with assert_raises(TypeError):
        tensor.view(shape=(3, 2))  # E: No matching overload
    with assert_raises(TypeError):
        tensor.view("float32")  # E: No matching overload
