# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import warnings

import torch
from shape_extensions import assert_raises, assert_shape


def test_minimum_out_returns_output_tensor() -> None:
    left = torch.ones((2, 3))
    right = torch.tensor([0.0, 1.0, 2.0])
    out = torch.empty((2, 3))
    result = torch.minimum(left, right, out=out)
    assert_shape(result.shape, (2, 3))
    assert result is out
    assert out.tolist() == [[0.0, 1.0, 1.0], [0.0, 1.0, 1.0]]
    default = torch.minimum(left, right, out=None)
    assert_shape(default.shape, (2, 3))
    assert_shape(torch.minimum(left, right).shape, (2, 3))


def test_minimum_out_is_resized() -> None:
    left = torch.ones((2, 3))
    empty = torch.empty(0)
    result = torch.minimum(left, left, out=empty)
    assert_shape(result.shape, (2, 3))
    assert tuple(empty.shape) == (2, 3)
    larger = torch.empty((4, 4))
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        resized = torch.minimum(left, left, out=larger)
    assert_shape(resized.shape, (2, 3))
    assert all(issubclass(w.category, UserWarning) for w in caught)


def test_minimum_out_rejects_invalid_arguments() -> None:
    left = torch.ones((2, 3))
    wrong = torch.ones(4)
    out = torch.empty((2, 3))
    with assert_raises(TypeError):
        torch.minimum(left, left, out="x")  # E: parameter `out`
    with assert_raises(TypeError):
        torch.minimum(left, left, out)  # E: Expected 2 positional arguments
    with assert_raises(TypeError):
        torch.minimum(left, 2.0, out=out)  # E: parameter `other`
    with assert_raises(RuntimeError):
        torch.minimum(left, wrong, out=out)  # E: broadcast
