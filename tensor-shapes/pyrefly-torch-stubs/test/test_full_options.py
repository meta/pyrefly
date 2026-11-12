# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_full_pinning_options() -> None:
    matrix = torch.full((2, 3), 7.0, pin_memory=False)
    assert_type(matrix, Tensor[[2, 3]])
    assert_shape(matrix.shape, (2, 3))
    assert not matrix.is_pinned()
    unset = torch.full((2, 3), 7, pin_memory=None, requires_grad=None)
    assert_shape(unset.shape, (2, 3))
    assert not unset.requires_grad
    sizes: list[int] = [2, 3]
    gradual = torch.full(sizes, 7.0, pin_memory=False)
    assert_type(gradual, Tensor)
    assert tuple(gradual.shape) == (2, 3)


def test_full_layout_and_output() -> None:
    result = torch.full((2, 3), 7.0, layout=torch.strided)
    assert result.layout == torch.strided
    assert_shape(result.shape, (2, 3))
    out = torch.empty((2, 3))
    result = torch.full((2, 3), 7.0, out=out)
    assert result is out
    assert torch.equal(result, torch.tensor([[7.0] * 3, [7.0] * 3]))
    assert_shape(result.shape, (2, 3))
    resized = torch.full((2, 3), 7.0, out=torch.empty(0))
    assert_shape(resized.shape, (2, 3))
    dtype: torch.dtype = torch.float64
    result = torch.full((2, 3), 7, dtype=dtype, device=torch.device("cpu"))
    assert result.dtype == torch.float64
    assert result.device == torch.device("cpu")
    assert_shape(result.shape, (2, 3))


def test_full_fill_values() -> None:
    integers = torch.full((2,), 3)
    assert integers.dtype == torch.int64
    flags = torch.full((2,), True)
    assert flags.dtype == torch.bool
    complex_values = torch.full((2,), 1 + 2j)
    assert complex_values.dtype == torch.complex64
    assert_shape(complex_values.shape, (2,))


def test_full_rejects_invalid_options() -> None:
    with assert_raises(TypeError):
        torch.full((2, 3), 7.0, dtype="float64")  # E: parameter `dtype`
    with assert_raises(TypeError):
        torch.full((2, 3), 7.0, device=object())  # E: parameter `device`
    with assert_raises(TypeError):
        torch.full((2, 3), 7.0, out=object())  # E: parameter `out`
    with assert_raises(TypeError):
        torch.full((2, 3), 7.0, layout="strided")  # E: parameter `layout`
    with assert_raises(TypeError):
        torch.full((2, 3), 7.0, pin_memory="yes")  # E: parameter `pin_memory`
    with assert_raises(TypeError):
        torch.full((2, 3), "seven")  # E: No matching overload
    with assert_raises(TypeError):
        torch.full(2, 3, 7.0)  # E: No matching overload
