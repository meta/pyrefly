# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type, TypeVar

import numpy as np
import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor

_Array = TypeVar("_Array", Tensor, np.ndarray)


def _shift_active(value: _Array, offset: int) -> _Array:
    active = value >= 0
    value[active] += offset
    return value


def test_numpy_bool_mask_reads() -> None:
    tensor = torch.tensor(
        [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0], [6.0, 7.0, 8.0], [9.0, 10.0, 11.0]]
    )
    assert_shape(tensor.shape, (4, 3))
    rows = np.array([True, False, True, False])
    selected = tensor[rows]
    assert_type(selected, Tensor)
    assert selected.tolist() == [[0.0, 1.0, 2.0], [6.0, 7.0, 8.0]]
    elements = np.zeros((4, 3), dtype=np.bool_)
    elements[0, 0] = elements[3, 2] = True
    picked = tensor[elements]
    assert_type(picked, Tensor)
    assert picked.tolist() == [0.0, 11.0]


def test_numpy_bool_mask_writes() -> None:
    values = torch.tensor([3, -1, 5, -1])
    active = values.numpy() >= 0
    values[active] += 10
    assert values.tolist() == [13, -1, 15, -1]
    filled = torch.zeros(4)
    filled[np.array([True, False, True, False])] = 1.0
    assert filled.tolist() == [1.0, 0.0, 1.0, 0.0]
    assert_shape(filled.shape, (4,))


def test_numpy_bool_mask_in_constrained_function() -> None:
    shifted = _shift_active(torch.tensor([3, -1, 5]), 10)
    assert shifted.tolist() == [13, -1, 15]
    array = _shift_active(np.array([3, -1, 5]), 10)
    assert array.tolist() == [13, -1, 15]
    assert_shape(torch.zeros(3).shape, (3,))


def test_shaped_indexing_controls() -> None:
    tensor = torch.zeros((4, 3))
    assert_shape(tensor[0].shape, (3,))
    assert_shape(tensor[1:3].shape, (2, 3))
    assert_shape(tensor[0, 1:].shape, (2,))
    assert_shape(tensor[..., None].shape, (4, 3, 1))
    assert tuple(tensor[[0, 2]].shape) == (2, 3)
    mask = torch.tensor([True, False, True, False])
    assert tuple(tensor[mask].shape) == (2, 3)


def test_indexing_rejects_strings_and_floats() -> None:
    tensor = torch.zeros(4)
    assert_shape(tensor.shape, (4,))
    with assert_raises(IndexError):
        tensor["bad"]  # E: Cannot index
    with assert_raises(IndexError):
        tensor[1.5]  # E: Cannot index
