# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_normal_generator_is_reproducible() -> None:
    first = torch.empty((2, 3))
    generator = torch.Generator().manual_seed(0)
    result = first.normal_(generator=generator)
    assert result is first
    assert_shape(first.shape, (2, 3))
    second = torch.empty((2, 3))
    generator = torch.Generator().manual_seed(0)
    second.normal_(generator=generator)
    assert torch.equal(first, second)


def test_normal_generator_with_mean_and_std() -> None:
    tensor = torch.empty((2, 3))
    generator = torch.Generator().manual_seed(0)
    result = tensor.normal_(1.0, 2.0, generator=generator)
    assert result is tensor
    assert_shape(tensor.shape, (2, 3))
    result = tensor.normal_(mean=0.5, std=0.1, generator=None)
    assert_shape(tensor.shape, (2, 3))
    shaped = tensor.normal_()
    assert_type(shaped, Tensor[[2, 3]])
    assert_shape(shaped.shape, (2, 3))


def test_normal_rejects_invalid_generators() -> None:
    tensor = torch.empty((2, 3))
    generator = torch.Generator()
    with assert_raises(TypeError):
        tensor.normal_(0.0, 1.0, generator)  # E: Expected 2 positional arguments
    with assert_raises(TypeError):
        tensor.normal_(generator=0)  # E: parameter `generator`
