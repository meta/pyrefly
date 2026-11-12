# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type, SupportsIndex

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


class Position:
    def __index__(self) -> int:
        return 2


def _read(tensor: Tensor, index: SupportsIndex) -> Tensor:
    value = tensor[index]
    assert_type(value, Tensor)
    return value


def _write(tensor: Tensor, index: SupportsIndex, value: float) -> None:
    tensor[index] = value


def test_supports_index_reads_and_writes() -> None:
    tensor = torch.tensor([0.0, 1.0, 2.0, 3.0])
    assert _read(tensor, Position()).item() == 2.0
    _write(tensor, Position(), 9.0)
    assert tensor.tolist() == [0.0, 1.0, 9.0, 3.0]
    assert_shape(tensor.shape, (4,))


def test_tensor_mask_augmented_assignment() -> None:
    tensor = torch.tensor([3.0, -1.0, 5.0])
    active = tensor >= 0
    tensor[active] += 10
    assert tensor.tolist() == [13.0, -1.0, 15.0]
    assert_shape(tensor.shape, (3,))


def test_rejects_string_and_float_indices() -> None:
    tensor = torch.zeros(4)
    assert_shape(tensor.shape, (4,))
    with assert_raises(IndexError):
        tensor["bad"]  # E: Cannot index
    with assert_raises(IndexError):
        tensor[1.5]  # E: Cannot index
    with assert_raises(IndexError):
        tensor["bad"] = 1.0  # E: Cannot set item
    with assert_raises(IndexError):
        tensor[1.5] = 1.0  # E: Cannot set item
