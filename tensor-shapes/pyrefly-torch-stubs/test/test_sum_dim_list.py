# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def _data() -> Tensor[[2, 3, 4]]:
    return torch.arange(24.0).reshape((2, 3, 4))


def test_sum_dim_list() -> None:
    data = _data()
    rows = data.sum(dim=[1, 2])
    assert_type(rows, Tensor)
    assert rows.tolist() == [66.0, 210.0]
    kept = data.sum([1, 2], True)
    assert tuple(kept.shape) == (2, 1, 1)
    columns = torch.sum(data, dim=[-1, 0])
    assert_type(columns, Tensor)
    assert columns.tolist() == [60.0, 92.0, 124.0]
    total = torch.sum(data, [], dtype=torch.float64)
    assert tuple(total.shape) == ()
    assert total.dtype == torch.float64
    assert_shape(data.sum(dim=(1, 2)).shape, (2,))


def test_sum_dim_list_rejects_invalid_dims() -> None:
    data = _data()
    assert_shape(data.sum(dim=1).shape, (2, 4))
    with assert_raises(TypeError):
        data.sum(dim=[1.5])  # E: No matching overload
    with assert_raises(RuntimeError):
        data.sum(dim=[1, 1])
    with assert_raises(IndexError):
        torch.sum(data, dim=[3])
