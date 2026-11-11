# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Any, Literal, overload

from numpy._shapes import (
    multivariate_normal_scalar_size_shape,
    multivariate_normal_shape,
)
from numpy._typing import (
    _ArrayLikeFloat_co,
    _ArrayLikeInt_co,
    _FloatLike_co,
    _ShapeLike,
    ArrayLike,
)
from numpy.random._generator import (
    _ArrayF32,
    _ArrayF64,
    _DTypeLikeF32,
    _DTypeLikeF64,
    _DTypeLikeFloat,
    _MethodExp,
    _NDArrayLikeFloat,
    Generator as _NumpyGenerator,
)

# Preserve NumPy's canonical re-exports before local shape-aware declarations.
from numpy.random._mt19937 import (
    MT19937 as MT19937,
)
from numpy.random._pcg64 import (
    PCG64 as PCG64,
    PCG64DXSM as PCG64DXSM,
)
from numpy.random._philox import (
    Philox as Philox,
)
from numpy.random._sfc64 import (
    SFC64 as SFC64,
)
from numpy.random.bit_generator import (
    BitGenerator as BitGenerator,
    SeedSequence as SeedSequence,
)
from numpy.random.mtrand import (
    beta as beta,
    binomial as binomial,
    bytes as bytes,
    chisquare as chisquare,
    choice as choice,
    dirichlet as dirichlet,
    exponential as exponential,
    f as f,
    gamma as gamma,
    geometric as geometric,
    get_state as get_state,
    gumbel as gumbel,
    hypergeometric as hypergeometric,
    laplace as laplace,
    logistic as logistic,
    lognormal as lognormal,
    logseries as logseries,
    multinomial as multinomial,
    multivariate_normal as multivariate_normal,
    negative_binomial as negative_binomial,
    noncentral_chisquare as noncentral_chisquare,
    noncentral_f as noncentral_f,
    normal as normal,
    pareto as pareto,
    permutation as permutation,
    poisson as poisson,
    power as power,
    rand as rand,
    randint as randint,
    random as random,
    random_integers as random_integers,
    random_sample as random_sample,
    RandomState as RandomState,
    ranf as ranf,
    rayleigh as rayleigh,
    sample as sample,
    seed as seed,
    set_state as set_state,
    shuffle as shuffle,
    standard_cauchy as standard_cauchy,
    standard_exponential as standard_exponential,
    standard_gamma as standard_gamma,
    standard_normal as standard_normal,
    standard_t as standard_t,
    triangular as triangular,
    uniform as uniform,
    vonmises as vonmises,
    wald as wald,
    weibull as weibull,
    zipf as zipf,
)
from shape_extensions import broadcast, Int, IntTuple, IntVar

from .. import dtype, float32, float64, ndarray

@overload
def randn() -> float: ...
@overload
def randn[Shape: IntTuple](*dims: *Shape) -> ndarray[Shape, dtype[float64]]: ...

# Subclasses NumPy's `Generator` so that only the overridden methods change and
# every other method keeps NumPy's own annotation.
class Generator(_NumpyGenerator):
    @overload
    def random(
        self, size: None = None, dtype: _DTypeLikeFloat = ..., out: None = None
    ) -> float: ...
    @overload
    def random[N: IntVar](
        self, size: Int[N], dtype: _DTypeLikeF64 = ..., out: None = None
    ) -> ndarray[[N], dtype[float64]]: ...
    @overload
    def random[Shape: IntTuple](
        self, size: Shape, dtype: _DTypeLikeF64 = ..., out: None = None
    ) -> ndarray[Shape, dtype[float64]]: ...
    @overload
    def random[N: IntVar](
        self, size: Int[N], dtype: _DTypeLikeF32, out: None = None
    ) -> ndarray[[N], dtype[float32]]: ...
    @overload
    def random[Shape: IntTuple](
        self, size: Shape, dtype: _DTypeLikeF32, out: None = None
    ) -> ndarray[Shape, dtype[float32]]: ...
    @overload
    def random(
        self, size: _ShapeLike, dtype: _DTypeLikeF64 = ..., out: None = None
    ) -> _ArrayF64: ...
    @overload
    def random(
        self, size: _ShapeLike, dtype: _DTypeLikeF32, out: None = None
    ) -> _ArrayF32: ...
    @overload
    def random[ArrayT: _ArrayF64](
        self, size: _ShapeLike | None = None, dtype: _DTypeLikeF64 = ..., *, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def random[ArrayT: _ArrayF32](
        self, size: _ShapeLike | None = None, *, dtype: _DTypeLikeF32, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def random[ArrayT: _ArrayF64](
        self, size: _ShapeLike | None, dtype: _DTypeLikeF64, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def random[ArrayT: _ArrayF32](
        self, size: _ShapeLike | None, dtype: _DTypeLikeF32, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def standard_cauchy(self, size: None = None) -> float: ...
    @overload
    def standard_cauchy[N: IntVar](
        self, size: Int[N]
    ) -> ndarray[[N], dtype[float64]]: ...
    @overload
    def standard_cauchy[Shape: IntTuple](
        self, size: Shape
    ) -> ndarray[Shape, dtype[float64]]: ...
    @overload
    def standard_cauchy(self, size: _ShapeLike) -> _ArrayF64: ...
    @overload
    def multivariate_normal[MeanShape: IntTuple, CovShape: IntTuple](
        self,
        /,
        mean: ndarray[MeanShape],
        cov: ndarray[CovShape],
        size: None = None,
        check_valid: Literal["warn", "raise", "ignore"] = "warn",
        tol: float = 1e-8,
        *,
        method: Literal["svd", "eigh", "cholesky"] = "svd",
    ) -> ndarray[
        multivariate_normal_scalar_size_shape(MeanShape, CovShape, None), dtype[float64]
    ]: ...
    @overload
    def multivariate_normal[MeanShape: IntTuple, CovShape: IntTuple, N: IntVar](
        self,
        /,
        mean: ndarray[MeanShape],
        cov: ndarray[CovShape],
        size: Int[N],
        check_valid: Literal["warn", "raise", "ignore"] = "warn",
        tol: float = 1e-8,
        *,
        method: Literal["svd", "eigh", "cholesky"] = "svd",
    ) -> ndarray[
        multivariate_normal_scalar_size_shape(MeanShape, CovShape, Int[N]),
        dtype[float64],
    ]: ...
    @overload
    def multivariate_normal[MeanShape: IntTuple, CovShape: IntTuple, Size: IntTuple](
        self,
        /,
        mean: ndarray[MeanShape],
        cov: ndarray[CovShape],
        size: Size,
        check_valid: Literal["warn", "raise", "ignore"] = "warn",
        tol: float = 1e-8,
        *,
        method: Literal["svd", "eigh", "cholesky"] = "svd",
    ) -> ndarray[
        multivariate_normal_shape(MeanShape, CovShape, Size), dtype[float64]
    ]: ...
    @overload
    def multivariate_normal(
        self,
        /,
        mean: ArrayLike,
        cov: ArrayLike,
        size: _ShapeLike | None = None,
        check_valid: Literal["warn", "raise", "ignore"] = "warn",
        tol: float = 1e-8,
        *,
        method: Literal["svd", "eigh", "cholesky"] = "svd",
    ) -> ndarray[IntTuple, dtype[float64]]: ...
    @overload
    def standard_normal(
        self, size: None = None, dtype: _DTypeLikeFloat = ..., out: None = None
    ) -> float: ...
    @overload
    def standard_normal[N: IntVar](
        self, size: Int[N], dtype: _DTypeLikeF64 = ..., out: None = None
    ) -> ndarray[[N], dtype[float64]]: ...
    @overload
    def standard_normal[Shape: IntTuple](
        self, size: Shape, dtype: _DTypeLikeF64 = ..., out: None = None
    ) -> ndarray[Shape, dtype[float64]]: ...
    @overload
    def standard_normal[N: IntVar](
        self, size: Int[N], dtype: _DTypeLikeF32, *, out: None = None
    ) -> ndarray[[N], dtype[float32]]: ...
    @overload
    def standard_normal[Shape: IntTuple](
        self, size: Shape, dtype: _DTypeLikeF32, *, out: None = None
    ) -> ndarray[Shape, dtype[float32]]: ...
    @overload
    def standard_normal(
        self, size: _ShapeLike, dtype: _DTypeLikeF64 = ..., out: None = None
    ) -> _ArrayF64: ...
    @overload
    def standard_normal(
        self, size: _ShapeLike, dtype: _DTypeLikeF32, *, out: None = None
    ) -> _ArrayF32: ...
    @overload
    def standard_normal[ArrayT: _ArrayF64](
        self, size: _ShapeLike | None = None, dtype: _DTypeLikeF64 = ..., *, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def standard_normal[ArrayT: _ArrayF32](
        self, size: _ShapeLike | None = None, *, dtype: _DTypeLikeF32, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def standard_normal[ArrayT: _ArrayF32](
        self, size: _ShapeLike | None, dtype: _DTypeLikeF32, out: ArrayT
    ) -> ArrayT: ...
    @overload
    def standard_exponential(
        self,
        size: None = None,
        dtype: _DTypeLikeFloat = ...,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> float: ...
    @overload
    def standard_exponential[N: IntVar](
        self,
        size: Int[N],
        dtype: _DTypeLikeF64 = ...,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> ndarray[[N], dtype[float64]]: ...
    @overload
    def standard_exponential[Shape: IntTuple](
        self,
        size: Shape,
        dtype: _DTypeLikeF64 = ...,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> ndarray[Shape, dtype[float64]]: ...
    @overload
    def standard_exponential[N: IntVar](
        self,
        size: Int[N],
        dtype: _DTypeLikeF32,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> ndarray[[N], dtype[float32]]: ...
    @overload
    def standard_exponential[Shape: IntTuple](
        self,
        size: Shape,
        dtype: _DTypeLikeF32,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> ndarray[Shape, dtype[float32]]: ...
    @overload
    def standard_exponential(
        self,
        size: _ShapeLike,
        dtype: _DTypeLikeF64 = ...,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> _ArrayF64: ...
    @overload
    def standard_exponential(
        self,
        size: _ShapeLike,
        dtype: _DTypeLikeF32,
        method: _MethodExp = "zig",
        out: None = None,
    ) -> _ArrayF32: ...
    @overload
    def standard_exponential[ArrayT: _ArrayF64](
        self,
        size: _ShapeLike | None = None,
        dtype: _DTypeLikeF64 = ...,
        method: _MethodExp = "zig",
        *,
        out: ArrayT,
    ) -> ArrayT: ...
    @overload
    def standard_exponential[ArrayT: _ArrayF32](
        self,
        size: _ShapeLike | None = None,
        *,
        dtype: _DTypeLikeF32,
        method: _MethodExp = "zig",
        out: ArrayT,
    ) -> ArrayT: ...
    @overload
    def standard_exponential[ArrayT: _ArrayF32](
        self,
        size: _ShapeLike | None,
        dtype: _DTypeLikeF32,
        method: _MethodExp = "zig",
        *,
        out: ArrayT,
    ) -> ArrayT: ...
    @overload
    def normal(
        self, /, loc: _FloatLike_co = 0.0, scale: _FloatLike_co = 1.0, size: None = None
    ) -> float: ...
    # With `size`, the result has exactly that shape, and NumPy raises if `loc` or
    # `scale` does not broadcast to it.
    @overload
    def normal[N: IntVar](
        self, /, loc: _ArrayLikeFloat_co, scale: _ArrayLikeFloat_co, size: Int[N]
    ) -> ndarray[[N], dtype[float64]]: ...
    @overload
    def normal[Shape: IntTuple](
        self, /, loc: _ArrayLikeFloat_co, scale: _ArrayLikeFloat_co, size: Shape
    ) -> ndarray[Shape, dtype[float64]]: ...
    @overload
    def normal[N: IntVar](
        self,
        /,
        loc: _ArrayLikeFloat_co = 0.0,
        scale: _ArrayLikeFloat_co = 1.0,
        *,
        size: Int[N],
    ) -> ndarray[[N], dtype[float64]]: ...
    @overload
    def normal[Shape: IntTuple](
        self,
        /,
        loc: _ArrayLikeFloat_co = 0.0,
        scale: _ArrayLikeFloat_co = 1.0,
        *,
        size: Shape,
    ) -> ndarray[Shape, dtype[float64]]: ...
    # Without `size`, the result has the broadcast shape of `loc` and `scale`.
    @overload
    def normal[LocShape: IntTuple = [], ScaleShape: IntTuple = []](
        self,
        /,
        loc: ndarray[LocShape] | float = 0.0,
        scale: ndarray[ScaleShape] | float = 1.0,
        size: None = None,
    ) -> ndarray[broadcast(LocShape, ScaleShape), dtype[float64]]: ...
    @overload
    def normal(
        self, /, loc: _ArrayLikeFloat_co, scale: _ArrayLikeFloat_co, size: _ShapeLike
    ) -> _ArrayF64: ...
    @overload
    def normal(
        self, /, loc: _ArrayLikeFloat_co, scale: _NDArrayLikeFloat, size: None
    ) -> _ArrayF64: ...
    @overload
    def normal(
        self,
        /,
        loc: _ArrayLikeFloat_co = 0.0,
        scale: _ArrayLikeFloat_co = 1.0,
        *,
        size: _ShapeLike,
    ) -> _ArrayF64: ...
    @overload
    def normal(
        self,
        /,
        loc: _ArrayLikeFloat_co = 0.0,
        *,
        scale: _NDArrayLikeFloat,
        size: None = None,
    ) -> _ArrayF64: ...
    @overload
    def normal(
        self,
        /,
        loc: _NDArrayLikeFloat,
        scale: _ArrayLikeFloat_co = 1.0,
        size: None = None,
    ) -> _ArrayF64: ...
    @overload
    def normal(
        self,
        /,
        loc: _ArrayLikeFloat_co = 0.0,
        scale: _ArrayLikeFloat_co = 1.0,
        size: None = None,
    ) -> _ArrayF64 | Any: ...

def default_rng(
    seed: _ArrayLikeInt_co
    | SeedSequence
    | BitGenerator
    | _NumpyGenerator
    | RandomState
    | None = None,
) -> Generator: ...
