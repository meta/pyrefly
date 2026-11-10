# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import Any, assert_type, Hashable, NamedTuple

import jax
import jax.numpy as jnp
from jax import Array, lax
from shape_extensions import assert_raises, assert_shape, IntTuple


class Pair(NamedTuple):
    matrix: Array[[4, 3]]
    vector: Array[[2]]


def test_pytree_collective_leaf_shapes() -> None:
    def local(x: Array[[4, 3]]) -> Array[[4, 3]]:
        gathered = lax.all_gather({"x": x}, "i")
        assert_type(gathered, Any)
        assert_shape(gathered["x"].shape, IntTuple, runtime=(4, 4, 3))
        exchanged = lax.all_to_all({"x": x}, "i", 0, 1, tiled=True)
        assert_type(exchanged, Any)
        assert_shape(exchanged["x"].shape, IntTuple, runtime=(1, 12))
        scattered = lax.psum_scatter({"x": x}, "i", tiled=True)
        assert_type(scattered, Any)
        assert_shape(scattered["x"].shape, IntTuple, runtime=(1, 3))
        return x

    jax.vmap(local, axis_name="i")(jnp.ones((4, 4, 3)))


def test_collective_array_tuples() -> None:
    def local(x: Array[[4, 3]]) -> Array[[4, 3]]:
        gathered = lax.all_gather((x, x[0, :2]), "i")
        assert_type(gathered, Any)
        assert_shape(gathered[0].shape, IntTuple, runtime=(4, 4, 3))
        assert_shape(gathered[1].shape, IntTuple, runtime=(4, 2))
        exchanged = lax.all_to_all((x, x[:, :2]), "i", 0, 1, tiled=True)
        assert_type(exchanged, Any)
        assert_shape(exchanged[0].shape, IntTuple, runtime=(1, 12))
        assert_shape(exchanged[1].shape, IntTuple, runtime=(1, 8))
        scattered = lax.psum_scatter((x, x[:, :2]), "i", tiled=True)
        assert_type(scattered, Any)
        assert_shape(scattered[0].shape, IntTuple, runtime=(1, 3))
        assert_shape(scattered[1].shape, IntTuple, runtime=(1, 2))
        return x

    jax.vmap(local, axis_name="i")(jnp.ones((4, 4, 3)))


def test_collective_namedtuple_falls_back_to_any() -> None:
    def local(matrix: Array[[4, 3]], vector: Array[[2]]) -> Array[[4, 3]]:
        gathered = lax.all_gather(Pair(matrix, vector), "i")
        # Any keeps field access unchecked instead of claiming an unchanged leaf shape.
        assert_type(gathered, Any)
        assert_shape(gathered.matrix.shape, IntTuple, runtime=(4, 4, 3))
        assert_shape(gathered.vector.shape, IntTuple, runtime=(4, 2))
        return matrix

    jax.vmap(local, axis_name="i")(jnp.ones((4, 4, 3)), jnp.ones((4, 2)))


def test_collective_single_array_shape_information() -> None:
    def local(x: Array[[4, 3]]) -> Array[[4, 3]]:
        assert_shape(lax.all_gather(x, "i").shape, (int, 4, 3), runtime=(4, 4, 3))
        assert_shape(
            lax.all_gather(x, "i", axis=-1).shape, (4, 3, int), runtime=(4, 3, 4)
        )
        assert_shape(
            lax.all_gather(x, "i", axis=1, tiled=True).shape, (4, int), runtime=(4, 12)
        )
        assert_shape(lax.psum_scatter(x, "i").shape, (3,))
        assert_shape(
            lax.psum_scatter(x, "i", tiled=True).shape, (int, 3), runtime=(1, 3)
        )
        scalar = jnp.sum(x)
        assert_shape(lax.all_gather(scalar, "i").shape, (int,), runtime=(4,))
        with assert_raises(ValueError):
            lax.all_gather(scalar, "i", tiled=True)  # E: axis out of bounds
        with assert_raises(ValueError):
            lax.all_gather(x, "i", axis=3)  # E: all_gather axis out of bounds
        with assert_raises(ValueError):
            lax.all_gather(x, "i", axis=2, tiled=True)  # E: axis out of bounds
        with assert_raises(ValueError):
            lax.psum_scatter(x, "i", scatter_dimension=2)  # E: dimension out of bounds
        return x

    jax.vmap(local, axis_name="i")(jnp.ones((4, 4, 3)))


def check_dynamic_collective_axes(x: Array[[4, 3]], axis: int) -> None:
    assert_type(lax.all_gather(x, "i", axis=axis), Array[IntTuple])
    assert_type(lax.psum_scatter(x, "i", scatter_dimension=axis), Array[IntTuple])


def test_empty_axis_collectives_return_input() -> None:
    matrix = jnp.ones((4, 3))
    scalar = jnp.asarray(2.0)
    gathered = lax.all_gather(matrix, (), axis=99, tiled=True)
    assert_type(gathered, Array[[4, 3]])
    assert_shape(gathered.shape, (4, 3))
    assert gathered is matrix
    scattered = lax.psum_scatter(matrix, (), scatter_dimension=99)
    assert_type(scattered, Array[[4, 3]])
    assert_shape(scattered.shape, (4, 3))
    assert scattered is matrix
    gathered_scalar = lax.all_gather(scalar, ())
    assert_type(gathered_scalar, Array[[]])
    assert_shape(gathered_scalar.shape, ())
    scattered_scalar = lax.psum_scatter(scalar, ())
    assert_type(scattered_scalar, Array[[]])
    assert_shape(scattered_scalar.shape, ())
    pair = Pair(matrix, jnp.ones((2,)))
    gathered_pair = lax.all_gather(pair, ())
    scattered_pair = lax.psum_scatter(pair, ())
    assert_type(gathered_pair, Pair)
    assert_type(scattered_pair, Pair)
    assert_shape(gathered_pair.matrix.shape, (4, 3))
    assert_shape(scattered_pair.vector.shape, (2,))
    assert gathered_pair is pair and scattered_pair is pair


def check_dynamic_axis_names(
    x: Array[[4, 3]], names: Hashable, tuple_names: tuple[str, ...], scalar: float
) -> None:
    assert_type(lax.all_gather(x, names), Array[IntTuple])
    assert_type(lax.psum_scatter(x, names), Array[IntTuple])
    assert_type(lax.all_gather(x, tuple_names), Array[IntTuple])
    assert_type(lax.psum_scatter(x, tuple_names), Array[IntTuple])
    assert_type(lax.all_gather(scalar, names), Any)
    assert_type(lax.psum_scatter(scalar, names), Any)
