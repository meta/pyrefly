# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import warnings
from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape, IntTuple
from torch import Tensor


def test_cat_out_returns_output_tensor() -> None:
    top = torch.ones((2, 3))
    bottom = torch.zeros((1, 3))
    out = torch.empty((3, 3))
    result = torch.cat((top, bottom), out=out)
    assert_shape(result.shape, (3, 3))
    assert result is out
    assert out.tolist() == [[1.0] * 3, [1.0] * 3, [0.0] * 3]
    wide = torch.cat([top, top], 1, out=torch.empty((2, 6)))
    assert_shape(wide.shape, (2, 6))
    default = torch.cat((top, bottom), out=None)
    assert_shape(default.shape, (3, 3))


def test_cat_out_is_resized() -> None:
    top = torch.ones((2, 3))
    empty = torch.empty(0)
    result = torch.cat((top, top), out=empty)
    assert_shape(result.shape, (4, 3))
    assert result is empty
    assert tuple(empty.shape) == (4, 3)
    larger = torch.empty((5, 5))
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        resized = torch.cat((top, top), out=larger)
    assert_shape(resized.shape, (4, 3))
    assert all(issubclass(w.category, UserWarning) for w in caught)


def test_cat_out_with_gradual_tensor_list() -> None:
    tensors: list[Tensor] = [torch.ones((2, 3)), torch.ones((1, 3))]
    out = torch.empty((3, 3))
    result = torch.cat(tensors, out=out)
    assert_type(result, Tensor[IntTuple])
    assert tuple(result.shape) == (3, 3)
    # A list has no per-element shapes, so this mismatch is found only at runtime.
    tensors.append(torch.ones((1, 2)))
    with assert_raises(RuntimeError):
        torch.cat(tensors, out=out)


def test_cat_out_rejects_invalid_arguments() -> None:
    top = torch.ones((2, 3))
    narrow = torch.ones((2, 2))
    out = torch.empty((3, 3))
    assert_shape(torch.cat((top, top)).shape, (4, 3))
    with assert_raises(TypeError):
        torch.cat((top, top), out="x")  # E: parameter `out`
    with assert_raises(TypeError):
        torch.cat((top, top), 0, out)  # E: No matching overload
    with assert_raises(RuntimeError):
        torch.cat((top, narrow), out=out)  # E: match outside the concatenated dimension
    with assert_raises(IndexError):
        torch.cat((top, top), dim=2, out=out)  # E: cat dimension out of range
