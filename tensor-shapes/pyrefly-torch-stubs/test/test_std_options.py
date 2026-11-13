# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def _data() -> Tensor[[2, 3]]:
    return torch.tensor([[1.0, 2.0, 3.0], [4.0, 6.0, 8.0]])


def test_tensor_std_unbiased() -> None:
    tensor = _data()
    total = tensor.std(unbiased=False)
    assert_shape(total.shape, ())
    assert torch.allclose(total, torch.tensor(2.3805), atol=1e-4)
    total = tensor.std(False)
    assert_shape(total.shape, ())
    assert torch.allclose(total, torch.tensor(2.3805), atol=1e-4)
    rows = tensor.std(dim=1, unbiased=False)
    assert_shape(rows.shape, (2,))
    assert torch.allclose(rows, torch.tensor([0.8165, 1.6330]), atol=1e-4)
    columns = tensor.std(0, False)
    assert_shape(columns.shape, (3,))
    assert torch.allclose(columns, torch.tensor([1.5, 2.0, 2.5]))
    columns = tensor.std(0, True)
    assert_shape(columns.shape, (3,))
    kept = tensor.std(0, False, True)
    assert_shape(kept.shape, (1, 3))
    assert torch.allclose(kept, torch.tensor([[1.5, 2.0, 2.5]]))


def test_tensor_std_correction() -> None:
    tensor = _data()
    rows = tensor.std(dim=1, correction=0)
    assert_shape(rows.shape, (2,))
    assert torch.allclose(rows, torch.tensor([0.8165, 1.6330]), atol=1e-4)
    total = tensor.std(correction=0)
    assert_shape(total.shape, ())
    kept = tensor.std(1, keepdim=True)
    assert_shape(kept.shape, (2, 1))
    assert torch.allclose(kept, torch.tensor([[1.0], [2.0]]))
    assert_shape(tensor.std().shape, ())


def test_torch_std_options() -> None:
    tensor = _data()
    columns = torch.std(tensor, 0, False)
    assert_shape(columns.shape, (3,))
    assert torch.allclose(columns, torch.tensor([1.5, 2.0, 2.5]))
    total = torch.std(tensor, unbiased=False)
    assert_shape(total.shape, ())
    kept = torch.std(tensor, dim=1, correction=0, keepdim=True)
    assert_shape(kept.shape, (2, 1))


def test_std_rejects_invalid_combinations() -> None:
    tensor = _data()
    with assert_raises(TypeError):
        tensor.std(unbiased=False, keepdim=True)  # E: No matching overload
    with assert_raises(TypeError):
        tensor.std(1, unbiased=False, correction=0)  # E: No matching overload
    with assert_raises(TypeError):
        tensor.std(1, correction="biased")  # E: No matching overload
