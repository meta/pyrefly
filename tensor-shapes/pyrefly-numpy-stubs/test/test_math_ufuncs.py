# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import Any, assert_type, cast, Literal

import numpy as np
from shape_extensions import assert_raises, assert_shape, IntTuple


def make_array(shape: Any, value: Any = 1.0) -> Any:
    return np.full(shape, value)


def test_exponential_and_log_preserve_shape() -> None:
    a = np.full(4, 2.0)
    b = np.ones((3, 4))

    assert_shape(np.exp(a).shape, (4,))
    assert_shape(np.log(a).shape, (4,))
    assert_shape(np.log2(a).shape, (4,))
    assert_shape(np.log10(a).shape, (4,))
    assert_shape(np.power(b, 2).shape, (3, 4))


def test_unary_ufuncs_type_arraylike_inputs() -> None:
    matrix = np.ones((3, 4))
    sqrt_array = np.sqrt(matrix)
    assert_type(sqrt_array, np.ndarray[[3, 4], Any])
    assert_shape(sqrt_array.shape, (3, 4))
    # Shared _UnaryUFunc sequence overload (not sqrt-specific).
    assert_type(np.sqrt([1.0, 4.0]), np.ndarray)
    assert_type(np.sin([0.0, 1.0]), np.ndarray)


def test_unary_ufuncs_preserve_shape() -> None:
    a = np.ones((2, 3))

    assert_shape(np.arcsin(a).shape, (2, 3))
    assert_shape(np.absolute(a).shape, (2, 3))
    assert_shape(np.exp2(a).shape, (2, 3))
    assert_shape(np.isfinite(a).shape, (2, 3))
    assert_shape(np.sin(a).shape, (2, 3))
    assert_shape(np.square(a).shape, (2, 3))
    # @lint-ignore SPELL
    assert np.arcsin.nin == 1
    assert np.abs.nout == 1
    # @lint-ignore SPELL
    assert np.absolute.nin == 1
    assert np.absolute.nout == 1
    assert np.absolute(-1.0) == 1.0


def test_binary_ufuncs_preserve_matrix_shape() -> None:
    a = np.ones((3, 4))
    b = np.full((3, 4), 2.0)

    assert_shape(np.minimum(a, b).shape, (3, 4))
    assert_shape(np.maximum(a, b).shape, (3, 4))
    assert_shape(np.arctan2(a, b).shape, (3, 4))


def test_binary_ufunc_objects_broadcast_arrays_and_scalars() -> None:
    matrix = np.ones((3, 1))
    row = np.full((1, 4), 2.0)

    assert_shape(np.add(matrix, row).shape, (3, 4))
    assert_shape(np.multiply(matrix, 2.0).shape, (3, 1))
    assert_shape(np.power(matrix, row).shape, (3, 4))
    assert_shape(np.equal(2.0, row).shape, (1, 4))
    # @lint-ignore SPELL
    assert np.add.nin == 2
    assert np.add.nout == 1
    # @lint-ignore SPELL
    assert np.arctan2.nin == 2
    # @lint-ignore SPELL
    assert np.matmul.nin == 2


def test_matmul_preserves_shapes_and_accepts_array_like_inputs() -> None:
    left = np.ones((2, 3))
    right = np.ones((3, 4))
    result = np.matmul(left, right)
    assert_type(result, np.ndarray[[2, 4], Any])
    assert_shape(result.shape, (2, 4))

    array_like = np.matmul([[1.0, 2.0]], [[1.0], [2.0]])
    assert_type(array_like, Any)
    assert array_like.shape == (1, 1)

    unknown = cast(Any, left)
    assert_type(np.matmul(unknown, right), Any)


def test_binary_ufunc_at_is_in_place() -> None:
    values = np.zeros(3)

    assert_type(np.maximum.at(values, [0, 1], 1.0), None)
    assert_shape(values.shape, (3,))


def test_multi_output_ufuncs_preserve_shapes() -> None:
    matrix = np.ones((2, 3))
    row = np.full((1, 3), 2.0)

    fraction, exponent = np.frexp(matrix)
    assert_shape(fraction.shape, (2, 3))
    assert_shape(exponent.shape, (2, 3))
    fractional, integral = np.modf(matrix)
    assert_shape(fractional.shape, (2, 3))
    assert_shape(integral.shape, (2, 3))
    quotient, remainder = np.divmod(matrix, row)
    assert_shape(quotient.shape, (2, 3))
    assert_shape(remainder.shape, (2, 3))

    frexp_fraction_out = np.empty_like(matrix)
    frexp_exponent_out = np.empty_like(matrix)
    frexp_result = np.frexp(matrix, frexp_fraction_out, frexp_exponent_out)
    assert frexp_result[0] is frexp_fraction_out
    assert frexp_result[1] is frexp_exponent_out

    divmod_quotient_out = np.empty_like(matrix)
    divmod_remainder_out = np.empty_like(matrix)
    divmod_result = np.divmod(matrix, row, divmod_quotient_out, divmod_remainder_out)
    assert divmod_result[0] is divmod_quotient_out
    assert divmod_result[1] is divmod_remainder_out


def test_vector_gufuncs_track_core_and_batch_dimensions() -> None:
    matrices = cast(
        "np.ndarray[tuple[Literal[4], Literal[2], Literal[3]]]",
        make_array((4, 2, 3)),
    )
    vectors = np.ones((4, 3))
    right_matrices = cast(
        "np.ndarray[tuple[Literal[4], Literal[3], Literal[5]]]",
        make_array((4, 3, 5)),
    )

    assert_shape(np.matvec(matrices, vectors).shape, (4, 2))
    assert_shape(np.vecdot(vectors, vectors).shape, (4,))
    assert_shape(np.vecmat(vectors, right_matrices).shape, (4, 5))
    try:
        np.vecdot(  # E: core dimension 'n' has conflicting extents 3 and 4
            np.ones(3), np.ones(4)
        )
    except ValueError:
        pass
    else:
        raise AssertionError("expected NumPy to reject mismatched core dimensions")


def test_extrema_broadcast_row_vector_over_matrix() -> None:
    matrix = np.ones((3, 4))
    row = np.full(4, 2.0)

    assert_shape(np.minimum(matrix, row).shape, (3, 4))
    assert_shape(np.minimum(row, matrix).shape, (3, 4))
    assert_shape(np.maximum(matrix, row).shape, (3, 4))
    assert_shape(np.maximum(row, matrix).shape, (3, 4))


def test_binary_ufuncs_broadcast_higher_rank_arrays() -> None:
    a = cast(
        "np.ndarray[tuple[Literal[2], Literal[3], Literal[1], Literal[5]]]",
        make_array((2, 3, 1, 5)),
    )
    b = cast(
        "np.ndarray[tuple[Literal[1], Literal[3], Literal[4], Literal[1]]]",
        make_array((1, 3, 4, 1), 2.0),
    )

    assert_shape(np.minimum(a, b).shape, (2, 3, 4, 5))
    assert_shape(np.maximum(a, b).shape, (2, 3, 4, 5))
    assert_shape(np.arctan2(a, b).shape, (2, 3, 4, 5))


def test_binary_ufuncs_fall_back_for_unknown_shapes() -> None:
    unknown = cast("np.ndarray", make_array((2, 3)))
    concrete = np.ones((2, 3), dtype=np.int64)

    minimum = np.minimum(unknown, concrete)
    # TODO(stroxler): Preserve more precision when broadcasting against unknown shapes.
    assert_type(minimum, np.ndarray[IntTuple])
    assert_shape(minimum.shape, IntTuple, runtime=(2, 3))


def test_binary_ufuncs_keep_broad_mixed_dtype_results() -> None:
    left = np.ones((2, 1), dtype=np.float32)
    right = np.ones((1, 3), dtype=np.int64)

    minimum = np.minimum(left, right)
    assert_type(minimum, np.ndarray[[2, 3], Any])
    assert_shape(minimum.shape, (2, 3))


def test_binary_ufuncs_reject_incompatible_broadcast() -> None:
    a = np.ones((3, 4))
    b = np.ones(5)

    assert_shape(np.minimum(a, np.ones((3, 4))).shape, (3, 4))
    try:
        np.minimum(  # E: Cannot evaluate type-level shape DSL call: Cannot broadcast dimension Int[4] with dimension Int[5] at position 1
            a, b
        )
    except ValueError:
        pass
    else:
        raise AssertionError("expected NumPy to reject incompatible shapes")


def test_binary_ufuncs_remain_positional_only() -> None:
    a = np.ones((3, 4))
    b = np.ones((3, 4))

    assert_shape(np.minimum(a, b).shape, (3, 4))
    try:
        np.minimum(  # E: No matching overload found
            x1=a,
            x2=b,
        )
    except TypeError:
        pass
    else:
        raise AssertionError("expected NumPy to require positional arguments")


def test_trig_preserves_shape() -> None:
    angles = np.ones((2, 3))

    assert_shape(np.sin(angles).shape, (2, 3))
    assert_shape(np.cos(angles).shape, (2, 3))
    assert_shape(np.tan(angles).shape, (2, 3))
    assert_shape(np.arcsin(np.full((2, 3), 0.5)).shape, (2, 3))


def test_rounding_preserves_shape() -> None:
    a = np.full(5, -1.7)

    assert_shape(np.floor(a).shape, (5,))
    assert_shape(np.ceil(a).shape, (5,))
    assert_shape(np.round(a).shape, (5,))
    assert_shape(np.round(a, 2).shape, (5,))
    assert_shape(np.round(a=a, decimals=-1).shape, (5,))
    assert_shape(np.trunc(a).shape, (5,))
    assert_shape(np.clip(a, -1.0, 2.0).shape, (5,))


def test_round_scalars_and_sequences() -> None:
    rounded_float = np.round(0.234, decimals=2)
    rounded_int = np.round(3)
    rounded_float32 = np.round(np.float32(1.25), 1)
    rounded_list = np.round([1.24, 2.56], decimals=1)
    assert_type(rounded_float, Any)
    assert_type(rounded_int, Any)
    assert_type(rounded_float32, np.float32)
    assert_type(rounded_list, np.ndarray)
    assert isinstance(rounded_float, np.float64)
    assert rounded_float == 0.23
    assert isinstance(rounded_int, np.int64)
    assert isinstance(rounded_float32, np.float32)
    assert rounded_list.tolist() == [1.2, 2.6]
    with assert_raises(TypeError):
        np.round(None)  # E: No matching overload


def test_round_array_dtypes() -> None:
    floats = np.ones((2, 3))
    integers = np.full((2, 3), 7, dtype=np.int8)
    complexes = np.full((2, 3), 1.25 + 2.5j, dtype=np.complex64)
    booleans = np.ones((2, 3), dtype=np.bool_)
    gradual_dtype = cast(
        "np.ndarray[tuple[Literal[2], Literal[3]]]", make_array((2, 3), 1.25)
    )
    unknown_rank = cast(np.ndarray, floats)

    assert_type(np.round(floats, 1), np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_type(np.round(integers, -1), np.ndarray[[2, 3], np.dtype[np.int8]])
    assert_type(
        np.round(complexes, decimals=1), np.ndarray[[2, 3], np.dtype[np.complex64]]
    )
    assert_type(np.round(booleans), np.ndarray[[2, 3], np.dtype[np.float16]])
    assert_type(np.round(gradual_dtype, 1), np.ndarray[[2, 3], Any])
    assert_shape(np.round(unknown_rank).shape, IntTuple, runtime=(2, 3))
    assert np.round(floats, 1).dtype == np.dtype(np.float64)
    assert np.round(integers, -1).tolist() == [[10, 10, 10], [10, 10, 10]]
    assert np.round(integers, -1).dtype == np.dtype(np.int8)
    assert np.round(complexes, decimals=1).dtype == np.dtype(np.complex64)
    assert np.round(booleans).dtype == np.dtype(np.float16)


def test_round_rank_zero_and_sequence_inputs() -> None:
    scalar_array = np.ones(())
    unknown_scalar = cast(np.ndarray, np.ones(()))

    rounded = np.round(scalar_array, 1)
    assert_type(rounded, Any)
    assert_type(np.round(unknown_scalar), Any)
    assert_shape(scalar_array.shape, ())
    assert isinstance(rounded, np.float64)
    assert isinstance(np.round(unknown_scalar), np.float64)

    rounded_tuple = np.round((1.24, 2.56), 1)
    nested = np.round([[1.25], [2.5]])
    assert_type(rounded_tuple, np.ndarray)
    assert_type(nested, np.ndarray)
    assert rounded_tuple.tolist() == [1.2, 2.6]
    assert nested.tolist() == [[1.0], [2.0]]
    with assert_raises(TypeError):
        np.round("1.5")  # E: No matching overload
    with assert_raises(TypeError):
        np.round(["1.5"])  # E: No matching overload


class ArrayUfuncDispatch:
    def __array__(self, dtype: Any = None, copy: Any = None) -> np.ndarray[[2], Any]:
        return np.ones(2)

    def __array_ufunc__(
        self, ufunc: Any, method: str, *inputs: Any, **kwargs: Any
    ) -> str:
        return "custom result"


class ExternalObject:
    pass


def test_unary_ufunc_external_dispatch() -> None:
    values = ArrayUfuncDispatch()
    signed = np.sign(values)
    logged = np.log1p(values)
    assert_type(signed, Any)
    assert_type(logged, Any)
    assert signed == logged == "custom result"
    assert_shape(values.__array__().shape, (2,))


def test_nan_to_num_external_objects() -> None:
    external = ExternalObject()
    union = cast("list[ExternalObject] | np.ndarray | ExternalObject", np.ones((2, 3)))
    result = np.nan_to_num(union)
    untouched = np.nan_to_num(external)
    assert_type(result, Any)
    assert_type(untouched, Any)
    assert untouched is external
    assert_shape(result.shape, IntTuple, runtime=(2, 3))


def test_clip_scalar_or_none_bounds() -> None:
    x = np.ones((2, 3))

    scalar = np.ones(())
    assert_type(np.clip(scalar, 0.0, None), Any)
    assert_type(np.clip(scalar, 0.0, 1.0), Any)
    assert isinstance(np.clip(scalar, 0.0, None), np.float64)
    assert isinstance(np.clip(scalar, 0.0, 1.0), np.float64)
    unknown_rank = cast(np.ndarray, scalar)
    assert_type(np.clip(unknown_rank, 0.0, None), Any)

    lower = np.clip(x, 0.0, None)
    upper = np.clip(x, None, 1.0)
    unbounded = np.clip(x, None, None)
    assert_type(lower, np.ndarray[[2, 3], Any])
    assert_type(upper, np.ndarray[[2, 3], Any])
    assert_type(unbounded, np.ndarray[[2, 3], Any])
    assert_shape(lower.shape, (2, 3))
    assert_shape(upper.shape, (2, 3))
    assert_shape(unbounded.shape, (2, 3))
    assert_shape(x.clip(0, None).shape, (2, 3))
    assert_type(np.clip(x, 0, 1), np.ndarray[[2, 3], Any])


def test_clip_keyword_bounds() -> None:
    x = np.ones((2, 3))

    lower = np.clip(a=x, a_min=0.0, a_max=None)
    upper = np.clip(x, a_max=1.0, a_min=None)
    assert_type(lower, np.ndarray[[2, 3], Any])
    assert_type(upper, np.ndarray[[2, 3], Any])
    assert_shape(lower.shape, (2, 3))
    assert_shape(upper.shape, (2, 3))


def test_clip_numpy_scalar_and_optional_bounds() -> None:
    x = np.ones((2, 3))
    bound = cast("float | None", 0.0)

    numpy_scalar = np.clip(x, np.float32(0.0), np.int64(1))
    optional = np.clip(x, bound, None)
    complex_scalar = np.clip(x, 0j, None)
    assert_type(numpy_scalar, np.ndarray[[2, 3], Any])
    assert_type(optional, np.ndarray[[2, 3], Any])
    assert_type(complex_scalar, np.ndarray[[2, 3], Any])
    assert_shape(numpy_scalar.shape, (2, 3))
    assert_shape(optional.shape, (2, 3))
    assert_shape(complex_scalar.shape, (2, 3))


def test_clip_array_bounds_remain_gradual() -> None:
    x = np.ones((2, 1))
    row = np.zeros((1, 3))

    lower = np.clip(x, row, None)
    upper = np.clip(a=x, a_min=None, a_max=row)
    sequence = np.clip(x, [0.0, 1.0, 2.0], None)
    assert_type(lower, Any)
    assert_type(upper, Any)
    assert_type(sequence, Any)
    assert_type(np.clip(cast(Any, x), 0.0, None), Any)
    assert_shape(lower.shape, IntTuple, runtime=(2, 3))
    assert_shape(upper.shape, IntTuple, runtime=(2, 3))
    assert_shape(sequence.shape, IntTuple, runtime=(2, 3))


def test_clip_float_bounds_can_promote_integer_dtype() -> None:
    x = np.ones((2, 3), dtype=np.int64)

    clipped = np.clip(x, 0.5, None)
    bounded = np.clip(x, 0.5, 1.5)
    assert_type(clipped, np.ndarray[[2, 3], Any])
    assert_type(bounded, np.ndarray[[2, 3], Any])
    assert_shape(clipped.shape, (2, 3))
    assert_shape(bounded.shape, (2, 3))
    assert clipped.dtype == np.dtype(np.float64)
    assert bounded.dtype == np.dtype(np.float64)


def test_nan_to_num_preserves_array_shape() -> None:
    vector = np.full(3, np.nan)
    matrix = np.ones((2, 3))

    assert_shape(np.nan_to_num(vector).shape, (3,))
    assert_shape(
        np.nan_to_num(matrix, copy=False, nan=0.0, posinf=1.0, neginf=-1.0).shape,
        (2, 3),
    )
    assert_type(np.nan_to_num(matrix).dtype, np.dtype[np.float64])
    assert_shape(np.nan_to_num([1.0, np.nan]).shape, IntTuple, runtime=(2,))
    assert np.nan_to_num(vector).tolist() == [0.0, 0.0, 0.0]


def test_nan_to_num_rank_zero_array_returns_scalar() -> None:
    scalar = np.ones(())

    string_scalar = np.str_("hello")
    assert_type(np.nan_to_num(string_scalar), Any)
    assert isinstance(np.nan_to_num(string_scalar), np.str_)

    assert_shape(scalar.shape, ())
    assert_type(np.nan_to_num(scalar), Any)
    assert isinstance(np.nan_to_num(scalar), np.float64)
