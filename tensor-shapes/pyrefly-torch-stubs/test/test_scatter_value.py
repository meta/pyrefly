# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape


def test_scatter_scalar_value() -> None:
    tensor = torch.zeros((2, 3))
    index = torch.tensor([[0, 1, 2], [2, 0, 1]])
    filled = tensor.scatter(1, index, 5.0)
    assert_shape(filled.shape, (2, 3))
    assert torch.equal(filled, torch.full((2, 3), 5.0))
    column = torch.tensor([[0], [2]])
    partial = tensor.scatter(1, column, value=7)
    assert_shape(partial.shape, (2, 3))
    assert partial.tolist() == [[7.0, 0.0, 0.0], [0.0, 0.0, 7.0]]
    named = tensor.scatter(dim=1, index=index, value=1.5)
    assert_shape(named.shape, (2, 3))


def test_scatter_scalar_value_kinds() -> None:
    index = torch.tensor([[0], [1]])
    flags = torch.zeros((2, 2), dtype=torch.bool)
    flags = flags.scatter(1, index, True)
    assert_shape(flags.shape, (2, 2))
    assert flags.tolist() == [[True, False], [False, True]]
    complex_values = torch.zeros((2, 2), dtype=torch.complex64)
    filled = complex_values.scatter(1, index, 1 + 2j)
    assert_shape(filled.shape, (2, 2))
    assert filled[0, 0].item() == 1 + 2j


def test_scatter_tensor_source_keyword() -> None:
    tensor = torch.zeros((2, 3))
    index = torch.tensor([[0, 1, 2], [2, 0, 1]])
    result = tensor.scatter(1, index, src=torch.ones((2, 3)))
    assert_shape(result.shape, (2, 3))


def test_scatter_scalar_value_rejects_invalid_arguments() -> None:
    tensor = torch.zeros((2, 3))
    index = torch.tensor([[0, 1, 2], [2, 0, 1]])
    vector_index = torch.tensor([0, 1])
    tall_index = torch.zeros((3, 1), dtype=torch.int64)
    source = torch.ones((2, 3))
    with assert_raises(TypeError):
        tensor.scatter(1, index, src=5.0)  # E: No matching overload
    with assert_raises(TypeError):
        tensor.scatter(1, index, value=source)  # E: No matching overload
    with assert_raises(TypeError):
        tensor.scatter(1, index, "a")  # E: No matching overload
    with assert_raises(RuntimeError):
        tensor.scatter(1, vector_index, 1.0)  # E: index rank must match input rank
    with assert_raises(RuntimeError):
        tensor.scatter(1, tall_index, 1.0)  # E: index shape exceeds input shape
