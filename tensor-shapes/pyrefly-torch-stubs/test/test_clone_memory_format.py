# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_clone_memory_format() -> None:
    tensor = torch.ones((2, 3, 4, 5))
    cloned = tensor.clone(memory_format=torch.channels_last)
    assert_type(cloned, Tensor[[2, 3, 4, 5]])
    assert_shape(cloned.shape, (2, 3, 4, 5))
    assert torch.equal(cloned, tensor)
    assert cloned is not tensor
    assert cloned.data_ptr() != tensor.data_ptr()
    assert cloned.is_contiguous(memory_format=torch.channels_last)

    format: torch.memory_format = torch.contiguous_format
    contiguous = cloned.clone(memory_format=format)
    assert_shape(contiguous.shape, (2, 3, 4, 5))
    assert contiguous.is_contiguous()
    assert torch.equal(contiguous, tensor)


def test_clone_default_format() -> None:
    tensor = torch.ones((2, 3))
    cloned = tensor.clone()
    assert_type(cloned, Tensor[[2, 3]])
    assert_shape(cloned.shape, (2, 3))
    assert cloned is not tensor
    assert torch.equal(cloned, tensor)
    explicit = tensor.clone(memory_format=None)
    assert_shape(explicit.shape, (2, 3))
    assert torch.equal(explicit, tensor)


def test_clone_rejects_invalid_options() -> None:
    tensor = torch.ones((2, 3))
    with assert_raises(TypeError):
        tensor.clone(memory_format=object())  # E: parameter `memory_format`
    with assert_raises(TypeError):
        tensor.clone(torch.contiguous_format)  # E: Expected 0 positional arguments
