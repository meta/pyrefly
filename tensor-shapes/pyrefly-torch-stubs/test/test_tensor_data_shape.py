# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type, TYPE_CHECKING

import torch
from shape_extensions import assert_raises, assert_shape, IntTuple
from torch import nn, Tensor


def check_generic_parameter_data[Shape: IntTuple](
    parameter: nn.Parameter[Shape], replacement: Tensor[Shape]
) -> None:
    assert_type(parameter.data, Tensor[Shape])
    parameter.data = replacement


def test_tensor_data_keeps_shape_and_shares_storage() -> None:
    tensor = torch.ones((2, 3), requires_grad=True)
    data = tensor.data
    assert_type(data, Tensor[[2, 3]])
    assert type(data) is Tensor
    assert data is not tensor
    assert data.data_ptr() == tensor.data_ptr()
    assert not data.requires_grad
    replacement = torch.zeros((2, 3))
    tensor.data = replacement
    assert tensor.data_ptr() == replacement.data_ptr()
    assert_shape(tensor.shape, (2, 3))
    assert_shape(tensor.data.shape, (2, 3))


def test_parameter_data_is_plain_tensor_with_same_shape_replacement() -> None:
    parameter = nn.Parameter(torch.ones((2, 3)))
    data = parameter.data
    assert_type(data, Tensor[[2, 3]])
    assert type(data) is Tensor
    assert data.data_ptr() == parameter.data_ptr()
    assert not data.requires_grad
    identity = id(parameter)
    replacement = torch.zeros((2, 3))
    parameter.data = replacement
    parameter.data += 1.0
    check_generic_parameter_data(parameter, replacement)
    assert id(parameter) == identity and type(parameter) is nn.Parameter
    assert parameter.data_ptr() == replacement.data_ptr()
    assert torch.equal(parameter, torch.ones((2, 3)))
    assert_shape(parameter.shape, (2, 3))
    assert_shape(parameter.data.shape, (2, 3))


def test_known_data_shape_is_a_static_mutation_invariant() -> None:
    tensor = torch.ones((2, 3))
    parameter = nn.Parameter(torch.ones((2, 3)))
    if TYPE_CHECKING:
        # Native Torch permits this reshape; the precise static contract does not.
        tensor.data = torch.zeros((3, 2))  # E: not assignable
        parameter.data = torch.zeros((3, 2))  # E: not assignable
    else:
        identity = id(parameter)
        tensor.data = torch.zeros((3, 2))
        parameter.data = torch.zeros((3, 2))
        assert type(parameter) is nn.Parameter and id(parameter) == identity
        assert_shape(tensor.shape, (3, 2))
        assert_shape(parameter.shape, (3, 2))


def test_data_rejects_non_tensor_values() -> None:
    parameter = nn.Parameter(torch.ones((2, 3)))
    with assert_raises(TypeError):
        parameter.data = "not tensor"  # E: not assignable
    with assert_raises(TypeError):
        parameter.data = None  # E: not assignable
