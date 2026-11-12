# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape


def test_deterministic_algorithm_controls() -> None:
    mode = torch.are_deterministic_algorithms_enabled()
    assert_type(mode, bool)
    warn_only = torch.is_deterministic_algorithms_warn_only_enabled()
    assert_type(warn_only, bool)
    try:
        torch.use_deterministic_algorithms(True, warn_only=True)
        assert torch.are_deterministic_algorithms_enabled()
        assert torch.is_deterministic_algorithms_warn_only_enabled()

        result = torch.use_deterministic_algorithms(False)
        assert_type(result, None)
        assert not torch.are_deterministic_algorithms_enabled()
        assert not torch.is_deterministic_algorithms_warn_only_enabled()

        with assert_raises(TypeError):
            torch.use_deterministic_algorithms(True, True)  # E: positional
    finally:
        torch.use_deterministic_algorithms(mode, warn_only=warn_only)


def test_float32_matmul_precision_controls() -> None:
    precision = torch.get_float32_matmul_precision()
    assert_type(precision, str)
    try:
        result = torch.set_float32_matmul_precision("high")
        assert_type(result, None)
        assert torch.get_float32_matmul_precision() == "high"

        torch.set_float32_matmul_precision(precision="medium")
        assert torch.get_float32_matmul_precision() == "medium"

        with assert_raises(RuntimeError):
            torch.set_float32_matmul_precision(3)  # E: parameter `precision`
    finally:
        torch.set_float32_matmul_precision(precision)


def test_default_device_controls() -> None:
    original = torch.get_default_device()
    assert_type(original, torch.device)
    try:
        result = torch.set_default_device("meta")
        assert_type(result, None)
        assert torch.get_default_device() == torch.device("meta")
        tensor = torch.empty((2, 3))
        assert tensor.device == torch.device("meta")
        assert_shape(tensor.shape, (2, 3))

        torch.set_default_device(device=torch.device("cpu"))
        assert torch.get_default_device() == torch.device("cpu")
        torch.set_default_device(None)
        assert torch.empty(1).device == torch.device("cpu")

        with assert_raises(TypeError):
            torch.set_default_device(object())  # E: parameter `device`
        with assert_raises(TypeError):
            torch.set_default_device("cpu", "meta")  # E: Expected 1 positional argument
    finally:
        cpu = torch.device("cpu")
        torch.set_default_device(None if original == cpu else original)
