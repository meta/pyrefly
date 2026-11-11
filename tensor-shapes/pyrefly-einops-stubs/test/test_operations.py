# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Any, assert_type, TYPE_CHECKING

import jax.numpy as jnp
import numpy as np
from einops import (
    __version__,
    asnumpy,
    EinopsError,
    einsum,
    pack,
    parse_shape,
    rearrange,
    reduce,
    repeat,
    unpack,
)
from shape_extensions import assert_shape, IntTuple
from torch import arange, ones, Tensor

if TYPE_CHECKING:
    from einops import _NumpyResult
    from jax import Array


def test_pattern_operations() -> None:
    image = ones((2, 3, 5, 7))
    rearranged = rearrange(image, "b c h w -> b h w c")
    assert_type(rearranged, Tensor[[2, 5, 7, 3]])
    assert_shape(rearranged.shape, (2, 5, 7, 3))
    reduced = reduce(ones((2, 3)), "row col -> row", "mean")
    assert_type(reduced, Tensor[[2]])
    assert_shape(reduced.shape, (2,))
    repeated = repeat(ones((2, 3)), "row col -> row col 2")
    assert_type(repeated, Tensor[[2, 3, 2]])
    assert_shape(
        repeated.shape,
        (2, 3, 2),
    )
    assert_shape(
        repeat(ones((2, 3)), "b c -> b c copies", copies=4).shape,
        (2, 3, 4),
        runtime=(2, 3, 4),
    )
    assert_type(repeat(arange(5), "n -> d n", d=6), Tensor[[6, 5]])
    assert_type(
        rearrange(
            ones((2, 17, 96)),
            "b n (three h d) -> three b h n d",
            three=3,
            h=4,
        ),
        Tensor[[3, 2, 4, 17, 8]],
    )
    assert_type(
        reduce(ones((2, 12, 10)), "b (h d) n -> b h", "sum", h=3),
        Tensor[[2, 3]],
    )
    dynamic_axes: dict[str, int] = {"copies": 4}
    assert_shape(
        repeat(ones((2, 3)), "b c -> b c copies", **dynamic_axes).shape,
        (2, 3, int),
        runtime=(2, 3, 4),
    )
    if TYPE_CHECKING:
        rearrange(  # E: named axes must appear on both sides of the pattern
            image, "b c h w -> b c h missing"
        )
        repeat(image, "b c h w -> b c h w", unused=2)  # E: axis 'unused' is not used
        repeat(  # E: axis 'copies' must have a nonnegative length, got -2
            ones((2, 3)), "b c -> b c copies", copies=-2
        )
        repeat(  # E: expected input rank 2, got 4
            image, "b c -> b c copies", **dynamic_axes
        )
    else:
        try:
            rearrange(image, "b c h w -> b c h missing")
        except EinopsError:
            pass
        else:
            raise AssertionError("invalid rearrange pattern should raise EinopsError")


def test_einsum() -> None:
    left = ones((2, 3, 5))
    right = ones((2, 5, 7))
    product = einsum(
        left,
        right,
        "batch row inner, batch inner col -> batch row col",
    )
    assert_type(product, Tensor[[2, 3, 7]])
    assert_shape(product.shape, (2, 3, 7))
    assert_type(einsum(ones((3, 3)), "row row ->"), Tensor[[]])
    assert_type(einsum(ones((3,)), ones((5,)), "row, col -> row col"), Tensor[[3, 5]])
    assert_shape(
        einsum(
            ones((2,)),
            ones((2,)),
            ones((2,)),
            ones((2,)),
            ones((2,)),
            "i, i, i, i, i -> i",
        ).shape,
        IntTuple,
        runtime=(2,),
    )


def test_numpy_shape_operations() -> None:
    image = np.ones((2, 3, 5, 7))
    rearranged = rearrange(image, "b c h w -> b h w c")
    reduced = reduce(np.ones((2, 3)), "row col -> row", "mean")
    repeated = repeat(np.ones((2, 3)), "row col -> row col copies", copies=4)
    product = einsum(
        np.ones((2, 3, 5)),
        np.ones((2, 5, 7)),
        "batch row inner, batch inner col -> batch row col",
    )
    diagonal = einsum(np.ones((3, 3)), "row row -> row")
    mixed = einsum(
        np.ones((2,), dtype=np.float32), np.ones((2,), dtype=np.int32), "i, i -> i"
    )
    triple = einsum(np.ones((2,)), np.ones((2,)), np.ones((2,)), "i, i, i -> i")
    quadruple = einsum(
        np.ones((2,)), np.ones((2,)), np.ones((2,)), np.ones((2,)), "i, i, i, i -> i"
    )
    assert_shape(rearranged.shape, (2, 5, 7, 3), runtime=(2, 5, 7, 3))
    assert_shape(reduced.shape, (2,), runtime=(2,))
    assert_shape(repeated.shape, (2, 3, 4), runtime=(2, 3, 4))
    assert_shape(product.shape, (2, 3, 7), runtime=(2, 3, 7))
    assert_shape(diagonal.shape, (3,), runtime=(3,))
    assert_shape(mixed.shape, (2,), runtime=(2,))
    assert_shape(triple.shape, (2,), runtime=(2,))
    assert_shape(quadruple.shape, (2,), runtime=(2,))
    if TYPE_CHECKING:
        assert_type(rearranged, np.ndarray[[2, 5, 7, 3], np.dtype[np.float64]])
        assert_type(reduced, _NumpyResult[[2]])
        assert_type(repeated, np.ndarray[[2, 3, 4], np.dtype[np.float64]])
        assert_type(product, _NumpyResult[[2, 3, 7]])
        assert_type(diagonal, _NumpyResult[[3]])
        assert_type(mixed, _NumpyResult[[2]])
        assert_type(triple, _NumpyResult[[2]])
        assert_type(quadruple, _NumpyResult[[2]])
        rearrange(  # E: named axes must appear on both sides of the pattern
            image, "b c h w -> b c h missing"
        )
        repeat(image, "b c h w -> b c h w", unused=2)  # E: axis 'unused' is not used
    else:
        try:
            rearrange(image, "b c h w -> b c h missing")
        except EinopsError:
            pass
        else:
            raise AssertionError("invalid NumPy pattern should raise EinopsError")


def test_numpy_scalar_results() -> None:
    total = reduce(np.ones((2, 3)), "row col ->", "sum")
    diagonal_sum = einsum(np.ones((3, 3)), "row row ->")
    object_array = np.array([1, 2], dtype=np.object_)
    object_sum = reduce(object_array, "i ->", "sum")
    object_contractions = (
        einsum(object_array, "i ->"),
        einsum(object_array, object_array, "i, i ->"),
        einsum(object_array, object_array, object_array, "i, i, i ->"),
        einsum(object_array, object_array, object_array, object_array, "i, i, i, i ->"),
    )
    converted = total.__array__()
    assert_shape(total.shape, (), runtime=())
    assert_shape(diagonal_sum.shape, (), runtime=())
    assert_shape(converted.shape, (), runtime=())
    if TYPE_CHECKING:
        assert_type(total, _NumpyResult[[]])
        assert_type(diagonal_sum, _NumpyResult[[]])
        assert_type(converted, np.ndarray[[], Any])
        assert_type(object_sum, Any)
        assert_type(object_contractions, tuple[Any, Any, Any, Any])
    else:
        assert object_contractions == (3, 5, 9, 17)
        assert object_sum == 3
        assert isinstance(object_sum, int)
        assert isinstance(total, np.generic)
        assert isinstance(diagonal_sum, np.generic)
        assert not isinstance(total, np.ndarray)
        assert np.asarray(total).item() == 6
        assert np.asarray(diagonal_sum).item() == 3


def test_jax_shape_operations() -> None:
    image = jnp.ones((2, 3, 5, 7))
    rearranged = rearrange(image, "b c h w -> b h w c")
    reduced = reduce(jnp.ones((2, 3)), "row col -> row", "mean")
    repeated = repeat(jnp.ones((2, 3)), "row col -> row col copies", copies=4)
    product = einsum(
        jnp.ones((2, 3, 5)),
        jnp.ones((2, 5, 7)),
        "batch row inner, batch inner col -> batch row col",
    )
    diagonal = einsum(jnp.ones((3, 3)), "row row ->")
    triple = einsum(jnp.ones((2,)), jnp.ones((2,)), jnp.ones((2,)), "i, i, i -> i")
    quadruple = einsum(
        jnp.ones((2,)),
        jnp.ones((2,)),
        jnp.ones((2,)),
        jnp.ones((2,)),
        "i, i, i, i -> i",
    )
    assert_shape(rearranged.shape, (2, 5, 7, 3), runtime=(2, 5, 7, 3))
    assert_shape(reduced.shape, (2,), runtime=(2,))
    assert_shape(repeated.shape, (2, 3, 4), runtime=(2, 3, 4))
    assert_shape(product.shape, (2, 3, 7), runtime=(2, 3, 7))
    assert_shape(diagonal.shape, (), runtime=())
    assert_shape(triple.shape, (2,), runtime=(2,))
    assert_shape(quadruple.shape, (2,), runtime=(2,))
    if TYPE_CHECKING:
        assert_type(rearranged, Array[[2, 5, 7, 3]])
        assert_type(reduced, Array[[2]])
        assert_type(repeated, Array[[2, 3, 4]])
        assert_type(product, Array[[2, 3, 7]])
        assert_type(diagonal, Array[[]])
        assert_type(triple, Array[[2]])
        assert_type(quadruple, Array[[2]])
        rearrange(  # E: named axes must appear on both sides of the pattern
            image, "b c h w -> b c h missing"
        )
        repeat(image, "b c h w -> b c h w", unused=2)  # E: axis 'unused' is not used
    else:
        try:
            rearrange(image, "b c h w -> b c h missing")
        except EinopsError:
            pass
        else:
            raise AssertionError("invalid JAX pattern should raise EinopsError")


def test_other_backends_at_runtime() -> None:
    if not TYPE_CHECKING:
        assert_shape(reduce(np.ones((2, 3)), "row col -> row", "mean").shape, (2,))
        assert_shape(
            repeat(jnp.ones((2, 3)), "row col -> row col 2").shape,
            (2, 3, 2),
        )
        assert_shape(einsum(np.ones((3, 3)), "row row ->").shape, ())
        assert_shape(
            einsum(
                jnp.ones((3,)),
                jnp.ones((5,)),
                "row, col -> row col",
            ).shape,
            (3, 5),
        )


def test_auxiliary_api() -> None:
    assert_type(__version__, str)
    tensor = ones((2, 3, 5))
    assert parse_shape(tensor, "batch channel width") == {
        "batch": 2,
        "channel": 3,
        "width": 5,
    }
    assert_type(parse_shape(tensor, "batch channel width"), dict[str, int])
    assert_shape(asnumpy(tensor).shape, (2, 3, 5))

    packed, packed_shapes = pack([ones((2, 3)), ones((2, 5))], "batch *")
    assert_type(packed, Tensor[IntTuple])
    assert_shape(packed.shape, IntTuple, runtime=(2, 8))
    first, second = unpack(packed, packed_shapes, "batch *")
    assert_type(first, Tensor[IntTuple])
    assert_type(second, Tensor[IntTuple])
    assert_shape(first.shape, IntTuple, runtime=(2, 3))
    assert_shape(second.shape, IntTuple, runtime=(2, 5))
    assert issubclass(EinopsError, RuntimeError)
