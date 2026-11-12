# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_shape, IntTuple
from torch import nn, Tensor


def check_generic_parameter[Shape: IntTuple](parameter: nn.Parameter[Shape]) -> None:
    assert_type(parameter.clone(), Tensor[Shape])
    assert_type(parameter.detach(), Tensor[Shape])


def test_plain_tensor_keeps_clone_and_detach_shapes() -> None:
    tensor = torch.ones((2, 3), requires_grad=True)
    cloned = tensor.clone()
    detached = tensor.detach()
    assert_type(cloned, Tensor[[2, 3]])
    assert_type(detached, Tensor[[2, 3]])
    assert type(cloned) is Tensor
    assert type(detached) is Tensor
    assert cloned is not tensor and detached is not tensor
    assert cloned.data_ptr() != tensor.data_ptr()
    assert detached.data_ptr() == tensor.data_ptr()
    assert cloned.requires_grad and not detached.requires_grad
    assert torch.equal(cloned, tensor) and torch.equal(detached, tensor)
    assert_shape(cloned.shape, (2, 3))
    assert_shape(detached.shape, (2, 3))


def test_parameter_clone_and_detach_return_plain_tensors() -> None:
    parameter = nn.Parameter(torch.ones((2, 3)))
    cloned = parameter.clone()
    detached = parameter.detach()
    assert_type(cloned, Tensor[[2, 3]])
    assert_type(detached, Tensor[[2, 3]])
    assert type(parameter) is nn.Parameter
    assert type(cloned) is Tensor and type(detached) is Tensor
    assert cloned.data_ptr() != parameter.data_ptr()
    assert detached.data_ptr() == parameter.data_ptr()
    assert cloned.requires_grad and not detached.requires_grad
    check_generic_parameter(parameter)
    assert_shape(cloned.shape, (2, 3))
    assert_shape(detached.shape, (2, 3))


def test_snapshot_can_mix_detached_weights_and_plain_tensors() -> None:
    parameter = nn.Parameter(torch.ones((2, 3)))
    snapshot = {"weight": parameter.detach().clone()}
    assert_type(snapshot, dict[str, Tensor[[2, 3]]])
    snapshot["ema"] = torch.zeros((2, 3))
    assert type(snapshot["weight"]) is Tensor
    assert snapshot["weight"].data_ptr() != parameter.data_ptr()
    assert torch.equal(snapshot["ema"], torch.zeros((2, 3)))
    assert_shape(snapshot["weight"].shape, (2, 3))
