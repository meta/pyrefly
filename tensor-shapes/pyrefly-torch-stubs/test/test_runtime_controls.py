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
    warn_only = torch.is_deterministic_algorithms_warn_only_enabled()  # E: No attribute
    assert_type(warn_only, bool)  # E: assert_type
    try:
        torch.use_deterministic_algorithms(True, warn_only=True)  # E: No attribute
        assert torch.are_deterministic_algorithms_enabled()
        assert torch.is_deterministic_algorithms_warn_only_enabled()  # E: No attribute

        result = torch.use_deterministic_algorithms(False)  # E: No attribute
        assert_type(result, None)  # E: assert_type
        assert not torch.are_deterministic_algorithms_enabled()
        assert not torch.is_deterministic_algorithms_warn_only_enabled()  # E: attribute

        with assert_raises(TypeError):
            torch.use_deterministic_algorithms(True, True)  # E: No attribute
    finally:
        torch.use_deterministic_algorithms(mode, warn_only=warn_only)  # E: No attribute
