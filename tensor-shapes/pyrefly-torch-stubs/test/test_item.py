# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import assert_type, TYPE_CHECKING

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_item_scalar() -> None:
    scalar = torch.tensor(2.0)
    assert_shape(scalar.shape, ())
    assert_type(scalar.item(), float | int)
    assert scalar.item() == 2.0


def test_item_rejects_multiple_elements() -> None:
    vector = torch.ones(2)
    assert_shape(vector.shape, (2,))

    with assert_raises(RuntimeError):
        vector.item()

    matrix = torch.ones((5, 7))
    assert_shape(matrix.shape, (5, 7))

    with assert_raises(RuntimeError):
        matrix.item()

    empty = torch.ones(0)
    assert_shape(empty.shape, (0,))

    with assert_raises(RuntimeError):
        empty.item()


def test_item_accepts_single_element_non_scalar() -> None:
    vector = torch.ones(1)
    assert_shape(vector.shape, (1,))
    value = vector.item()
    assert_type(value, float | int)
    assert value == 1.0

    cube = torch.ones((1, 1, 1))
    assert_shape(cube.shape, (1, 1, 1))
    value = cube.item()
    assert value == 1.0

    count = len([0])
    sized = torch.ones(count)
    assert tuple(sized.shape) == (1,)
    value = sized.item()
    assert value == 1.0


def test_item_value_kinds() -> None:
    integer = torch.tensor([3])
    assert_shape(integer.shape, (1,))
    value = integer.item()
    assert_type(value, float | int)
    assert isinstance(value, int) and value == 3

    flag = torch.tensor([True])
    assert_shape(flag.shape, (1,))
    value = flag.item()
    assert value is True


if TYPE_CHECKING:

    def check_gradual_tensor(x: Tensor) -> None:
        assert_type(x.item(), float | int)
