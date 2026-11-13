# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import torch
from shape_extensions import assert_raises, assert_shape


def test_mm_output_dtype_on_meta() -> None:
    left = torch.ones((2, 3), device="meta", dtype=torch.float16)
    right = torch.ones((3, 4), device="meta", dtype=torch.float16)
    result = torch.mm(left, right, out_dtype=torch.float32)
    assert_shape(result.shape, (2, 4))
    assert result.dtype == torch.float32
    assert result.device == torch.device("meta")
    positional = torch.mm(left, right, torch.float32)
    assert_shape(positional.shape, (2, 4))
    assert positional.dtype == torch.float32


def test_mm_output_buffer() -> None:
    left = torch.ones((2, 3))
    right = torch.ones((3, 4))
    ordinary = torch.mm(left, right)
    assert_shape(ordinary.shape, (2, 4))
    assert torch.equal(ordinary, torch.full((2, 4), 3.0))
    out = torch.empty((2, 4))
    result = torch.mm(input=left, mat2=right, out=out)
    assert result is out
    assert_shape(result.shape, (2, 4))
    assert torch.equal(result, ordinary)


def test_mm_rejects_invalid_output_dtype() -> None:
    left = torch.ones((2, 3))
    right = torch.ones((3, 4))
    with assert_raises(TypeError):
        torch.mm(left, right, out_dtype=None)  # E: No matching overload
    with assert_raises(TypeError):
        torch.mm(left, right, out_dtype="float32")  # E: No matching overload
    with assert_raises(TypeError):
        torch.mm(left, right, out=object())  # E: parameter `out`
