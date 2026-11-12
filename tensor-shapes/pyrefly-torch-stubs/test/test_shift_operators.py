# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_right_shift_by_scalar_and_tensor() -> None:
    values = torch.tensor([0, 1, 2, 3])
    shifted = values >> 1
    assert_type(shifted, Tensor[[4]])
    assert shifted.tolist() == [0, 0, 1, 1]
    rows = torch.tensor([[8], [16]])
    broadcast = rows >> values
    assert_type(broadcast, Tensor[[2, 4]])
    assert broadcast.tolist() == [[8, 4, 2, 1], [16, 8, 4, 2]]
    assert_shape(values.shape, (4,))


def test_scalar_right_shifted_by_tensor() -> None:
    bits = torch.arange(4)
    shifted = 5 >> bits
    assert_type(shifted, Tensor[[4]])
    assert shifted.tolist() == [5, 2, 1, 0]
    flags = ((5 >> bits) & 1).float()
    assert flags.tolist() == [1.0, 0.0, 1.0, 0.0]
    assert_shape(bits.shape, (4,))


def test_scalar_right_shifted_by_parameter() -> None:
    parameter = torch.nn.Parameter(torch.arange(4), requires_grad=False)
    shifted = 5 >> parameter
    # Torch returns a plain tensor, not a parameter.
    assert_type(shifted, Tensor[[4]])
    assert type(shifted) is torch.Tensor
    assert shifted.tolist() == [5, 2, 1, 0]
    assert_shape(parameter.shape, (4,))


def test_right_shift_runtime_errors() -> None:
    bits = torch.arange(4)
    wrong = torch.ones(3, dtype=torch.int64)
    assert_shape(bits.shape, (4,))
    # Torch declares floating-point scalars, but its integer kernels reject them.
    with assert_raises(NotImplementedError):
        5.0 >> bits
    with assert_raises(NotImplementedError):
        bits >> 1.0
    with assert_raises(TypeError):
        bits >> "1"  # E: `>>` is not supported
    with assert_raises(TypeError):
        "1" >> bits  # E: `>>` is not supported
    with assert_raises(RuntimeError):
        bits >> wrong  # E: broadcast
