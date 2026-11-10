# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import Any, assert_type, cast

import numpy as np
from shape_extensions import assert_raises, assert_shape, IntTuple


def test_matrix_arithmetic() -> None:
    a = np.ones((3, 4))
    b = np.full((3, 4), 2.0)

    assert_shape((a + b).shape, (3, 4))
    assert_shape((b - a).shape, (3, 4))
    assert_shape((a * b).shape, (3, 4))
    assert_shape((b**2).shape, (3, 4))


def test_column_subtraction_preserves_shape() -> None:
    outcomes = np.ones((3, 1))
    probabilities = np.full((3, 1), 0.25)

    assert_shape((outcomes - probabilities).shape, (3, 1))
    assert_shape((1.0 - probabilities).shape, (3, 1))


def test_scalar_rhs_arithmetic() -> None:
    a = np.full(4, 2.0)
    b = np.ones((3, 4))
    c = np.full((3, 4), 2.0)

    assert_shape((a * 2.0).shape, (4,))
    assert_shape((a + 1.0).shape, (4,))
    assert_shape((a - 1.0).shape, (4,))
    assert_shape((a**2).shape, (4,))
    assert_shape((b + 1.0).shape, (3, 4))
    assert_shape((1.0 - b).shape, (3, 4))
    assert_shape((c * 2.0).shape, (3, 4))


def test_unary_arithmetic() -> None:
    a = np.full(5, -1.0)

    assert_shape(np.abs(a).shape, (5,))
    assert_shape(np.negative(a).shape, (5,))
    assert_shape((-a).shape, (5,))
    assert_shape((+a).shape, (5,))
    assert_shape((-np.ones((3, 4))).shape, (3, 4))


def test_scalar_array_arithmetic() -> None:
    a = np.full((2, 3), 2.0, dtype=np.float32)

    assert_shape((a + 1j).shape, (2, 3))
    assert_shape((1j + a).shape, (2, 3))
    assert_shape((a - 1j).shape, (2, 3))
    assert_shape((1j - a).shape, (2, 3))
    assert_shape((a * 1j).shape, (2, 3))
    assert_shape((1j * a).shape, (2, 3))
    assert_shape((a / 1j).shape, (2, 3))
    assert_shape((1j / a).shape, (2, 3))
    assert_shape((a**1j).shape, (2, 3))
    assert_shape((1j**a).shape, (2, 3))
    assert_type(a + 1j, np.ndarray[[2, 3]])
    assert (a + 1j).dtype == np.complex64

    scalar = np.array(2.0)
    assert_shape((scalar * 1j).shape, ())

    real = np.float64(2.0)
    assert_shape((real * a).shape, (2, 3))
    assert_shape((a * real).shape, (2, 3))
    assert (a * real).dtype == np.float64


def test_builtin_abs_preserves_shape() -> None:
    scalar = np.array(-2.0)
    vector = np.full(5, -1.0)
    matrix = np.full((3, 4), -2, dtype=np.int64)

    assert_type(abs(scalar), np.ndarray[[], Any])
    assert_shape(abs(scalar).shape, ())
    assert_type(abs(vector), np.ndarray[[5], Any])
    assert_shape(abs(vector).shape, (5,))
    assert_type(abs(matrix), np.ndarray[[3, 4], Any])
    assert_shape(abs(matrix).shape, (3, 4))
    assert np.array_equal(abs(matrix), np.full((3, 4), 2))

    error = abs(scalar - 1.0) + abs(scalar + 1.0)
    assert_type(error, np.ndarray[[], Any])
    assert_shape(error.shape, ())
    assert error == 4.0


def test_builtin_abs_preserves_unknown_shape() -> None:
    unknown = cast("np.ndarray", np.full((2, 3), -1.0))

    assert_type(abs(unknown), np.ndarray)
    assert_shape(abs(unknown).shape, IntTuple, runtime=(2, 3))


def test_builtin_abs_complex_array() -> None:
    values = np.full((2, 3), 3 + 4j, dtype=np.complex64)
    magnitudes = abs(values)

    assert_type(magnitudes, np.ndarray[[2, 3], Any])
    assert_shape(magnitudes.shape, (2, 3))
    assert magnitudes.dtype == np.float32
    assert np.array_equal(magnitudes, np.full((2, 3), 5.0))


def test_scalar_defaults_preserve_shape_and_dtype() -> None:
    a = np.full((2, 3), 2.0, dtype=np.float64)

    assert_type(a + 1.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(1.0 + a, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(a - 1.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(1.0 - a, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(a * 2.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(2.0 * a, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(a / 2.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(2.0 / a, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(a // 2.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(2.0 // a, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(a % 2.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(2.0 % a, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(a**2.0, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(2.0**a, np.ndarray[[2, 3], np.dtype[np.float64]])

    assert_type(a < 2.0, np.ndarray[[2, 3], np.dtype[np.bool_]])
    assert_type(a <= 2.0, np.ndarray[[2, 3], np.dtype[np.bool_]])
    assert_type(a > 2.0, np.ndarray[[2, 3], np.dtype[np.bool_]])
    assert_type(a >= 2.0, np.ndarray[[2, 3], np.dtype[np.bool_]])
    assert_type(a == 2.0, np.ndarray[[2, 3], np.dtype[np.bool_]])
    assert_type(a != 2.0, np.ndarray[[2, 3], np.dtype[np.bool_]])

    integers = np.ones((2, 3), dtype=np.int64)
    assert_type(integers & 1, np.ndarray[[2, 3], Any])
    assert_type(1 & integers, np.ndarray[[2, 3], Any])
    assert_type(integers | 1, np.ndarray[[2, 3], Any])
    assert_type(1 | integers, np.ndarray[[2, 3], Any])
    assert_type(integers ^ 1, np.ndarray[[2, 3], Any])
    assert_type(1 ^ integers, np.ndarray[[2, 3], Any])

    scalar = np.array(2.0)
    assert_type(scalar + 1.0, np.ndarray[[]])
    assert_shape((scalar + 1.0).shape, ())
    assert_shape((a + scalar).shape, (2, 3))


def test_scalar_defaults_preserve_unknown_shapes() -> None:
    unknown = cast("np.ndarray", np.ones((2, 3), dtype=np.int64))
    concrete = np.ones((2, 3))

    assert_type(unknown + 1.0, np.ndarray)
    assert_type(1.0 + unknown, np.ndarray)
    assert_type(unknown + concrete, np.ndarray)
    assert_type(concrete + unknown, np.ndarray[IntTuple, np.dtype[np.float64]])
    assert_shape((unknown + 1.0).shape, IntTuple, runtime=(2, 3))
    assert_type(unknown > 1.0, Any)
    assert_type(unknown & 1, np.ndarray)


def check_array_like_operands() -> None:
    # NumPy coerces a list operand of `@`, but the overlay still accepts only arrays
    # there.
    matrix = np.eye(3)
    scale = np.float64(2.0)

    product = matrix @ [1.0, 0.0, 0.0]  # E: not supported
    quotient = [2.0, 4.0] / scale

    assert product.tolist() == [1.0, 0.0, 0.0]
    assert quotient.tolist() == [1.0, 2.0]


def check_unary_ufunc_sequence_input() -> None:
    # A list input to a unary ufunc always produces an array, never a scalar.
    radians = np.deg2rad([23.0, -47.0, 31.0])
    finite = np.isfinite([1.0, 2.0])

    x, y, z = radians
    assert finite.all()
    assert (x, y, z) != (0.0, 0.0, 0.0)


def test_integer_array_shifts() -> None:
    values = np.full((2, 1), 8, dtype=np.uint64)
    counts = np.ones((1, 3), dtype=np.uint64)

    right = values >> np.uint64(1)
    left = values << np.uint64(1)
    reflected_right = np.uint64(16) >> values
    reflected_left = np.uint64(1) << values
    broadcasted = values >> counts
    assert_type(right, np.ndarray[[2, 1], Any])
    assert_type(left, np.ndarray[[2, 1], Any])
    assert_shape(right.shape, (2, 1))
    assert_shape(left.shape, (2, 1))
    assert_shape(reflected_right.shape, (2, 1))
    assert_shape(reflected_left.shape, (2, 1))
    assert_shape(broadcasted.shape, (2, 3))
    assert right.tolist() == [[4], [4]]
    assert left.tolist() == [[16], [16]]
    with assert_raises(ValueError):
        values >> np.ones((3, 2), dtype=np.uint64)  # E: Cannot broadcast
    with assert_raises(TypeError):
        values >> 0.5  # E: not supported


def test_shift_dtype_promotion_and_scalar_result() -> None:
    values = np.ones((2, 3), dtype=np.int8)
    promoted = values << np.int64(1)
    scalar = np.array(8, dtype=np.uint64) >> np.uint64(1)
    assert_type(promoted, np.ndarray[[2, 3], Any])
    assert_type(scalar, Any)
    assert_shape(promoted.shape, (2, 3))
    assert_shape(scalar.shape, IntTuple, runtime=())
    assert promoted.dtype == np.dtype(np.int64)
    assert isinstance(scalar, np.uint64)


def test_array_arithmetic_with_numeric_lists() -> None:
    values = np.ones(int("3"))
    counts = np.full(3, 2, dtype=np.int64)
    column = [[0.1], [0.2]]

    added = values + column
    reflected = column + values
    subtracted = values - [1, 2, 3]
    divided = values / [[1.0], [2.0]]
    powered = counts ** [2, 2, 2]
    multiplied = values * [1.0, 2.0, 3.0]
    assert_type(added, np.ndarray)
    assert_type(reflected, np.ndarray)
    assert_type(subtracted, np.ndarray)
    assert_type(divided, np.ndarray)
    assert_type(powered, np.ndarray)
    assert_type(multiplied, np.ndarray)
    assert_type(values + 1.0, np.ndarray[[int], np.dtype[np.float64]])
    assert_shape(counts.shape, (3,))
    assert added.shape == reflected.shape == divided.shape == (2, 3)
    assert subtracted.shape == (3,)
    assert powered.dtype == np.dtype(np.int64)
    assert isinstance(multiplied, np.ndarray)
    with assert_raises(TypeError):
        values + ["a", "b", "c"]  # E: `+` is not supported
