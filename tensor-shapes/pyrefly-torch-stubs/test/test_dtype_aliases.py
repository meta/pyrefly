# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
from shape_extensions import assert_shape


def test_float_dtype_alias() -> None:
    assert_type(torch.float, torch.dtype)
    assert_type(torch.pi, float)
    assert torch.float is torch.float32
    assert isinstance(torch.float, torch.dtype)
    assert_shape(torch.ones((2, 3), dtype=torch.float).shape, (2, 3))


def test_unsigned_dtype() -> None:
    dtype = torch.uint16
    tensor = torch.ones((2, 3)).to(dtype)
    assert tensor.dtype == dtype
    assert_shape(tensor.shape, (2, 3))


def test_half_dtype_alias() -> None:
    assert torch.half is torch.float16
    assert_shape(torch.ones((2, 3), dtype=torch.half).shape, (2, 3))


def test_low_precision_and_wide_unsigned_dtypes() -> None:
    for dtype, itemsize in (
        (torch.float8_e4m3fn, 1),
        (torch.float8_e5m2, 1),
        (torch.float8_e8m0fnu, 1),
        (torch.float4_e2m1fn_x2, 1),
        (torch.uint32, 4),
        (torch.uint64, 8),
    ):
        assert_type(dtype, torch.dtype)
        assert dtype.itemsize == itemsize
        tensor = torch.empty((2, 3), dtype=dtype)
        assert tensor.dtype == dtype
        assert_shape(tensor.shape, (2, 3))


def test_dtype_itemsize() -> None:
    tensor = torch.ones((2, 3), dtype=torch.float)
    itemsize = torch.float.itemsize
    assert_type(itemsize, int)
    assert itemsize == tensor.element_size() == 4
    assert_shape(tensor.shape, (2, 3))


def test_double_and_complex_dtype_aliases() -> None:
    double = torch.double
    cfloat = torch.cfloat
    cdouble = torch.cdouble
    assert_type(double, torch.dtype)
    assert_type(cfloat, torch.dtype)
    assert_type(cdouble, torch.dtype)
    assert double is torch.float64
    assert cfloat is torch.complex64
    assert cdouble is torch.complex128
    for dtype in (double, cfloat, cdouble):
        tensor = torch.zeros((2, 3), dtype=dtype)
        assert tensor.dtype == dtype
        assert_shape(tensor.shape, (2, 3))
    viewed = torch.ones((2, 4)).view(double)
    assert viewed.dtype == torch.float64
    assert tuple(viewed.shape) == (2, 2)
