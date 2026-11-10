# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape


def test_permute_list_dimensions() -> None:
    tensor = torch.ones(2, 3, 4)
    result = torch.permute(tensor, [2, 0, 1])  # E: No matching overload
    assert_shape(result.shape, (4, 2, 3))  # E: assert_shape
    result = tensor.permute([2, 0, 1])  # E: No matching overload
    assert_shape(result.shape, (4, 2, 3))  # E: assert_shape
    result = torch.permute(tensor, [-1, -3, -2])  # E: No matching overload
    assert_shape(result.shape, (4, 2, 3))  # E: assert_shape
    result = torch.permute(input=tensor, dims=[2, 0, 1])  # E: No matching overload
    assert_shape(result.shape, (4, 2, 3))  # E: assert_shape
    with assert_raises(RuntimeError):
        torch.permute(input=tensor, dims=[0, 0, 1])  # E: No matching overload


def test_permute_dynamic_list_dimensions() -> None:
    tensor = torch.ones(2, 3, 4)
    dimensions = list(reversed(range(tensor.dim())))
    result = torch.permute(tensor, dimensions)  # E: No matching overload
    assert tuple(result.shape) == (4, 3, 2)
    result = tensor.permute(dimensions)  # E: No matching overload
    assert tuple(result.shape) == (4, 3, 2)
    with assert_raises(RuntimeError):
        tensor.permute(dimensions + [0])  # E: No matching overload


def test_permute_tuple_dimensions() -> None:
    tensor = torch.ones(2, 3, 4)
    assert_shape(torch.permute(tensor, (2, 0, 1)).shape, (4, 2, 3))
    assert_shape(tensor.permute(2, 0, 1).shape, (4, 2, 3))
    result = torch.permute(input=tensor, dims=(2, 0, 1))  # E: No matching overload
    assert_shape(result.shape, (4, 2, 3))  # E: assert_shape
