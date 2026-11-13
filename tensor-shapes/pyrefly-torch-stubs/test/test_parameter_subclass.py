# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from typing import assert_type

import torch
import torch.nn as nn
from shape_extensions import assert_raises, assert_shape, IntTuple


class TaggedParameter(nn.Parameter):
    tag: str


class ShapedTaggedParameter[Shape: IntTuple](nn.Parameter[Shape]):
    tag: str


def test_parameter_keeps_exact_shapes() -> None:
    parameter = nn.Parameter(torch.ones((2, 3)), requires_grad=False)
    assert_type(parameter, nn.Parameter[[2, 3]])
    assert type(parameter) is nn.Parameter
    assert not parameter.requires_grad
    assert_shape(parameter.shape, (2, 3))


def test_parameter_subclass_keeps_custom_attributes() -> None:
    parameter = TaggedParameter(torch.ones((2, 3)))
    assert_type(parameter, TaggedParameter)
    parameter.tag = "weight"
    assert type(parameter) is TaggedParameter
    assert parameter.tag == "weight"
    assert tuple(parameter.shape) == (2, 3)
    empty = TaggedParameter()
    assert_type(empty, TaggedParameter)
    assert type(empty) is TaggedParameter
    assert tuple(empty.shape) == (0,)
    with assert_raises(AttributeError):
        TaggedParameter([1.0])  # E: No matching overload


def test_generic_parameter_subclass_keeps_shape() -> None:
    parameter = ShapedTaggedParameter(torch.ones((2, 3)), requires_grad=False)
    assert_type(parameter, ShapedTaggedParameter[[2, 3]])
    parameter.tag = "bias"
    assert type(parameter) is ShapedTaggedParameter
    assert parameter.tag == "bias"
    assert not parameter.requires_grad
    assert_shape(parameter.shape, (2, 3))
