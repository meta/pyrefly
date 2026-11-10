# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import logging
import re


def make_logger() -> logging.Logger:
    return logging.getLogger(__name__)


class Lookalike:
    def getLogger(self) -> int:
        return 0


logger = logging.getLogger(__name__)
wrapped_logger = make_logger()
PATTERN = re.compile("a+")

lookalike = Lookalike().getLogger()
