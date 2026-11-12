# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type, TYPE_CHECKING

import torch
import torch.nn.functional as F
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def resize_list(image: Tensor, size: list[int]) -> Tensor:
    result = F.interpolate(image, size=size, mode="nearest")
    assert_type(result, Tensor)
    return result


def test_interpolate_integer_lists() -> None:
    image = torch.tensor([[[[1.0, 2.0], [3.0, 4.0]]]])
    result = resize_list(image, [4, 4])
    assert torch.equal(result, image.repeat_interleave(2, 2).repeat_interleave(2, 3))
    if not TYPE_CHECKING:
        assert_shape(result.shape, (1, 1, 4, 4))
    signal = torch.ones((2, 3, 8))
    resized = F.interpolate(signal, [4])
    assert_type(resized, Tensor)
    assert torch.equal(resized, torch.ones((2, 3, 4)))
    if not TYPE_CHECKING:
        assert_shape(resized.shape, (2, 3, 4))


def test_interpolate_keeps_tuple_size_precision() -> None:
    image = torch.ones((2, 3, 8, 8))
    result = F.interpolate(image, (4, 6), mode="nearest")
    assert_type(result, Tensor[[2, 3, 4, 6]])
    assert_shape(result.shape, (2, 3, 4, 6))
    with assert_raises(ValueError):
        F.interpolate(image, (4,))  # E: size must match the spatial rank


def test_interpolate_rejects_bad_list_elements_and_options() -> None:
    image = torch.ones((1, 1, 2, 2))
    with assert_raises(TypeError):
        F.interpolate(image, [2, 2.0])  # E: No matching overload
    with assert_raises(TypeError):
        F.interpolate(image, [2, "2"])  # E: No matching overload
    with assert_raises(TypeError):
        F.interpolate(image, "wrong")  # E: No matching overload
    with assert_raises(TypeError):
        F.interpolate(image, [4, 4], unknown=True)  # E: No matching overload
    if not TYPE_CHECKING:
        with assert_raises(ValueError):
            F.interpolate(image, [4])
