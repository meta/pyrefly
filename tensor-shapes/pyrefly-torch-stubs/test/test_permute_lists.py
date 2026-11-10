# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_permute_list_dimensions() -> None:
    tensor = torch.ones(2, 3, 4)
    result = torch.permute(tensor, [2, 0, 1])
    assert_type(result, Tensor)
    assert tuple(result.shape) == (4, 2, 3)
    result = tensor.permute([2, 0, 1])
    assert_type(result, Tensor)
    assert tuple(result.shape) == (4, 2, 3)
    result = torch.permute(tensor, [-1, -3, -2])
    assert_type(result, Tensor)
    assert tuple(result.shape) == (4, 2, 3)
    result = torch.permute(input=tensor, dims=[2, 0, 1])
    assert_type(result, Tensor)
    assert tuple(result.shape) == (4, 2, 3)
    with assert_raises(RuntimeError):
        torch.permute(input=tensor, dims=[0, 0, 1])


def test_permute_dynamic_list_dimensions() -> None:
    tensor = torch.ones(2, 3, 4)
    dimensions = list(reversed(range(tensor.dim())))
    result = torch.permute(tensor, dimensions)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (4, 3, 2)
    result = tensor.permute(dimensions)
    assert_type(result, Tensor)
    assert tuple(result.shape) == (4, 3, 2)
    with assert_raises(RuntimeError):
        tensor.permute(dimensions + [0])


def test_permute_tuple_dimensions() -> None:
    tensor = torch.ones(2, 3, 4)
    assert_shape(torch.permute(tensor, (2, 0, 1)).shape, (4, 2, 3))
    assert_shape(tensor.permute(2, 0, 1).shape, (4, 2, 3))
    result = torch.permute(input=tensor, dims=(2, 0, 1))
    assert_shape(result.shape, (4, 2, 3))
