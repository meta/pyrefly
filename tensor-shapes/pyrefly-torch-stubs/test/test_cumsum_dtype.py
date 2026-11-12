# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape


def test_tensor_cumsum_dtype() -> None:
    tensor = torch.ones((2, 3), dtype=torch.int32)
    result = tensor.cumsum(1, dtype=torch.float64)
    assert result.dtype == torch.float64
    assert torch.equal(result, torch.tensor([[1, 2, 3], [1, 2, 3]]))
    assert_shape(result.shape, (2, 3))
    dtype: torch.dtype = torch.int64
    result = tensor.cumsum(dim=-1, dtype=dtype)
    assert result.dtype == torch.int64
    assert_shape(result.shape, (2, 3))


def test_torch_cumsum_dtype_and_out() -> None:
    tensor = torch.ones((2, 3), dtype=torch.int32)
    result = torch.cumsum(tensor, 0, dtype=torch.float64)
    assert result.dtype == torch.float64
    assert_shape(result.shape, (2, 3))
    out = torch.empty((2, 3), dtype=torch.float64)
    tensor = tensor.to(torch.float64)
    result = torch.cumsum(tensor, dim=1, out=out)
    assert result is out
    assert torch.equal(out, torch.tensor([[1, 2, 3], [1, 2, 3]]))
    assert_shape(result.shape, (2, 3))


def test_cumsum_rejects_invalid_options() -> None:
    tensor = torch.ones((2, 3))
    with assert_raises(TypeError):
        tensor.cumsum(0, dtype="float64")  # E: parameter `dtype`
    with assert_raises(TypeError):
        torch.cumsum(tensor, 0, dtype="float64")  # E: parameter `dtype`
    with assert_raises(TypeError):
        torch.cumsum(tensor, 0, out=object())  # E: parameter `out`
    with assert_raises(TypeError):
        tensor.cumsum(0, torch.float64)  # E: Expected 1 positional argument
    with assert_raises(TypeError):
        torch.cumsum(tensor, object())  # E: parameter `dim`
