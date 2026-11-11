# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import assert_type, TYPE_CHECKING

import jax.numpy as jnp
from jax import Array, lax
from shape_extensions import assert_raises, assert_shape, IntTuple, IntVar


def test_reshape_rejects_invalid_sizes_and_permutations() -> None:
    x = jnp.arange(6).reshape((2, 3))
    assert_shape(lax.reshape(x, (3, 2)).shape, (3, 2))
    if not TYPE_CHECKING:
        for sizes, dimensions in [
            ((7,), None),
            ((-1,), None),
            ((6,), (0,)),
            ((6,), (0, 0)),
            ((6,), (-1, 0)),
        ]:
            with assert_raises((TypeError, ValueError)):
                lax.reshape(x, sizes, dimensions)
    if TYPE_CHECKING:
        lax.reshape(x, (7,))  # E: reshape target element count does not match the input
        lax.reshape(x, (-1,))  # E: lax.reshape sizes must be non-negative
        lax.reshape(x, (6,), (0,))  # E: reshape dimensions must cover every input axis
        lax.reshape(x, (6,), (0, 0))  # E: duplicate reshape dimension
        lax.reshape(x, (6,), (-1, 0))  # E: reshape dimension out of bounds


def test_reshape_sequence_sizes_and_permutations() -> None:
    x = jnp.ones((2, 3))
    assert_shape(lax.reshape(x, (3, 2), [1, 0]).shape, (3, 2))
    assert_shape(lax.reshape(jnp.ones((0, 3)), (3, 0)).shape, (3, 0))
    assert_shape(lax.reshape(x, [3, 2]).shape, IntTuple, runtime=(3, 2))


def check_symbolic_reshape[N: IntVar, M: IntVar, K: IntVar](
    x: Array[[N, M, K]],
) -> None:
    batch, sequence, width = x.shape
    assert_type(lax.reshape(x, (batch * sequence, width)), Array[[N * M, K]])
    assert_type(lax.reshape(x, (batch * sequence, width), (1, 0, 2)), Array[[N * M, K]])


def test_reshape_unshaped_operand() -> None:
    x: Array = jnp.arange(6)
    assert_shape(lax.reshape(x, (2, 3)).shape, (2, 3))


def check_variable_permutation(x: Array[[2, 3]], perm: tuple[int, int]) -> None:
    # TODO(stroxler): A tuple-valued Flag permutation also erases a known target shape.
    assert_type(lax.reshape(x, (3, 2), perm), Array[IntTuple])
