# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import warnings

import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor


def _matrix() -> Tensor[[2, 3]]:
    return torch.arange(6.0).reshape((2, 3))


def test_norm_frobenius() -> None:
    matrix = _matrix()
    total = torch.norm(matrix, p="fro")
    assert_shape(total.shape, ())
    assert abs(total.item() - 55**0.5) < 1e-5
    positional = torch.norm(matrix, "fro")
    assert_shape(positional.shape, ())
    rows = torch.norm(matrix, p="fro", dim=1)
    assert_shape(rows.shape, (2,))
    assert torch.allclose(rows, torch.tensor([5**0.5, 50**0.5]))
    kept = torch.norm(matrix, p="fro", dim=1, keepdim=True)
    assert_shape(kept.shape, (2, 1))
    method = matrix.norm(p="fro")
    assert_shape(method.shape, ())
    method_rows = matrix.norm("fro", 1)
    assert_shape(method_rows.shape, (2,))


def test_norm_frobenius_over_two_dimensions() -> None:
    cube = torch.ones((2, 3, 4))
    kept = cube.norm(p="fro", dim=(1, 2), keepdim=True)
    assert_shape(kept.shape, (2, 1, 1))
    assert torch.allclose(kept, torch.full((2, 1, 1), 12**0.5))


def test_norm_numeric_orders() -> None:
    matrix = _matrix()
    default = torch.norm(matrix)
    frobenius = torch.norm(matrix, p="fro")
    assert torch.equal(default, frobenius)
    assert_shape(torch.norm(matrix, p=2).shape, ())
    assert_shape(matrix.norm(p=float("inf"), dim=0).shape, (3,))
    assert torch.norm(matrix, p=float("inf")).item() == 5.0


def test_norm_rejects_unsupported_orders() -> None:
    matrix = _matrix()
    assert_shape(torch.norm(matrix).shape, ())
    with assert_raises(RuntimeError):
        torch.norm(matrix, p="bad")  # E: parameter `p`
    with assert_raises(TypeError):
        torch.norm(matrix, p=[1])  # E: parameter `p`
    # Torch accepts the nuclear norm, which the overlay does not declare.
    assert torch.norm(matrix, p="nuc").item() > 0  # E: parameter `p`
    # Torch requires exactly two dimensions for the Frobenius norm with `dim`; the
    # reduced-shape rule does not check this.
    cube = torch.ones((2, 3, 4))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        with assert_raises(RuntimeError):
            torch.norm(cube, p="fro", dim=(0, 1, 2))
