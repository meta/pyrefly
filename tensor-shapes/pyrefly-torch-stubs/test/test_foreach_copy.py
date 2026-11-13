# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_raises, assert_shape


def test_foreach_copy_values_and_container_identity() -> None:
    destinations = [torch.zeros(2), torch.zeros(2)]
    sources = (torch.tensor([1.0, 2.0]), torch.tensor([3.0, 4.0]))
    first, second = destinations
    result = torch._foreach_copy_(destinations, sources)
    assert_type(result, list[torch.Tensor])
    assert result is destinations
    assert destinations[0] is first
    assert destinations[1] is second
    assert first.tolist() == [1.0, 2.0]
    assert second.tolist() == [3.0, 4.0]
    assert_shape(first.shape, (2,))
    assert_shape(second.shape, (2,))


def test_foreach_copy_tuple_and_non_blocking() -> None:
    destination = torch.zeros(2)
    source = torch.tensor([5.0, 6.0])
    destinations = (destination,)
    sources = [source]
    result = torch._foreach_copy_(destinations, sources, non_blocking=False)
    assert_type(result, tuple[torch.Tensor, ...])
    assert result is destinations
    assert destination.tolist() == [5.0, 6.0]
    assert_shape(destination.shape, (2,))


def test_foreach_copy_rejects_invalid_arguments() -> None:
    tensor = torch.zeros(2)
    with assert_raises(TypeError):
        torch._foreach_copy_([tensor], "not tensors")  # E: No matching overload
    with assert_raises(TypeError):
        torch._foreach_copy_(None, [tensor])  # E: No matching overload
    with assert_raises(TypeError):
        torch._foreach_copy_([tensor], None)  # E: No matching overload
    with assert_raises(TypeError):
        torch._foreach_copy_([tensor], [tensor], "never")  # E: No matching overload
