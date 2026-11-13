# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from collections.abc import Sequence
from typing import Any, overload, Protocol

import numpy as np
from jax import Array as JaxArray
from shape_extensions import CaptureNamedInts, Flag, IntTuple, NamedInts
from torch import Tensor

from ._shapes import einsum_shape, rearrange_shape, reduce_shape, repeat_shape

__version__: str

class _Array[Shape: IntTuple = IntTuple](Protocol):
    @property
    def shape(self) -> Shape: ...

class _NumpyResult[Shape: IntTuple = IntTuple](_Array[Shape], Protocol):
    # NumPy reductions and einsum may return either arrays or array scalars.
    @property
    def dtype(self) -> np.dtype[Any]: ...
    def __array__(self) -> np.ndarray[Shape, Any]: ...

class EinopsError(RuntimeError): ...

def asnumpy[Shape: IntTuple](tensor: _Array[Shape]) -> _Array[Shape]: ...
def parse_shape(tensor: Tensor, pattern: str) -> dict[str, int]: ...

# TODO: Add shape primitives for the `*` packing pattern so these return precise shapes.
def pack(
    tensors: Sequence[Tensor], pattern: str
) -> tuple[Tensor[IntTuple], list[tuple[int, ...]]]: ...
def unpack(
    tensor: Tensor,
    packed_shapes: Sequence[Sequence[int]],
    pattern: str,
) -> list[Tensor[IntTuple]]: ...
@overload
def rearrange[Shape: IntTuple, Pattern: Flag[str], Axes: NamedInts](
    tensor: Tensor[Shape], pattern: Pattern, **axes_lengths: CaptureNamedInts[Axes]
) -> Tensor[rearrange_shape(Pattern, Shape, Axes)]: ...
@overload
def rearrange[Shape: IntTuple, Pattern: Flag[str], Axes: NamedInts](
    tensor: JaxArray[Shape], pattern: Pattern, **axes_lengths: CaptureNamedInts[Axes]
) -> JaxArray[rearrange_shape(Pattern, Shape, Axes)]: ...
@overload
def rearrange[Shape: IntTuple, DType, Pattern: Flag[str], Axes: NamedInts](
    tensor: np.ndarray[Shape, DType],
    pattern: Pattern,
    **axes_lengths: CaptureNamedInts[Axes],
) -> np.ndarray[rearrange_shape(Pattern, Shape, Axes), DType]: ...
@overload
def rearrange(tensor: Any, pattern: str, **axes_lengths: int) -> Any: ...
@overload
def reduce[Shape: IntTuple, Pattern: Flag[str], Axes: NamedInts](
    tensor: Tensor[Shape],
    pattern: Pattern,
    reduction: Any,
    **axes_lengths: CaptureNamedInts[Axes],
) -> Tensor[reduce_shape(Pattern, Shape, Axes)]: ...
@overload
def reduce[Shape: IntTuple, Pattern: Flag[str], Axes: NamedInts](
    tensor: JaxArray[Shape],
    pattern: Pattern,
    reduction: Any,
    **axes_lengths: CaptureNamedInts[Axes],
) -> JaxArray[reduce_shape(Pattern, Shape, Axes)]: ...
@overload
def reduce[
    Shape: IntTuple,
    Scalar: np.number | np.bool_,
    Pattern: Flag[str],
    Axes: NamedInts,
](
    tensor: np.ndarray[Shape, np.dtype[Scalar]],
    pattern: Pattern,
    reduction: Any,
    **axes_lengths: CaptureNamedInts[Axes],
) -> _NumpyResult[reduce_shape(Pattern, Shape, Axes)]: ...
@overload
def reduce(tensor: Any, pattern: str, reduction: Any, **axes_lengths: int) -> Any: ...
@overload
def repeat[Shape: IntTuple, Pattern: Flag[str], Axes: NamedInts](
    tensor: Tensor[Shape], pattern: Pattern, **axes_lengths: CaptureNamedInts[Axes]
) -> Tensor[repeat_shape(Pattern, Shape, Axes)]: ...
@overload
def repeat[Shape: IntTuple, Pattern: Flag[str], Axes: NamedInts](
    tensor: JaxArray[Shape], pattern: Pattern, **axes_lengths: CaptureNamedInts[Axes]
) -> JaxArray[repeat_shape(Pattern, Shape, Axes)]: ...
@overload
def repeat[Shape: IntTuple, DType, Pattern: Flag[str], Axes: NamedInts](
    tensor: np.ndarray[Shape, DType],
    pattern: Pattern,
    **axes_lengths: CaptureNamedInts[Axes],
) -> np.ndarray[repeat_shape(Pattern, Shape, Axes), DType]: ...
@overload
def repeat(tensor: Any, pattern: str, **axes_lengths: int) -> Any: ...
@overload
def einsum[S1: IntTuple, Pattern: Flag[str]](
    tensor: Tensor[S1], pattern: Pattern, /
) -> Tensor[einsum_shape(Pattern, tuple[S1])]: ...
@overload
def einsum[S1: IntTuple, S2: IntTuple, Pattern: Flag[str]](
    tensor1: Tensor[S1], tensor2: Tensor[S2], pattern: Pattern, /
) -> Tensor[einsum_shape(Pattern, tuple[S1, S2])]: ...
@overload
def einsum[S1: IntTuple, S2: IntTuple, S3: IntTuple, Pattern: Flag[str]](
    tensor1: Tensor[S1],
    tensor2: Tensor[S2],
    tensor3: Tensor[S3],
    pattern: Pattern,
    /,
) -> Tensor[einsum_shape(Pattern, tuple[S1, S2, S3])]: ...
@overload
def einsum[
    S1: IntTuple,
    S2: IntTuple,
    S3: IntTuple,
    S4: IntTuple,
    Pattern: Flag[str],
](
    tensor1: Tensor[S1],
    tensor2: Tensor[S2],
    tensor3: Tensor[S3],
    tensor4: Tensor[S4],
    pattern: Pattern,
    /,
) -> Tensor[einsum_shape(Pattern, tuple[S1, S2, S3, S4])]: ...
@overload
def einsum[S1: IntTuple, Pattern: Flag[str]](
    tensor: JaxArray[S1], pattern: Pattern, /
) -> JaxArray[einsum_shape(Pattern, tuple[S1])]: ...
@overload
def einsum[S1: IntTuple, S2: IntTuple, Pattern: Flag[str]](
    tensor1: JaxArray[S1], tensor2: JaxArray[S2], pattern: Pattern, /
) -> JaxArray[einsum_shape(Pattern, tuple[S1, S2])]: ...
@overload
def einsum[S1: IntTuple, S2: IntTuple, S3: IntTuple, Pattern: Flag[str]](
    tensor1: JaxArray[S1],
    tensor2: JaxArray[S2],
    tensor3: JaxArray[S3],
    pattern: Pattern,
    /,
) -> JaxArray[einsum_shape(Pattern, tuple[S1, S2, S3])]: ...
@overload
def einsum[
    S1: IntTuple,
    S2: IntTuple,
    S3: IntTuple,
    S4: IntTuple,
    Pattern: Flag[str],
](
    tensor1: JaxArray[S1],
    tensor2: JaxArray[S2],
    tensor3: JaxArray[S3],
    tensor4: JaxArray[S4],
    pattern: Pattern,
    /,
) -> JaxArray[einsum_shape(Pattern, tuple[S1, S2, S3, S4])]: ...
@overload
def einsum[S1: IntTuple, Scalar: np.number | np.bool_, Pattern: Flag[str]](
    tensor: np.ndarray[S1, np.dtype[Scalar]], pattern: Pattern, /
) -> _NumpyResult[einsum_shape(Pattern, tuple[S1])]: ...
@overload
def einsum[
    S1: IntTuple,
    S2: IntTuple,
    Scalar: np.number | np.bool_,
    Pattern: Flag[str],
](
    tensor1: np.ndarray[S1, np.dtype[Scalar]],
    tensor2: np.ndarray[S2, np.dtype[Scalar]],
    pattern: Pattern,
    /,
) -> _NumpyResult[einsum_shape(Pattern, tuple[S1, S2])]: ...
@overload
def einsum[
    S1: IntTuple,
    S2: IntTuple,
    S3: IntTuple,
    Scalar: np.number | np.bool_,
    Pattern: Flag[str],
](
    tensor1: np.ndarray[S1, np.dtype[Scalar]],
    tensor2: np.ndarray[S2, np.dtype[Scalar]],
    tensor3: np.ndarray[S3, np.dtype[Scalar]],
    pattern: Pattern,
    /,
) -> _NumpyResult[einsum_shape(Pattern, tuple[S1, S2, S3])]: ...
@overload
def einsum[
    S1: IntTuple,
    S2: IntTuple,
    S3: IntTuple,
    S4: IntTuple,
    Scalar: np.number | np.bool_,
    Pattern: Flag[str],
](
    tensor1: np.ndarray[S1, np.dtype[Scalar]],
    tensor2: np.ndarray[S2, np.dtype[Scalar]],
    tensor3: np.ndarray[S3, np.dtype[Scalar]],
    tensor4: np.ndarray[S4, np.dtype[Scalar]],
    pattern: Pattern,
    /,
) -> _NumpyResult[einsum_shape(Pattern, tuple[S1, S2, S3, S4])]: ...
@overload
def einsum(*tensors_and_pattern: Any) -> Any: ...
