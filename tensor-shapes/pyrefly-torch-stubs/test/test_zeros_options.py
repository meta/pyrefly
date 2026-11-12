# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_zeros_pinning_options() -> None:
    vector = torch.zeros(3, pin_memory=False)  # E: Unexpected keyword
    assert_type(vector, Tensor[[3]])  # E: assert_type
    assert_shape(vector.shape, (3,))  # E: assert_shape
    assert not vector.is_pinned()
    matrix = torch.zeros((2, 3), pin_memory=False)  # E: Unexpected keyword
    assert_type(matrix, Tensor[[2, 3]])  # E: assert_type
    assert_shape(matrix.shape, (2, 3))  # E: assert_shape
    separate = torch.zeros(2, 3, pin_memory=False)  # E: Unexpected keyword
    assert_shape(separate.shape, (2, 3))
    sizes: list[int] = [2, 3]
    gradual = torch.zeros(sizes, pin_memory=False)  # E: No matching overload
    assert_type(gradual, Tensor)  # E: assert_type
    assert tuple(gradual.shape) == (2, 3)


def test_zeros_layout_and_output() -> None:
    result = torch.zeros((2, 3), layout=torch.strided)  # E: Unexpected keyword
    assert result.layout == torch.strided
    assert_shape(result.shape, (2, 3))  # E: assert_shape
    out = torch.empty((2, 3))
    result = torch.zeros((2, 3), out=out)  # E: Unexpected keyword
    assert result is out
    assert torch.equal(result, torch.tensor([[0, 0, 0], [0, 0, 0]]))
    assert_shape(result.shape, (2, 3))
    dtype: torch.dtype = torch.float64
    result = torch.zeros((2, 3), dtype=dtype, device=torch.device("cpu"))
    assert result.dtype == torch.float64
    assert result.device == torch.device("cpu")
    assert_shape(result.shape, (2, 3))


def test_zeros_rejects_invalid_options() -> None:
    with assert_raises(TypeError):
        torch.zeros((2, 3), dtype="float64")
    with assert_raises(TypeError):
        torch.zeros((2, 3), device=object())
    with assert_raises(TypeError):
        torch.zeros((2, 3), out=object())  # E: Unexpected keyword
    with assert_raises(TypeError):
        torch.zeros((2, 3), layout="strided")  # E: Unexpected keyword
    with assert_raises(TypeError):
        torch.zeros((2, 3), pin_memory="yes")  # E: Unexpected keyword
