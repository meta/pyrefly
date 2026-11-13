# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import assert_type

import numpy as np
from shape_extensions import assert_raises, assert_shape, Int, IntTuple, IntVar


def test_randn_1d_shape() -> None:
    assert_shape(np.random.randn(5).shape, (5,))


def test_randn_2d_shape() -> None:
    assert_shape(np.random.randn(5, 3).shape, (5, 3))


def test_randn_2d_singleton_dimension_shape() -> None:
    assert_shape(np.random.randn(5, 1).shape, (5, 1))


def test_randn_3d_shape() -> None:
    result = np.random.randn(5, 3, 2)
    assert_type(result, np.ndarray[[5, 3, 2], np.dtype[np.float64]])
    assert_shape(result.shape, (5, 3, 2))


def test_randn_4d_shape() -> None:
    result = np.random.randn(2, 3, 4, 5)
    assert_type(result, np.ndarray[[2, 3, 4, 5], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3, 4, 5))


def check_randn_no_args_returns_float() -> None:
    assert_type(np.random.randn(), float)


def check_standard_normal_and_standard_exponential_return_float_without_size() -> None:
    rng = np.random.default_rng(0)
    assert_type(rng.standard_normal(), float)
    assert_type(rng.standard_exponential(), float)


def test_standard_normal_and_standard_exponential_int_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.standard_normal(5)
    assert_type(result, np.ndarray[[5], np.dtype[np.float64]])
    assert_shape(result.shape, (5,))
    assert_shape(rng.standard_exponential(5).shape, (5,))


def test_standard_normal_and_standard_exponential_tuple_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.standard_exponential((2, 3))
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.standard_normal((2, 3)).shape, (2, 3))


def test_standard_normal_and_standard_exponential_float32() -> None:
    rng = np.random.default_rng(0)
    result = rng.standard_normal((2, 3), dtype=np.float32)
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float32]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.standard_exponential((2, 3), np.float32).shape, (2, 3))


def test_standard_normal_and_standard_exponential_out() -> None:
    rng = np.random.default_rng(0)
    out = np.empty((2, 3))
    result = rng.standard_normal(out=out)
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.standard_exponential(out=out).shape, (2, 3))


def test_standard_normal_and_standard_exponential_list_size() -> None:
    rng = np.random.default_rng(0)
    assert_shape(rng.standard_normal([2, 3]).shape, IntTuple, runtime=(2, 3))
    assert_shape(rng.standard_exponential([2, 3]).shape, IntTuple, runtime=(2, 3))


def check_standard_normal_and_standard_exponential_symbolic_size[N: IntVar, M: IntVar](
    n: Int[N], m: Int[M]
) -> None:
    rng = np.random.default_rng(0)
    assert_type(rng.standard_normal(n), np.ndarray[[N], np.dtype[np.float64]])
    assert_type(
        rng.standard_exponential((n, m)), np.ndarray[[N, M], np.dtype[np.float64]]
    )


def check_normal_returns_float_without_size() -> None:
    rng = np.random.default_rng(0)
    assert_type(rng.normal(), float)
    assert_type(rng.normal(1.0, 2.0), float)


def test_normal_int_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.normal(size=5)
    assert_type(result, np.ndarray[[5], np.dtype[np.float64]])
    assert_shape(result.shape, (5,))
    assert_shape(rng.normal(0.0, 1.0, 5).shape, (5,))


def test_normal_tuple_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.normal(0.0, 1.0, (2, 3))
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.normal(size=(2, 3)).shape, (2, 3))


def test_normal_array_parameters_with_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.normal(np.zeros(3), 1.0, size=(2, 3))
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.normal(np.zeros((10, 1)), size=(10, 5)).shape, (10, 5))


def test_normal_array_parameters_without_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.normal(np.zeros(3), np.ones((2, 1)))
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.normal(np.zeros(3)).shape, (3,))
    assert_shape(rng.normal(scale=np.ones((2, 1))).shape, (2, 1))


def test_normal_list_size_and_parameters() -> None:
    rng = np.random.default_rng(0)
    assert_shape(rng.normal(size=[2, 3]).shape, IntTuple, runtime=(2, 3))
    assert_shape(rng.normal([0.0, 1.0, 2.0]).shape, IntTuple, runtime=(3,))


def test_normal_mismatched_parameters() -> None:
    rng = np.random.default_rng(0)
    with assert_raises(ValueError):
        rng.normal(np.zeros(3), np.ones(2))  # E: Cannot broadcast


def check_normal_symbolic_shapes[N: IntVar, M: IntVar](
    n: Int[N], loc: np.ndarray[[N, 1]], scale: np.ndarray[[M]]
) -> None:
    rng = np.random.default_rng(0)
    assert_type(rng.normal(size=n), np.ndarray[[N], np.dtype[np.float64]])
    assert_type(rng.normal(loc, scale), np.ndarray[[N, M], np.dtype[np.float64]])


def test_multivariate_normal_without_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.multivariate_normal(np.zeros(3), np.eye(3))
    assert_type(result, np.ndarray[[3], np.dtype[np.float64]])
    assert_shape(result.shape, (3,))


def test_multivariate_normal_int_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.multivariate_normal(np.zeros(3), np.eye(3), 5)
    assert_type(result, np.ndarray[[5, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (5, 3))


def test_multivariate_normal_tuple_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.multivariate_normal(np.zeros(3), np.eye(3), (5, 2))
    assert_type(result, np.ndarray[[5, 2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (5, 2, 3))


def test_multivariate_normal_unshaped_parameters() -> None:
    rng = np.random.default_rng(0)
    result = rng.multivariate_normal([0.0, 0.0, 0.0], np.eye(3).tolist(), 5)
    assert_type(result, np.ndarray[IntTuple, np.dtype[np.float64]])
    assert_shape(result.shape, IntTuple, runtime=(5, 3))


def test_multivariate_normal_mismatched_parameters() -> None:
    rng = np.random.default_rng(0)
    with assert_raises(ValueError):
        rng.multivariate_normal(np.zeros(3), np.eye(2))  # E: same length


def test_multivariate_normal_two_dimensional_mean() -> None:
    rng = np.random.default_rng(0)
    with assert_raises(ValueError):
        rng.multivariate_normal(np.zeros((2, 3)), np.eye(3))  # E: must be 1-D


def test_multivariate_normal_non_square_covariance() -> None:
    rng = np.random.default_rng(0)
    with assert_raises(ValueError):
        rng.multivariate_normal(np.zeros(3), np.zeros((3, 4)))  # E: same length


def check_multivariate_normal_symbolic_size[N: IntVar, M: IntVar](
    mean: np.ndarray[[M], np.dtype[np.float64]],
    cov: np.ndarray[[M, M], np.dtype[np.float64]],
    n: Int[N],
) -> None:
    rng = np.random.default_rng(0)
    assert_type(
        rng.multivariate_normal(mean, cov, n),
        np.ndarray[[N, M], np.dtype[np.float64]],
    )


def test_multivariate_normal_keyword_arguments() -> None:
    rng = np.random.default_rng(0)
    result = rng.multivariate_normal(
        np.zeros(3), np.eye(3), 5, check_valid="raise", method="cholesky"
    )
    assert_type(result, np.ndarray[[5, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (5, 3))


def check_random_and_standard_cauchy_return_float_without_size() -> None:
    rng = np.random.default_rng(0)
    assert_type(rng.random(), float)
    assert_type(rng.standard_cauchy(), float)


def test_random_and_standard_cauchy_int_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.random(5)
    assert_type(result, np.ndarray[[5], np.dtype[np.float64]])
    assert_shape(result.shape, (5,))
    assert_shape(rng.standard_cauchy(5).shape, (5,))


def test_random_and_standard_cauchy_tuple_size() -> None:
    rng = np.random.default_rng(0)
    result = rng.standard_cauchy((2, 3))
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.random((2, 3)).shape, (2, 3))


def test_random_float32() -> None:
    rng = np.random.default_rng(0)
    result = rng.random((2, 3), dtype=np.float32)
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float32]])
    assert_shape(result.shape, (2, 3))
    assert_shape(rng.random(4, np.float32).shape, (4,))


def test_random_out() -> None:
    rng = np.random.default_rng(0)
    out = np.empty((2, 3))
    result = rng.random(out=out)
    assert_type(result, np.ndarray[[2, 3], np.dtype[np.float64]])
    assert_shape(result.shape, (2, 3))


def test_random_list_size() -> None:
    rng = np.random.default_rng(0)
    assert_shape(rng.random([2, 3]).shape, IntTuple, runtime=(2, 3))


def check_random_and_standard_cauchy_symbolic_size[N: IntVar, M: IntVar](
    n: Int[N], m: Int[M]
) -> None:
    rng = np.random.default_rng(0)
    assert_type(rng.random(n), np.ndarray[[N], np.dtype[np.float64]])
    assert_type(rng.standard_cauchy((n, m)), np.ndarray[[N, M], np.dtype[np.float64]])
