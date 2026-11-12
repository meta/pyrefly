# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises


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
        result = torch.set_float32_matmul_precision("high")  # E: No attribute
        assert_type(result, None)  # E: assert_type(Unknown, None) failed
        assert torch.get_float32_matmul_precision() == "high"

        torch.set_float32_matmul_precision(precision="medium")  # E: No attribute
        assert torch.get_float32_matmul_precision() == "medium"

        with assert_raises(RuntimeError):
            torch.set_float32_matmul_precision(3)  # E: No attribute
    finally:
        torch.set_float32_matmul_precision(precision)  # E: No attribute
