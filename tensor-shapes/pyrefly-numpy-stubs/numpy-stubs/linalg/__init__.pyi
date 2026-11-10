# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from collections.abc import Sequence
from typing import Any, Literal, overload

from numpy._shapes import int_min, reduce_shape

# Preserve NumPy's canonical re-exports before local shape-aware declarations.
from numpy.linalg._linalg import (
    cholesky as cholesky,
    cond as cond,
    cross as cross,
    det as det,
    diagonal as diagonal,
    eig as eig,
    eigvals as eigvals,
    eigvalsh as eigvalsh,
    inv as inv,
    lstsq as lstsq,
    matmul as matmul,
    matrix_norm as matrix_norm,
    matrix_power as matrix_power,
    matrix_rank as matrix_rank,
    matrix_transpose as matrix_transpose,
    multi_dot as multi_dot,
    outer as outer,
    pinv as pinv,
    qr as qr,
    slogdet as slogdet,
    svdvals as svdvals,
    tensordot as tensordot,
    tensorinv as tensorinv,
    tensorsolve as tensorsolve,
    trace as trace,
    vecdot as vecdot,
    vector_norm as vector_norm,
)
from shape_extensions import Flag, Int, IntTuple, IntVar

from .. import complex64, complex128, dtype, float32, float64, floating, ndarray

class LinAlgError(ValueError): ...

type _UPLO = Literal["L", "U", "l", "u"]

# MVP shape surface only; NumPy dtype promotion is intentionally not modeled.
@overload
def solve[N: IntVar, DType](
    a: ndarray[[N, N], DType],
    b: ndarray[[N]],
) -> ndarray[[N], DType]: ...
@overload
def solve[N: IntVar, K: IntVar, DType](
    a: ndarray[[N, N], DType],
    b: ndarray[[N, K]],
) -> ndarray[[N, K], DType]: ...
@overload
def norm[N: IntVar, M: IntVar, DType](
    x: ndarray[[N, M, 3], DType],
    ord: None,
    axis: Literal[-1],
    keepdims: Literal[True],
) -> ndarray[[N, M, 1], DType]: ...
@overload
def norm[N: IntVar, M: IntVar, DType](
    x: ndarray[[N, M, 3], DType],
    ord: None = None,
    *,
    axis: Literal[-1],
    keepdims: Literal[True],
) -> ndarray[[N, M, 1], DType]: ...
@overload
def norm(
    x: ndarray,
    ord: Any = None,
    axis: None = None,
    keepdims: Literal[False] = False,
    **kwargs: Any,
) -> floating: ...
@overload
def norm[Shape: IntTuple, Axis: Flag[int | tuple[int, ...] | None], Keep: Flag[bool]](
    x: ndarray[Shape],
    ord: int | float | Literal["fro", "nuc"] | None = None,
    axis: Axis = None,
    keepdims: Keep = False,
) -> ndarray[reduce_shape(Shape, Axis, Keep)]: ...
@overload
def norm(
    x: Sequence[int | float | complex],
    ord: int | float | None = None,
    axis: None = None,
    keepdims: Literal[False] = False,
) -> floating: ...

# Hermitian eigenvalues are real, so complex inputs produce floating eigenvalues of
# the same precision. Integer inputs promote, so their dtypes stay gradual. Leading
# batch dimensions are preserved.
@overload
def eigh[N: IntVar, ScalarT: float32 | complex64, Batch: IntTuple = []](
    a: ndarray[[*Batch, N, N], dtype[ScalarT]],
    UPLO: _UPLO = "L",
) -> tuple[
    ndarray[[*Batch, N], dtype[float32]], ndarray[[*Batch, N, N], dtype[ScalarT]]
]: ...
@overload
def eigh[N: IntVar, ScalarT: float64 | complex128, Batch: IntTuple = []](
    a: ndarray[[*Batch, N, N], dtype[ScalarT]],
    UPLO: _UPLO = "L",
) -> tuple[
    ndarray[[*Batch, N], dtype[float64]], ndarray[[*Batch, N, N], dtype[ScalarT]]
]: ...
@overload
def eigh[N: IntVar, Batch: IntTuple = []](
    a: ndarray[[*Batch, N, N]],
    UPLO: _UPLO = "L",
) -> tuple[ndarray[[*Batch, N]], ndarray[[*Batch, N, N]]]: ...

# Singular values are real, so complex inputs produce floating singular values of the
# same precision, while `U` and `Vh` keep the input dtype. Integer inputs promote, so
# their dtypes stay gradual. NumPy computes full matrices by default; with
# `compute_uv=False`, it returns only the singular values. Leading batch dimensions
# are preserved.
@overload
def svd[M: IntVar, N: IntVar, ScalarT: float32 | complex64, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N], dtype[ScalarT]],
    full_matrices: Literal[True] = True,
    compute_uv: Literal[True] = True,
    hermitian: Literal[False] = False,
) -> tuple[
    ndarray[[*Batch, M, M], dtype[ScalarT]],
    ndarray[[*Batch, int_min(Int[M], Int[N])], dtype[float32]],
    ndarray[[*Batch, N, N], dtype[ScalarT]],
]: ...
@overload
def svd[M: IntVar, N: IntVar, ScalarT: float64 | complex128, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N], dtype[ScalarT]],
    full_matrices: Literal[True] = True,
    compute_uv: Literal[True] = True,
    hermitian: Literal[False] = False,
) -> tuple[
    ndarray[[*Batch, M, M], dtype[ScalarT]],
    ndarray[[*Batch, int_min(Int[M], Int[N])], dtype[float64]],
    ndarray[[*Batch, N, N], dtype[ScalarT]],
]: ...
@overload
def svd[M: IntVar, N: IntVar, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N]],
    full_matrices: Literal[True] = True,
    compute_uv: Literal[True] = True,
    hermitian: Literal[False] = False,
) -> tuple[
    ndarray[[*Batch, M, M]],
    ndarray[[*Batch, int_min(Int[M], Int[N])]],
    ndarray[[*Batch, N, N]],
]: ...
@overload
def svd[M: IntVar, N: IntVar, ScalarT: float32 | complex64, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N], dtype[ScalarT]],
    full_matrices: Literal[False],
    compute_uv: Literal[True] = True,
    hermitian: Literal[False] = False,
) -> tuple[
    ndarray[[*Batch, M, int_min(Int[M], Int[N])], dtype[ScalarT]],
    ndarray[[*Batch, int_min(Int[M], Int[N])], dtype[float32]],
    ndarray[[*Batch, int_min(Int[M], Int[N]), N], dtype[ScalarT]],
]: ...
@overload
def svd[M: IntVar, N: IntVar, ScalarT: float64 | complex128, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N], dtype[ScalarT]],
    full_matrices: Literal[False],
    compute_uv: Literal[True] = True,
    hermitian: Literal[False] = False,
) -> tuple[
    ndarray[[*Batch, M, int_min(Int[M], Int[N])], dtype[ScalarT]],
    ndarray[[*Batch, int_min(Int[M], Int[N])], dtype[float64]],
    ndarray[[*Batch, int_min(Int[M], Int[N]), N], dtype[ScalarT]],
]: ...
@overload
def svd[M: IntVar, N: IntVar, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N]],
    full_matrices: Literal[False],
    compute_uv: Literal[True] = True,
    hermitian: Literal[False] = False,
) -> tuple[
    ndarray[[*Batch, M, int_min(Int[M], Int[N])]],
    ndarray[[*Batch, int_min(Int[M], Int[N])]],
    ndarray[[*Batch, int_min(Int[M], Int[N]), N]],
]: ...
@overload
def svd[M: IntVar, N: IntVar, ScalarT: float32 | complex64, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N], dtype[ScalarT]],
    full_matrices: bool = True,
    *,
    compute_uv: Literal[False],
    hermitian: Literal[False] = False,
) -> ndarray[[*Batch, int_min(Int[M], Int[N])], dtype[float32]]: ...
@overload
def svd[M: IntVar, N: IntVar, ScalarT: float64 | complex128, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N], dtype[ScalarT]],
    full_matrices: bool = True,
    *,
    compute_uv: Literal[False],
    hermitian: Literal[False] = False,
) -> ndarray[[*Batch, int_min(Int[M], Int[N])], dtype[float64]]: ...
@overload
def svd[M: IntVar, N: IntVar, Batch: IntTuple = []](
    a: ndarray[[*Batch, M, N]],
    full_matrices: bool = True,
    *,
    compute_uv: Literal[False],
    hermitian: Literal[False] = False,
) -> ndarray[[*Batch, int_min(Int[M], Int[N])]]: ...
