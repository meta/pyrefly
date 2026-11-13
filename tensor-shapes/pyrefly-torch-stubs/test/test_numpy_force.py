# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import numpy as np
import torch
from shape_extensions import assert_raises, assert_shape


def test_numpy_force_converts_tensor_requiring_grad() -> None:
    tensor = torch.ones((2, 3), requires_grad=True)
    array = tensor.numpy(force=True)
    assert_type(array, np.ndarray[[2, 3]])
    assert_shape(array.shape, (2, 3))
    assert array.dtype == np.float32
    with assert_raises(RuntimeError):
        tensor.numpy()


def test_numpy_without_force_shares_memory() -> None:
    tensor = torch.zeros((2, 3))
    array = tensor.numpy(force=False)
    assert_type(array, np.ndarray[[2, 3]])
    array[0, 0] = 5.0
    assert tensor[0, 0].item() == 5.0
    default = tensor.numpy()
    assert_type(default, np.ndarray[[2, 3]])
    assert_shape(default.shape, (2, 3))


def test_numpy_force_rejects_invalid_arguments() -> None:
    tensor = torch.zeros((2, 3))
    assert_shape(tensor.numpy().shape, (2, 3))
    with assert_raises(TypeError):
        tensor.numpy(True)  # E: Expected 0 positional arguments
    with assert_raises(TypeError):
        tensor.numpy(force="yes")  # E: parameter `force`
    with assert_raises(TypeError):
        tensor.numpy(force=1)  # E: parameter `force`
