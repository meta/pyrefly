# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import assert_type, Literal, TYPE_CHECKING

import torch
from shape_extensions import (
    assert_raises,
    assert_shape,
    Int,
    IntTuple,
    IntVar,
)
from torch import Tensor


def test_zeros() -> None:
    assert_shape(torch.zeros((2, 3)).shape, (2, 3))
    with assert_raises(TypeError):
        torch.zeros("invalid")  # E: No matching overload


def test_empty_accepts_requires_grad() -> None:
    sized = torch.empty((0, 2), requires_grad=False)
    assert_shape(sized.shape, (0, 2))
    assert not sized.requires_grad
    separate = torch.empty(2, 3, dtype=torch.float32, device="cpu", requires_grad=True)
    assert_shape(separate.shape, (2, 3))
    assert separate.requires_grad
    assert_shape(torch.empty((), requires_grad=True).shape, ())


def test_tensor_data_constructors() -> None:
    assert_shape(torch.tensor(1).shape, ())
    assert_shape(torch.tensor([1, 2, 3]).shape, (3,))
    assert_shape(torch.tensor([[1, 2], [3, 4]]).shape, (2, 2))
    assert_shape(torch.tensor([[[1]], [[2]]]).shape, (2, 1, 1))
    assert_shape(torch.tensor([[[[1]]]]).shape, (1, 1, 1, 1))
    assert_shape(torch.tensor([[], []]).shape, (2, 0))
    with assert_raises(ValueError):
        torch.tensor([[1], [2, 3]])
    assert_shape(torch.Tensor().shape, (0,))
    assert_shape(torch.Tensor(2, 3).shape, (2, 3))


def test_creation_signatures_reject_invalid_arguments() -> None:
    tensor = torch.zeros((2, 3))

    with assert_raises(TypeError):
        torch.tensor([1, 2], torch.float32)  # E: Expected at most 1 positional argument
    with assert_raises(TypeError):
        torch.Tensor([1, 2], "cpu")  # E: Unpacked argument
    with assert_raises(TypeError):
        torch.full((2, 2), 0.0, torch.float32)  # E: Expected at most 2 positional
    with assert_raises(TypeError):
        torch.rand((2, 2), torch.float32)  # E: Unpacked argument
    with assert_raises(TypeError):
        torch.empty((2, 2), torch.float32)  # E: Unpacked argument
    with assert_raises(TypeError):
        tensor.new_zeros()  # E: No matching overload found
    with assert_raises(TypeError):
        tensor.new_zeros((2, 3), 4)  # E: not assignable


def test_size_factories() -> None:
    assert_shape(torch.zeros(2, 3).shape, (2, 3))
    assert_shape(torch.ones((2, 3)).shape, (2, 3))
    assert_shape(torch.empty((2, 0, 3)).shape, (2, 0, 3))
    assert_shape(torch.full((2, 3), 1.0).shape, (2, 3))
    assert_shape(torch.rand(2, 3, dtype=None).shape, (2, 3))
    assert_shape(torch.randn(()).shape, ())
    with assert_raises(RuntimeError):
        # TODO: BUG: Reject negative literal dimensions statically.
        torch.zeros((-1, 2))


def test_generator_and_tensor_factories() -> None:
    generator = torch.Generator()
    assert_shape(torch.rand((2, 3), generator=generator).shape, (2, 3))
    assert_shape(torch.randint(10, (1,)).shape, (1,))
    assert_shape(torch.randint(0, 10, (3,), generator=generator).shape, (3,))

    tensor = torch.zeros((2, 3))
    # Dynamic tuple concatenation stays gradual, unlike literal tuple sizes.
    assert_shape(
        tensor.new_zeros(tensor.shape + (4,)).shape,
        IntTuple,
        runtime=(2, 3, 4),
    )
    assert_shape(tensor.new_ones((4, 5)).shape, (4, 5))
    assert_shape(torch.as_tensor([1.0, 2.0]).shape, IntTuple, runtime=(2,))
    assert_shape(torch.from_numpy(tensor.numpy()).shape, (2, 3))

    # `randperm` preserves rank, but its value-dependent extent is gradual.
    assert_shape(torch.randperm(5, generator=generator).shape, IntTuple, runtime=(5,))


def test_new_factories_accept_list_sizes() -> None:
    tensor = torch.zeros((2, 3))
    zeros = tensor.new_zeros([4, 5])
    ones = tensor.new_ones([4, 5])
    empty = tensor.new_empty([4, 5])
    full = tensor.new_full([4, 5], 1.0)
    assert_shape(zeros.shape, IntTuple, runtime=(4, 5))
    assert_shape(ones.shape, IntTuple, runtime=(4, 5))
    assert_shape(empty.shape, IntTuple, runtime=(4, 5))
    assert_shape(full.shape, IntTuple, runtime=(4, 5))


def test_tuple_factory_shapes() -> None:
    tensor = torch.zeros((2, 3))
    assert_shape(tensor.new_zeros((2, 3, 4)).shape, (2, 3, 4))
    assert_shape(tensor.new_ones((0, 3)).shape, (0, 3))
    assert_shape(tensor.new_empty(()).shape, ())
    assert_shape(tensor.new_full((2, 3), 1.0).shape, (2, 3))


def test_full_like_options() -> None:
    tensor = torch.zeros((2, 3))
    typed = torch.full_like(tensor, 1.0, dtype=torch.float64)
    assert_shape(typed.shape, (2, 3))
    assert typed.dtype == torch.float64
    configured = torch.full_like(
        tensor,
        2.0,
        device="cpu",
        layout=torch.strided,
        requires_grad=True,
        memory_format=torch.preserve_format,
    )
    assert_shape(configured.shape, (2, 3))
    assert configured.requires_grad


def test_triangular_indices_options() -> None:
    lower = torch.tril_indices(
        3,
        4,
        dtype=torch.int64,
        device="cpu",
        layout=torch.strided,
    )
    upper = torch.triu_indices(
        3,
        4,
        1,
        dtype=torch.int64,
        device="cpu",
        layout=torch.strided,
    )
    assert_shape(lower.shape, (2, int), runtime=(2, 6))
    assert_shape(upper.shape, (2, int), runtime=(2, 6))


def test_arange() -> None:
    assert_shape(torch.arange(5).shape, (5,))
    assert_shape(torch.arange(2, 7).shape, (5,))
    assert_shape(torch.arange(0, 5, 2).shape, (3,))
    assert_shape(torch.arange(5, 0, -2).shape, (3,))
    assert_shape(torch.arange(4, 4, 2).shape, (0,))
    assert_shape(torch.arange(4, 4, -2).shape, (0,))
    assert_shape(torch.arange(0, 6, 2, dtype=None, device=None).shape, (3,))

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: arange step must be nonzero
        torch.arange(0, 5, 0)

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: arange step must be nonzero
        torch.arange(-9223372036854775808, 9223372036854775807, 0)

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: arange bounds are inconsistent with step
        torch.arange(-1)

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: arange bounds are inconsistent with step
        torch.arange(5, 0, 1)

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: arange bounds are inconsistent with step
        torch.arange(0, 5, -1)


def test_spaced_ranges() -> None:
    assert_shape(torch.linspace(0, 1, 5).shape, (5,))
    assert_shape(torch.linspace(0, 1, steps=0).shape, (0,))
    assert_shape(torch.logspace(0, 1, 5).shape, (5,))

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: extent must be non-negative
        torch.linspace(0, 1, -1)

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: extent must be non-negative
        torch.logspace(0, 1, -1)


def test_linspace_factory_kwargs() -> None:
    out = torch.empty(5)
    grad = torch.linspace(0, 1, 5, requires_grad=True)
    strided = torch.linspace(0, 1, 5, layout=torch.strided)
    pinned = torch.linspace(0, 1, 5, pin_memory=False)
    written = torch.linspace(0, 1, 5, out=out)
    assert_shape(grad.shape, (5,))
    assert_shape(strided.shape, (5,))
    assert_shape(pinned.shape, (5,))
    assert_shape(written.shape, (5,))
    assert grad.requires_grad
    assert written is out


def test_eye() -> None:
    assert_shape(torch.eye(3).shape, (3, 3))
    assert_shape(torch.eye(3, 4).shape, (3, 4))
    assert_shape(torch.eye(0, dtype=torch.float32, device="cpu").shape, (0, 0))

    with assert_raises(RuntimeError):
        # E: Cannot evaluate type-level shape DSL call: extent must be non-negative
        torch.eye(-1, 2)

    with assert_raises(TypeError):
        # E: No matching overload found
        torch.eye(2, 3, torch.float32)


if TYPE_CHECKING:

    def check_tensor_data_fallbacks(values: list[int], rows: list[list[int]]) -> None:
        assert_type(torch.tensor(values), Tensor[IntTuple])
        assert_type(torch.tensor(rows), Tensor[IntTuple])
        assert_type(torch.tensor([[1], [2, 3]]), Tensor[IntTuple])
        assert_type(torch.tensor([1, [2]]), Tensor[IntTuple])

    def check_constructor_context() -> None:
        matrix: Tensor[[2, 2]]
        matrix = torch.tensor(1)  # E: is not assignable to variable `matrix`
        matrix = torch.Tensor(1)  # E: is not assignable to variable `matrix`
        assert_type(matrix, Tensor[[2, 2]])

    def check_size_factories[N: IntVar](
        n: Int[N],
        plain: int,
        unbounded: tuple[int, ...],
        unpacked: tuple[Literal[1], *tuple[int, ...], Literal[3]],
        dimensions: list[int],
    ) -> None:
        assert_type(torch.randn(n, plain), Tensor[[N, int]])
        assert_type(torch.rand((n, plain)), Tensor[[N, int]])
        assert_type(torch.zeros(*dimensions), Tensor[IntTuple])
        assert_type(torch.ones(unbounded), Tensor[IntTuple])
        assert_type(torch.empty(*unpacked), Tensor[[1, *IntTuple, 3]])
        assert_type(torch.full(unbounded, 1.0), Tensor[IntTuple])

    def check_arange[N: IntVar, M: IntVar](
        end: Int[N], step: Int[M], dynamic: int
    ) -> None:
        assert_type(torch.arange(end), Tensor[[N]])
        assert_type(torch.arange(0, end), Tensor[[N]])
        assert_type(torch.arange(0, end, 2), Tensor[[N // 2]])
        assert_type(torch.arange(1, end), Tensor[[N - 1]])
        assert_type(torch.arange(end, 10), Tensor[[10 - N]])
        assert_type(torch.arange(0, end, step), Tensor[[int]])
        assert_type(torch.arange(dynamic), Tensor[[int]])
        assert_type(torch.arange(0, dynamic, dynamic), Tensor[[int]])
        assert_type(
            torch.arange(-9223372036854775808, 9223372036854775807),
            Tensor[[int]],
        )

        values = torch.arange(0, end, 2)[: end // 2].float() / end
        assert_type(values, Tensor[[N // 2]])

    def check_dimension_driven_creation[N: IntVar](n: Int[N], dynamic: int) -> None:
        assert_type(torch.linspace(0, 1, n), Tensor[[N]])
        assert_type(torch.linspace(0, 1, dynamic), Tensor[[int]])
        assert_type(torch.eye(n), Tensor[[N, N]])
        assert_type(torch.eye(n, dynamic), Tensor[[N, int]])

    def check_like_factories[Shape: IntTuple](x: Tensor[Shape]) -> None:
        assert_type(torch.zeros_like(x), Tensor[Shape])
        assert_type(torch.ones_like(x), Tensor[Shape])
        assert_type(torch.empty_like(x), Tensor[Shape])
        assert_type(torch.full_like(x, 2.5), Tensor[Shape])
        assert_type(torch.rand_like(x), Tensor[Shape])
        assert_type(torch.randn_like(x), Tensor[Shape])


def test_like_factories() -> None:
    x = torch.randn((2, 0, 3))
    for result in (
        torch.zeros_like(x),
        torch.ones_like(x),
        torch.ones_like(x, dtype=torch.float32, device=x.device),
        torch.empty_like(x),
        torch.full_like(x, 2.5),
        torch.rand_like(x),
        torch.randn_like(x),
    ):
        assert_shape(result.shape, (2, 0, 3))

    scalar = torch.tensor(1.0)
    for result in (
        torch.zeros_like(scalar),
        torch.ones_like(scalar),
        torch.empty_like(scalar),
        torch.full_like(scalar, 2.5),
        torch.rand_like(scalar),
        torch.randn_like(scalar),
    ):
        assert_shape(result.shape, ())
