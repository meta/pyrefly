# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from typing import assert_type

import numpy as np
from shape_extensions import assert_shape


def test_pairwise_distance_comparison() -> None:
    """Regression for https://github.com/facebook/pyrefly/issues/4917."""
    rng = np.random.default_rng(7)
    count = 300
    size = 20.0
    radius = 1.5
    positions = rng.uniform(0, size, (count, 2))

    deltas = positions[:, None, :] - positions[None, :, :]
    deltas -= size * np.round(deltas / size)
    squared_distances = (deltas**2).sum(-1)
    assert_type(squared_distances, np.ndarray[[300, 300], np.dtype[np.float64]])
    assert isinstance(squared_distances, np.ndarray)
    assert_shape(squared_distances.shape, (300, 300))
    assert squared_distances.dtype == np.dtype(np.float64)
    nearby = squared_distances < radius * radius
    assert_type(nearby, np.ndarray[[300, 300], np.dtype[np.bool_]])
    assert_shape(nearby.shape, (300, 300))
    assert nearby.dtype == np.bool_
