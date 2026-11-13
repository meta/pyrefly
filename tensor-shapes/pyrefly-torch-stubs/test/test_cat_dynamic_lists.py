# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from collections.abc import Callable
from typing import assert_type, TYPE_CHECKING

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def dynamic_cat(tensors: list[Tensor], dim: int = 0) -> Tensor:
    result = torch.cat(tensors, dim)
    assert_type(result, Tensor)
    return result


def use_concat(concat: Callable[[list[Tensor], int], Tensor]) -> Tensor:
    return concat([torch.as_tensor([1.0]), torch.as_tensor([2.0])], 0)


def test_cat_dynamic_lists_and_callback() -> None:
    tensors = [torch.as_tensor([1.0]), torch.as_tensor([2.0])]
    result = dynamic_cat(tensors)
    assert torch.equal(result, torch.tensor([1.0, 2.0]))
    callback = use_concat(torch.cat)
    assert_type(callback, Tensor)
    assert torch.equal(callback, torch.tensor([1.0, 2.0]))
    if not TYPE_CHECKING:
        assert_shape(result.shape, (2,))
        assert_shape(callback.shape, (2,))


def test_cat_starred_list_and_typed_out() -> None:
    dynamic: list[Tensor] = [torch.as_tensor([2.0, 3.0])]
    output = torch.zeros(3)
    first = torch.as_tensor([1.0])
    result = torch.cat([first, *dynamic], dim=0, out=output)
    assert_type(result, Tensor)
    assert result is output
    assert torch.equal(result, torch.tensor([1.0, 2.0, 3.0]))
    if not TYPE_CHECKING:
        assert_shape(result.shape, (3,))


def test_cat_literal_lists_keep_shapes() -> None:
    first = torch.ones((2, 3))
    second = torch.zeros((4, 3))
    result = torch.cat([first, second], dim=0)
    assert_type(result, Tensor[[6, 3]])
    assert_shape(result.shape, (6, 3))
    assert torch.equal(result, torch.cat((first, second)))
    with assert_raises(RuntimeError):
        torch.cat([first, torch.ones((4, 5))], 0)  # E: cat expects all tensor sizes
    with assert_raises(IndexError):
        torch.cat([first, first], 3)  # E: cat dimension


def test_cat_rejects_non_tensor_inputs() -> None:
    with assert_raises(TypeError):
        torch.cat("not tensors")  # E: not assignable
    with assert_raises(TypeError):
        torch.cat([1, 2])  # E: not assignable
    with assert_raises(TypeError):
        torch.cat([torch.ones(2)], dim=object())  # E: not a valid
