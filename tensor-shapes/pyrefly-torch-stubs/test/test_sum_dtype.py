# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape


def test_tensor_sum_dtype() -> None:
    tensor = torch.ones((2, 3))
    total = tensor.sum(dtype=torch.float64)
    assert total.dtype == torch.float64
    assert_shape(total.shape, ())
    columns = tensor.sum(0, dtype=torch.int64)
    assert columns.dtype == torch.int64
    assert_shape(columns.shape, (3,))
    dtype: torch.dtype = torch.float64
    rows = tensor.sum(dim=1, keepdim=True, dtype=dtype)
    assert rows.dtype == torch.float64
    assert_shape(rows.shape, (2, 1))


def test_torch_sum_dtype() -> None:
    tensor = torch.ones((2, 3))
    total = torch.sum(tensor, dtype=torch.float64)
    assert total.dtype == torch.float64
    assert_shape(total.shape, ())
    columns = torch.sum(tensor, 0, dtype=torch.int64)
    assert columns.dtype == torch.int64
    assert_shape(columns.shape, (3,))


def test_sum_rejects_invalid_dtype() -> None:
    tensor = torch.ones((2, 3))
    with assert_raises(TypeError):
        tensor.sum(dtype="float64")  # E: No matching overload
    with assert_raises(TypeError):
        torch.sum(tensor, dtype="float64")  # E: No matching overload
    with assert_raises(TypeError):
        tensor.sum(0, False, torch.float64)  # E: No matching overload
