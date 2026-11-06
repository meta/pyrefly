# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import Any, assert_type

import numpy as np
from shape_extensions import assert_shape, IntTuple


def test_argsort_preserves_shape_along_axis() -> None:
    vector = np.ones(5)
    matrix = np.ones((2, 3))

    assert_shape(np.argsort(vector).shape, (5,))
    assert_shape(np.argsort(matrix, axis=0).shape, (2, 3))
    assert_shape(np.argsort(matrix, -1, stable=True).shape, (2, 3))
    assert_shape(np.argsort(matrix, kind="stable").shape, (2, 3))
    assert_type(np.argsort(vector).dtype, np.dtype[np.intp])
    assert_shape(np.argsort([3, 1, 2]).shape, IntTuple, runtime=(3,))


def test_argsort_flattens_without_axis() -> None:
    matrix = np.ones((2, 3))
    scalar = np.ones(())

    assert_shape(np.argsort(matrix, axis=None).shape, (6,))
    # NumPy sorts a rank-zero array as a one-element vector.
    assert_shape(np.argsort(scalar).shape, IntTuple, runtime=(1,))


def test_cumsum_preserves_shape_along_axis() -> None:
    matrix = np.ones((2, 3))
    out = np.empty((2, 3))

    assert_shape(np.cumsum(matrix, axis=1).shape, (2, 3))
    assert_shape(np.cumsum(matrix, 0, np.float32).shape, (2, 3))
    assert_type(
        np.cumsum(matrix, axis=0, out=out), np.ndarray[[2, 3], np.dtype[np.float64]]
    )


def test_cumsum_flattens_without_axis() -> None:
    vector = np.ones(5)
    matrix = np.ones((2, 3))
    scalar = np.ones(())

    assert_shape(np.cumsum(vector).shape, (5,))
    assert_shape(np.cumsum(matrix).shape, (6,))
    assert_shape(np.cumsum(scalar).shape, (1,))
    assert_shape(np.cumsum([1, 2, 3]).shape, (int,), runtime=(3,))


def test_cumsum_dtype_follows_accumulation() -> None:
    matrix = np.ones((2, 3), dtype=np.float32)
    flags = np.ones(5, dtype=np.bool_)
    small = np.ones(5, dtype=np.int8)

    assert_shape(np.cumsum(flags, axis=0).shape, (5,))
    assert_type(np.cumsum(matrix, axis=1).dtype, np.dtype[np.float32])
    assert_type(np.cumsum(matrix).dtype, np.dtype[np.float32])
    # Boolean and small integer inputs accumulate in a wider integer dtype.
    assert_type(np.cumsum(flags, axis=0).dtype, Any)
    assert_type(np.cumsum(small, axis=0).dtype, Any)
    assert np.issubdtype(np.cumsum(flags, axis=0).dtype, np.integer)
    assert np.cumsum(small, axis=0).dtype != np.dtype(np.int8)
