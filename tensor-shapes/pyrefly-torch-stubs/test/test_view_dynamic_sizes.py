# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type, TYPE_CHECKING

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def dynamic_view(tensor: Tensor, sizes: tuple[int, ...]) -> Tensor:
    result = tensor.view(*sizes)
    assert_type(result, Tensor)
    return result


def test_view_integer_starred_sizes() -> None:
    source = torch.arange(6).reshape(2, 3)
    result = dynamic_view(source, (3, 2))
    assert torch.equal(result, torch.tensor([[0, 1], [2, 3], [4, 5]]))
    assert result.data_ptr() == source.data_ptr()
    if not TYPE_CHECKING:
        assert_shape(result.shape, (3, 2))


def test_view_mixed_fixed_and_starred_sizes() -> None:
    source = torch.ones((2, 3, 4))
    sizes: list[int] = [3, 4]
    result = source.view(2, *sizes)
    assert_type(result, Tensor)
    assert torch.equal(result, source)
    if not TYPE_CHECKING:
        assert_shape(result.shape, (2, 3, 4))


def partition_view(source: Tensor) -> Tensor:
    result = source.view(*source.shape[:-2], 2, -1, *source.shape[-1:])
    assert_type(result, Tensor)
    return result


def test_view_source_shape_slices_and_integer_pieces() -> None:
    source = torch.ones((6, 4))
    result = partition_view(source)
    assert torch.equal(result, torch.ones((2, 3, 4)))
    assert result.data_ptr() == source.data_ptr()
    if not TYPE_CHECKING:
        assert_shape(result.shape, (2, 3, 4))


def test_view_precise_reshape_and_dtype_controls() -> None:
    source = torch.ones((2, 4))
    assert_shape(source.view(4, 2).shape, (4, 2))
    assert_shape(source.view((4, 2)).shape, (4, 2))
    assert_shape(source.view(2, -1).shape, (2, 4))
    reinterpreted = source.view(dtype=torch.int16)
    assert_type(reinterpreted, Tensor)
    assert tuple(reinterpreted.shape) == (2, 8)
    with assert_raises(RuntimeError):
        source.view(3, 3)  # E: reshape


def test_view_rejects_non_integer_sizes() -> None:
    source = torch.ones((2, 3))
    with assert_raises(TypeError):
        source.view(2, "3")  # E: Unpacked argument
    with assert_raises(TypeError):
        source.view(2, 3.0)  # E: Unpacked argument
    if TYPE_CHECKING:
        wrong: tuple[float, ...] = (2.0, 3.0)
        source.view(*wrong)  # E: No matching overload
    assert_shape(source.shape, (2, 3))
