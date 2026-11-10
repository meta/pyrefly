# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape


def test_full_like_options() -> None:
    tensor = torch.ones((2, 3))
    result = torch.full_like(tensor, 2, dtype=torch.int64)
    assert result.dtype == torch.int64
    assert_shape(result.shape, (2, 3))
    result = torch.full_like(tensor, 2, device="cpu")
    assert result.device == torch.device("cpu")
    assert_shape(result.shape, (2, 3))
    result = torch.full_like(tensor, 2, requires_grad=True)
    assert result.requires_grad
    assert_shape(result.shape, (2, 3))
    with assert_raises(TypeError):
        torch.full_like(tensor, 2, device=object())  # E: parameter `device`
    complex_tensor = torch.zeros((2, 3), dtype=torch.complex64)
    result = torch.full_like(complex_tensor, 1 + 2j)
    assert result.dtype == torch.complex64
    assert_shape(result.shape, (2, 3))


def test_empty_like_options() -> None:
    tensor = torch.ones((2, 3))
    result = torch.empty_like(tensor, device="cpu")
    assert result.device == torch.device("cpu")
    assert_shape(result.shape, (2, 3))
    result = torch.empty_like(tensor, pin_memory=False)
    assert_shape(result.shape, (2, 3))
    result = torch.empty_like(tensor, layout=torch.strided)
    assert result.layout == torch.strided
    assert_shape(result.shape, (2, 3))
    image = torch.ones((2, 3, 4, 5))
    fmt = torch.channels_last
    result = torch.empty_like(image, memory_format=fmt)
    assert result.is_contiguous(memory_format=torch.channels_last)
    assert_shape(result.shape, (2, 3, 4, 5))
    with assert_raises(TypeError):
        torch.empty_like(tensor, torch.float64)  # E: Expected 1 positional argument
    with assert_raises(TypeError):
        torch.empty_like(tensor, device=object())  # E: parameter `device`


def test_randn_like_options() -> None:
    tensor = torch.ones((2, 3))
    dtype: torch.dtype = torch.float64
    result = torch.randn_like(tensor, dtype=dtype)
    assert result.dtype == torch.float64
    assert_shape(result.shape, (2, 3))
    generator = torch.Generator().manual_seed(0)
    first = torch.randn_like(tensor, generator=generator)
    generator = torch.Generator().manual_seed(0)
    second = torch.randn_like(tensor, generator=generator)
    assert torch.equal(first, second)
    assert_shape(first.shape, (2, 3))
    with assert_raises(TypeError):
        torch.randn_like(tensor, device=object())  # E: parameter `device`
