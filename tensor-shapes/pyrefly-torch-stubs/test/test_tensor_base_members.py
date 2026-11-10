# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Any, assert_type

import numpy as np
import torch
from shape_extensions import assert_shape, IntTuple
from torch import Tensor
from torch._C import TensorBase


def test_tensor_base_members(tensor: Tensor) -> None:
    base: TensorBase = tensor
    assert_type(base, TensorBase)
    assert_type(tensor.abs_(), Tensor)
    assert_type(tensor.acos_(), Tensor)
    assert_type(tensor.fill_(0), Tensor)
    assert_type(tensor.zero_(), Tensor)
    assert_type(tensor.add_(1.0), Tensor)
    assert_type(tensor.pin_memory(), Tensor)
    assert_type(tensor.byte(), Tensor)
    assert_type(tensor.unique(), Any)
    assert_type(tensor.numpy(), np.ndarray[IntTuple])
    assert_type(tensor.data_ptr(), int)
    assert_type(tensor.is_contiguous(), bool)
    assert_type(tensor.grad, Any)
    assert_type(tensor.is_cuda, bool)
    assert_type(float(torch.zeros(())), float)
    assert_type(int(torch.zeros(())), int)
    assert_type(len(tensor), int)


def test_scalar_formatting_runtime() -> None:
    scalar: Tensor[[]] = torch.ones(())
    formatted = f"{scalar:.3e}"
    assert_type(formatted, str)
    assert formatted == "1.000e+00"
    assert_shape(scalar.shape, ())


def test_shape_preserving_tensor_base_members() -> None:
    tensor = torch.ones((2, 3))
    assert_shape(tensor.fill_(0).shape, (2, 3))
    assert_shape(tensor.clone().shape, (2, 3))
    assert_shape(tensor.detach().shape, (2, 3))
    assert_shape(tensor.transpose(0, 1).contiguous().shape, (3, 2))


def test_memory_format_constants() -> None:
    assert_type(torch.contiguous_format, torch.memory_format)
    assert_type(torch.preserve_format, torch.memory_format)
    assert_type(torch.channels_last_3d, torch.memory_format)
    assert isinstance(torch.contiguous_format, torch.memory_format)
    assert isinstance(torch.preserve_format, torch.memory_format)
    assert isinstance(torch.channels_last_3d, torch.memory_format)


def test_torch_module_types(value: torch.FloatTensor) -> None:
    assert_type(torch.channels_last, torch.memory_format)
    assert_type(torch.uint8, Any)
    assert_type(value, Tensor)


def check_tensor_new_empty_size_tuple(tensor: Tensor) -> None:
    # Torch accepts a size tuple as well as varargs.
    assert_type(tensor.new_empty(0, 3), Tensor)
    assert_type(tensor.new_empty((0, 3)), Tensor[[0, 3]])
    assert_type(tensor.new_empty((0,)), Tensor[[0]])


def test_tensor_argsort_and_logit() -> None:
    source = torch.tensor([[3.0, 1.0, 3.0], [2.0, 0.0, 0.0]])
    indices = source.argsort(dim=1, descending=True, stable=True)
    assert_shape(indices.shape, (2, 3))
    assert torch.equal(
        indices, torch.argsort(source, dim=1, descending=True, stable=True)
    )
    probabilities = torch.full((2, 3), 0.5)
    logits = probabilities.logit(eps=0.1)
    assert_shape(logits.shape, (2, 3))
    assert torch.equal(logits, torch.zeros_like(logits))


def check_tensor_add_alpha(tensor: Tensor) -> None:
    # `alpha` scales the added tensor in the real in-place signature.
    assert_type(tensor.add_(tensor, alpha=0.5), Tensor)


def test_new_empty_and_add_alpha_runtime() -> None:
    base = torch.ones(2, 3)

    assert tuple(base.new_empty((0, 3)).shape) == (0, 3)
    assert tuple(base.new_empty(2, 3).shape) == (2, 3)
    assert torch.equal(base.add_(base, alpha=0.5), torch.full((2, 3), 1.5))
    assert_shape(base.shape, (2, 3))
