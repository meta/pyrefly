# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_empty_pin_memory() -> None:
    vector = torch.empty(4, pin_memory=False)  # E: No matching overload
    assert_shape(vector.shape, (4,))  # E: assert_shape((*IntTuple), (4,)) failed
    matrix = torch.empty((2, 3), pin_memory=False)  # E: No matching overload
    assert_shape(matrix.shape, (2, 3))  # E: assert_shape((*IntTuple), (2, 3)) failed
    separate = torch.empty(2, 3, pin_memory=False)  # E: Unexpected keyword
    assert_shape(separate.shape, (2, 3))


def test_empty_list_sizes() -> None:
    dims: list[int] = [2, 3]
    result = torch.empty(dims)  # E: No matching overload
    assert_type(result, Tensor)  # E: assert_type(Unknown, Tensor) failed
    assert tuple(result.shape) == (2, 3)
    dtype = torch.float64
    result = torch.empty([2, 3], dtype=dtype, pin_memory=False)  # E: No matching
    assert_type(result, Tensor)  # E: assert_type(Unknown, Tensor) failed
    assert result.dtype == torch.float64
    assert tuple(result.shape) == (2, 3)
    assert_shape(torch.empty((2, 3)).shape, (2, 3))


def test_empty_layout_memory_format_and_out() -> None:
    strided = torch.empty(2, 3, layout=torch.strided)  # E: Unexpected keyword
    assert strided.layout == torch.strided
    assert_shape(strided.shape, (2, 3))
    fmt = torch.channels_last
    image = torch.empty((2, 3, 4, 5), memory_format=fmt)  # E: No matching overload
    assert image.is_contiguous(memory_format=torch.channels_last)
    assert_shape(image.shape, (2, 3, 4, 5))  # E: assert_shape
    out = torch.empty(0)
    result = torch.empty((2, 3), out=out)  # E: No matching overload
    assert_shape(result.shape, (2, 3))  # E: assert_shape((*IntTuple), (2, 3)) failed
    assert result.data_ptr() == out.data_ptr()


def test_empty_rejects_invalid_options() -> None:
    with assert_raises(TypeError):
        torch.empty((2, 3), device=object())
    with assert_raises(TypeError):
        torch.empty((2, 3), dtype="float32")
