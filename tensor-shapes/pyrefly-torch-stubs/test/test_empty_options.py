# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_empty_pin_memory() -> None:
    vector = torch.empty(4, pin_memory=False)
    assert_shape(vector.shape, (4,))
    matrix = torch.empty((2, 3), pin_memory=False)
    assert_shape(matrix.shape, (2, 3))
    separate = torch.empty(2, 3, pin_memory=False)
    assert_shape(separate.shape, (2, 3))


def test_empty_list_sizes() -> None:
    dims: list[int] = [2, 3]
    result = torch.empty(dims)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (2, 3)
    dtype = torch.float64
    result = torch.empty([2, 3], dtype=dtype, pin_memory=False)
    assert_type(result, Tensor)
    assert result.dtype == torch.float64
    assert tuple(result.shape) == (2, 3)
    assert_shape(torch.empty((2, 3)).shape, (2, 3))


def test_empty_layout_memory_format_and_out() -> None:
    strided = torch.empty(2, 3, layout=torch.strided)
    assert strided.layout == torch.strided
    assert_shape(strided.shape, (2, 3))
    fmt = torch.channels_last
    image = torch.empty((2, 3, 4, 5), memory_format=fmt)
    assert image.is_contiguous(memory_format=torch.channels_last)
    assert_shape(image.shape, (2, 3, 4, 5))
    out = torch.empty(0)
    result = torch.empty((2, 3), out=out)
    assert_shape(result.shape, (2, 3))
    assert result.data_ptr() == out.data_ptr()


def test_empty_rejects_invalid_options() -> None:
    with assert_raises(TypeError):
        torch.empty((2, 3), device=object())  # E: parameter `device`
    with assert_raises(TypeError):
        torch.empty((2, 3), dtype="float32")  # E: parameter `dtype`
