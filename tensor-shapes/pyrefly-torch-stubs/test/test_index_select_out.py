# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import warnings

import torch
from shape_extensions import assert_raises, assert_shape


def _matrix() -> torch.Tensor[[4, 3]]:
    return torch.arange(12.0).reshape((4, 3))


def test_index_select_out_returns_output_tensor() -> None:
    matrix = _matrix()
    rows = torch.tensor([0, 2])
    out = torch.empty((2, 3))
    result = torch.index_select(matrix, 0, rows, out=out)  # E: Unexpected keyword
    assert_shape(result.shape, (2, 3))
    assert result is out
    assert out.tolist() == [[0.0, 1.0, 2.0], [6.0, 7.0, 8.0]]
    default = torch.index_select(matrix, 1, rows, out=None)  # E: Unexpected keyword
    assert_shape(default.shape, (4, 2))
    assert_shape(torch.index_select(matrix, 1, rows).shape, (4, 2))


def test_index_select_out_is_resized() -> None:
    matrix = _matrix()
    rows = torch.tensor([0, 2])
    empty = torch.empty(0)
    result = torch.index_select(matrix, 1, rows, out=empty)  # E: Unexpected keyword
    assert_shape(result.shape, (4, 2))
    assert tuple(empty.shape) == (4, 2)
    larger = torch.empty((5, 5))
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        resized = torch.index_select(matrix, 0, rows, out=larger)  # E: Unexpected
    assert_shape(resized.shape, (2, 3))
    assert all(issubclass(w.category, UserWarning) for w in caught)


def test_index_select_out_rejects_invalid_arguments() -> None:
    matrix = _matrix()
    rows = torch.tensor([0, 2])
    out = torch.empty((2, 3))
    with assert_raises(TypeError):
        torch.index_select(matrix, 0, rows, out="x")  # E: Unexpected keyword
    with assert_raises(TypeError):
        torch.index_select(matrix, 0, rows, out)  # E: Expected 3 positional
    with assert_raises(IndexError):
        torch.index_select(matrix, 2, rows, out=out)  # E: out of range # E: Unexpected


def test_index_select_accepts_keyword_input() -> None:
    matrix = _matrix()
    rows = torch.tensor([0, 2])
    # E: Missing argument `self` # E: Unexpected keyword argument `input`
    selected = torch.index_select(input=matrix, dim=1, index=rows)
    assert_shape(selected.shape, (4, 2))  # E: assert_shape
    out = torch.empty((2, 3))
    result = torch.index_select(  # E: Missing argument `self`
        input=matrix,  # E: Unexpected keyword argument `input`
        dim=0,
        index=rows,
        out=out,  # E: Unexpected keyword argument `out`
    )
    assert_shape(result.shape, (2, 3))  # E: assert_shape
    assert result is out
