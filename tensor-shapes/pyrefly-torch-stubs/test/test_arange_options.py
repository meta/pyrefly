# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Any, assert_type, TYPE_CHECKING

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def test_arange_float_dtype_and_grad() -> None:
    result = torch.arange(8, dtype=torch.float32, requires_grad=True)
    assert_type(result, Tensor[[8]])
    assert result.dtype == torch.float32 and result.requires_grad
    assert result.tolist() == [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
    result.sum().backward()
    assert torch.equal(result.grad, torch.ones(8))
    assert_shape(result.shape, (8,))


def test_arange_integer_precision_and_out() -> None:
    assert_shape(torch.arange(8).shape, (8,))
    assert_shape(torch.arange(2, 8).shape, (6,))
    assert_shape(torch.arange(1, 8, 2).shape, (4,))
    output = torch.empty(0, dtype=torch.int64)
    result = torch.arange(2, 8, out=output)
    assert_type(result, Tensor[[6]])
    assert result.tolist() == [2, 3, 4, 5, 6, 7]
    assert_shape(result.shape, (6,))
    assert result is output
    if not TYPE_CHECKING:
        # The output is resized; aliases are not given a new static shape here.
        assert_shape(output.shape, (6,))


def test_arange_layout_device_and_false_pinning() -> None:
    result = torch.arange(
        1,
        8,
        2,
        dtype=torch.float32,
        layout=torch.strided,
        device=torch.device("cpu"),
        pin_memory=False,
        requires_grad=False,
    )
    assert_type(result, Tensor[[4]])
    assert result.tolist() == [1.0, 3.0, 5.0, 7.0]
    assert result.device.type == "cpu" and not result.requires_grad
    assert_shape(result.shape, (4,))
    optional = torch.arange(3, pin_memory=None, requires_grad=None)
    assert_shape(optional.shape, (3,))


def real_range(start: float, end: float, step: float) -> Tensor[[int]]:
    result = torch.arange(start, end, step)
    assert_type(result, Tensor[[int]])
    return result


def check_any_options(dtype: Any, device: Any, flag: Any) -> None:
    result = torch.arange(5, dtype=dtype, device=device, requires_grad=flag)
    assert_type(result, Tensor[[5]])


def test_arange_real_bounds_keep_rank_one() -> None:
    result = real_range(1.0, 2.5, 0.5)
    assert result.tolist() == [1.0, 1.5, 2.0]
    assert_shape(result.shape, (int,), runtime=(3,))
    check_any_options(torch.float32, "cpu", False)


def test_arange_rejects_invalid_options_and_bounds() -> None:
    with assert_raises(TypeError):
        torch.arange(3, dtype="float32")  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange(3, device=object())  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange(3, requires_grad="yes")  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange(3, out="bad")  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange(3, layout="strided")  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange("3")  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange(None)  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange()  # E: No matching overload
    with assert_raises(TypeError):
        torch.arange(0, 3, 1, 2)  # E: No matching overload
    with assert_raises(RuntimeError):
        torch.arange(0, 3, 0)  # E: arange step
    # Complex endpoints are unsupported by native arange, despite generated Number.
    with assert_raises(RuntimeError):
        torch.arange(2 + 1j)  # E: No matching overload
