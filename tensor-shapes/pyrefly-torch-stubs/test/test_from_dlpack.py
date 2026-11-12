# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type, TYPE_CHECKING

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor
from torch.utils.dlpack import to_dlpack


class CpuProducer:
    def __init__(self, tensor: Tensor) -> None:
        self.tensor = tensor

    def __dlpack__(self, stream: None = None) -> object:
        return to_dlpack(self.tensor)

    def __dlpack_device__(self) -> tuple[int, int]:
        return (1, 0)


class IncompleteProducer:
    def __dlpack__(self) -> object:
        return object()


def test_from_dlpack_tensor_shares_cpu_memory() -> None:
    source = torch.tensor([[1.0, 2.0], [3.0, 4.0]])
    result = torch.from_dlpack(source, copy=False, device="cpu")
    assert_type(result, Tensor)
    assert isinstance(result, Tensor)
    assert result is not source
    assert result.data_ptr() == source.data_ptr()
    result[0, 0] = 9.0
    assert source[0, 0].item() == 9.0
    if not TYPE_CHECKING:
        # The canonical export returns a gradual Tensor, without shape propagation.
        assert_shape(result.shape, (2, 2))


def test_from_dlpack_capsule_is_consumed_once() -> None:
    source = torch.tensor([1.0, 2.0])
    capsule = to_dlpack(source)
    result = torch.from_dlpack(capsule)
    assert_type(result, Tensor)
    assert result.data_ptr() == source.data_ptr()
    result[1] = 7.0
    assert source[1].item() == 7.0
    if not TYPE_CHECKING:
        assert_shape(result.shape, (2,))


def test_from_dlpack_cpu_protocol_and_copy() -> None:
    source = torch.tensor([1.0, 2.0])
    producer = CpuProducer(source)
    shared = torch.from_dlpack(producer, device=torch.device("cpu"), copy=False)
    copied = torch.from_dlpack(producer, copy=True)
    assert_type(shared, Tensor)
    assert_type(copied, Tensor)
    assert shared.data_ptr() == source.data_ptr()
    assert copied.data_ptr() != source.data_ptr()
    copied[0] = 11.0
    assert source[0].item() == 1.0
    if not TYPE_CHECKING:
        assert_shape(shared.shape, (2,))
        assert_shape(copied.shape, (2,))


def test_from_dlpack_rejects_invalid_inputs_and_arity() -> None:
    # Opaque capsules/producers are typed Any upstream; validation is runtime-only.
    with assert_raises(RuntimeError):
        torch.from_dlpack("not a capsule")
    with assert_raises(AttributeError):
        torch.from_dlpack(IncompleteProducer())
    tensor = torch.ones(2)
    with assert_raises(TypeError):
        torch.from_dlpack(tensor, False)  # E: Expected 1 positional
    with assert_raises(AssertionError):
        torch.from_dlpack(tensor, device=object())  # E: parameter `device`
    if TYPE_CHECKING:
        torch.from_dlpack(tensor, copy="never")  # E: parameter `copy`
