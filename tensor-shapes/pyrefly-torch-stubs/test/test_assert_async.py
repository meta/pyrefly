# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape


def test_assert_async_accepts_true_tensors() -> None:
    condition = torch.tensor(True)
    message = "condition must hold"
    assert_shape(condition.shape, ())
    result = torch._assert_async(condition)  # E: No attribute
    assert_type(result, None)  # E: assert_type(Unknown, None) failed
    torch._assert_async(condition, message)  # E: No attribute
    torch._assert_async(input=condition, assert_msg=message)  # E: No attribute


def test_assert_async_raises_for_false_tensors() -> None:
    condition = torch.tensor(False)
    message = "condition must hold"
    with assert_raises(RuntimeError):
        torch._assert_async(condition)  # E: No attribute
    with assert_raises(RuntimeError):
        torch._assert_async(condition, message)  # E: No attribute


def test_assert_async_rejects_invalid_arguments() -> None:
    condition = torch.tensor(True)
    with assert_raises(TypeError):
        torch._assert_async(True)  # E: No attribute
    with assert_raises(TypeError):
        torch._assert_async(condition, 3)  # E: No attribute
