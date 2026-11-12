# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_shape
from torch import Tensor


def _first(ids: torch.LongTensor) -> Tensor:
    return ids[0]


def test_legacy_constructors() -> None:
    ids = torch.LongTensor([1, 2, 3])
    assert_type(ids, Tensor[[3]])
    assert ids.dtype == torch.int64
    assert ids.tolist() == [1, 2, 3]
    sized = torch.LongTensor(2, 3)
    assert_shape(sized.shape, (2, 3))
    assert sized.dtype == torch.int64
    flags = torch.BoolTensor((True, False))
    assert flags.dtype == torch.bool
    assert tuple(flags.shape) == (2,)
    from_size = torch.LongTensor(torch.Size([2, 3]))
    assert tuple(from_size.shape) == (2, 3)


def test_legacy_constructor_results_are_plain_tensors() -> None:
    ids = torch.LongTensor([1, 2, 3])
    assert type(ids) is torch.Tensor
    assert_shape((ids + 1).shape, (3,))
    assert_shape(ids.sum().shape, ())


def test_legacy_isinstance_checks_dtype() -> None:
    ids = torch.zeros(3, dtype=torch.int64)
    assert_shape(ids.shape, (3,))
    if isinstance(ids, torch.LongTensor):
        assert_type(ids, torch.LongTensor)
        assert _first(ids).item() == 0
    else:
        raise AssertionError("an int64 tensor is a LongTensor")
    flags = torch.zeros(3, dtype=torch.bool)
    assert isinstance(flags, torch.BoolTensor)
    assert_type(flags, torch.BoolTensor)
    assert not isinstance(torch.zeros(3), torch.LongTensor)


# Torch constructors and factories return plain `Tensor` objects, so a legacy class
# annotation accepts only values narrowed by `isinstance`.
def test_legacy_annotations_are_nominal() -> None:
    ids = torch.zeros(3, dtype=torch.int64)
    assert_shape(ids.shape, (3,))
    assert _first(ids).item() == 0  # E: parameter `ids`
    annotated: torch.LongTensor = torch.LongTensor([1])  # E: not assignable
    assert isinstance(annotated, torch.LongTensor)
