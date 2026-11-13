# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import Any, assert_type

import numpy as np
import torch
from shape_extensions import assert_raises, assert_shape
from torch import Tensor

# `soundfile.read` returns `AudioData | AudioData_2d`, an array of unknown rank or a
# two-dimensional array, whichever layout it decoded.
_AudioDType = np.dtype[np.float32 | np.float64 | np.int32 | np.int16]
_Audio = np.ndarray[tuple[int, ...], _AudioDType]
_Audio2d = np.ndarray[tuple[int, int], _AudioDType]


def _decoded(two_d: bool) -> _Audio | _Audio2d:
    data = np.arange(6, dtype=np.float32)
    return data.reshape(3, 2) if two_d else data


def _first_channel(two_d: bool) -> _Audio | _Audio2d | np.ndarray[Any, _AudioDType]:
    data = _decoded(two_d)
    if data.ndim == 2:
        return data[:, 0]
    return data


def test_from_numpy_union_of_layouts() -> None:
    stereo = torch.from_numpy(_decoded(True))
    # Known inference limit: this union with an unknown-rank member is currently
    # inferred from its two-dimensional member.
    assert_type(stereo, Tensor[[int, int]])
    assert tuple(stereo.shape) == (3, 2)
    mono = torch.from_numpy(_decoded(False))
    assert tuple(mono.shape) == (6,)
    assert mono.tolist() == [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    assert_shape(torch.from_numpy(torch.zeros(6).numpy()).shape, (6,))


def test_from_numpy_union_with_any_shape_member() -> None:
    channel = torch.from_numpy(_first_channel(True))
    assert_type(channel, Tensor)
    assert channel.tolist() == [0.0, 2.0, 4.0]
    assert_shape(torch.from_numpy(torch.zeros(3).numpy()).shape, (3,))


def test_from_numpy_union_with_tensor_derived_array() -> None:
    def convert(synthetic: bool) -> Tensor:
        array = torch.zeros(4).numpy() if synthetic else _decoded(False)
        return torch.from_numpy(array)

    assert tuple(convert(True).shape) == (4,)
    assert tuple(convert(False).shape) == (6,)
    assert_shape(torch.from_numpy(torch.zeros((2, 3)).numpy()).shape, (2, 3))


def test_from_numpy_shares_memory_and_keeps_precise_shapes() -> None:
    array = torch.zeros((2, 3)).numpy()
    tensor = torch.from_numpy(array)
    assert_type(tensor, Tensor[[2, 3]])
    array[0, 1] = 5.0
    assert tensor[0, 1].item() == 5.0
    assert_shape(tensor.shape, (2, 3))
    annotated: np.ndarray[tuple[int, int], np.dtype[np.float32]] = np.zeros(
        (2, 3), dtype=np.float32
    )
    assert_type(torch.from_numpy(annotated), Tensor[[int, int]])


def test_from_numpy_rejects_non_arrays() -> None:
    assert_shape(torch.from_numpy(torch.zeros(2).numpy()).shape, (2,))
    with assert_raises(TypeError):
        torch.from_numpy("bad")  # E: No matching overload
    with assert_raises(TypeError):
        torch.from_numpy(3)  # E: No matching overload
