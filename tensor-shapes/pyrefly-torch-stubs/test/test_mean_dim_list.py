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


def test_mean_dim_list() -> None:
    data = _data()
    rows = data.mean(dim=[1, 2])
    assert_type(rows, Tensor)
    assert rows.tolist() == [5.5, 17.5]
    kept = torch.mean(data, [-1, 0], keepdim=True)
    assert_type(kept, Tensor)
    assert tuple(kept.shape) == (1, 3, 1)
    total = data.mean([], dtype=torch.float64)
    assert tuple(total.shape) == ()
    assert total.dtype == torch.float64
    assert_shape(data.mean(dim=(1, 2)).shape, (2,))


def test_mean_dim_list_rejects_invalid_dims() -> None:
    data = _data()
    assert_shape(data.mean(dim=1).shape, (2, 4))
    with assert_raises(TypeError):
        data.mean(dim=[1.5])  # E: No matching overload
    with assert_raises(RuntimeError):
        torch.mean(data, dim=[1, 1])
    with assert_raises(IndexError):
        data.mean(dim=[3])
