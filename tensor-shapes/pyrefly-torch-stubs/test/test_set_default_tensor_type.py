# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import warnings
from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape


def test_set_default_tensor_type() -> None:
    original = torch.get_default_dtype()
    try:
        # Torch warns about this deprecated function once per process.
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            result = torch.set_default_tensor_type(torch.FloatTensor)
            assert_type(result, None)
            assert all(issubclass(w.category, UserWarning) for w in caught)
            torch.set_default_tensor_type("torch.DoubleTensor")
        tensor = torch.tensor([1.5, 2.5])
        assert tensor.dtype == torch.float64
        assert_shape(tensor.shape, (2,))
    finally:
        torch.set_default_dtype(original)
    assert torch.tensor([1.5]).dtype == original


def test_set_default_tensor_type_rejects_invalid_arguments() -> None:
    original = torch.get_default_dtype()
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            with assert_raises(TypeError):
                torch.set_default_tensor_type(t=torch.FloatTensor)  # E: positional
            with assert_raises(TypeError):
                torch.set_default_tensor_type(1)  # E: parameter `t`
    finally:
        torch.set_default_dtype(original)
    assert torch.get_default_dtype() == original
