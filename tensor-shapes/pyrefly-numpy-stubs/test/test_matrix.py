# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""The legacy `matrix` subclass.

The overlay re-exports `matrix` from `numpy.matrixlib`. NumPy 1.24 declares the
class in `numpy/__init__.pyi`, which this overlay replaces, and its `matrixlib`
re-exports the name back from `numpy`, so under that version the name resolves
in a cycle with no declaration left.
"""

import numpy as np
from shape_extensions import assert_raises


def check_matrix_members(intrinsics: np.matrix) -> int:
    return intrinsics.getI()  # E: not assignable


def test_matrix_runtime_members() -> None:
    identity = np.matrix(np.identity(3))

    with assert_raises(ValueError):
        np.asarray(identity).reshape(4, 4)
    assert identity.shape == (3, 3)
    assert identity.getI().shape == (3, 3)


def test_matrix_factories_agree_with_the_public_class() -> None:
    # `asmatrix` and `bmat` are re-exported from `numpy.matrixlib`, which
    # defines its own `matrix` on newer NumPy. Their results must stay
    # assignable to the class the overlay exposes.
    from_asmatrix: np.matrix = np.asmatrix(np.identity(3))
    from_bmat: np.matrix = np.bmat([[np.identity(2)]])

    with assert_raises(ValueError):
        np.asarray(from_asmatrix).reshape(5, 5)
    assert from_asmatrix.shape == (3, 3)
    assert from_bmat.shape == (2, 2)
