# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

from types import MappingProxyType
from typing import Any, assert_type, override, TYPE_CHECKING

import torch
import torch.nn as nn
from shape_extensions import assert_shape, Int, IntTuple, IntVar

if TYPE_CHECKING:
    from torch import Tensor


class LinearLayer[N: IntVar, M: IntVar](nn.Module):
    def __init__(self, n: Int[N], m: Int[M]) -> None:
        super().__init__()
        self.linear = nn.Linear(n, m)

    def forward[B: IntVar](self, x: Tensor[[B, N]]) -> Tensor[[B, M]]:
        return self.linear(x)

    @override
    def extra_repr(self) -> str:
        return f"in={self.linear.weight.shape[1]}, out={self.linear.weight.shape[0]}"


class BadExtraRepr(nn.Module):
    @override
    def extra_repr(self) -> int:  # E: inconsistent manner
        return 0


def test_extra_repr_override() -> None:
    layer = LinearLayer(6, 9)
    assert_type(layer.extra_repr(), str)
    assert layer.extra_repr() == "in=6, out=9"
    assert_shape(layer(torch.randn((2, 6))).shape, (2, 9))


def test_module_load_state_dict_mapping() -> None:
    model = torch.nn.Linear(3, 2)
    result = model.load_state_dict(MappingProxyType(model.state_dict()))
    assert_shape(model.weight.shape, (2, 3))
    assert result.missing_keys == result.unexpected_keys == []

    class ExtraState(torch.nn.Module):
        def __init__(self) -> None:
            super().__init__()
            self.extra: Any = None
            self.register_buffer("payload", torch.zeros(2))

        def get_extra_state(self) -> Any:
            return self.extra

        def set_extra_state(self, state: Any) -> None:
            self.extra = state

    extra = ExtraState()
    state = MappingProxyType({"payload": torch.ones(2), "_extra_state": "restored"})
    extra.load_state_dict(state)
    assert_shape(extra.get_buffer("payload").shape, IntTuple, runtime=(2,))
    assert extra.extra == "restored"
    assert torch.equal(extra.get_buffer("payload"), torch.ones(2))


def test_load_state_dict_pre_hook_arguments_and_ignored_returns() -> None:
    model = nn.Linear(3, 2)
    calls: list[str] = []

    def without_module(
        state_dict: dict[str, Any],
        prefix: str,
        local_metadata: dict[str, Any],
        strict: bool,
        missing_keys: list[str],
        unexpected_keys: list[str],
        error_msgs: list[str],
    ) -> str:
        assert prefix == ""
        assert "weight" in state_dict
        assert "version" in local_metadata
        assert strict
        assert missing_keys == unexpected_keys == error_msgs == []
        calls.append("without_module")
        return "ignored"

    def with_module(
        module: nn.Module, state_dict: dict[str, Any], prefix: str, *args: Any
    ) -> str:
        assert module is model
        assert prefix == ""
        assert "weight" in state_dict
        assert len(args) == 5
        calls.append("with_module")
        return "ignored"

    default_handle = model._register_load_state_dict_pre_hook(without_module)
    false_handle = model._register_load_state_dict_pre_hook(
        without_module, with_module=False
    )
    true_handle = model._register_load_state_dict_pre_hook(
        with_module, with_module=True
    )
    result = model.load_state_dict(model.state_dict())
    assert_shape(model.weight.shape, (2, 3))
    assert result.missing_keys == result.unexpected_keys == []
    assert calls == ["without_module", "without_module", "with_module"]
    default_handle.remove()
    false_handle.remove()
    true_handle.remove()
    model.load_state_dict(model.state_dict())
    assert calls == ["without_module", "without_module", "with_module"]


if TYPE_CHECKING:

    def check_load_state_dict_pre_hook_arity(model: nn.Module) -> None:
        def two_arguments(state_dict: dict[str, Tensor], prefix: str) -> None:
            pass

        def module_hook(
            module: nn.Module, state_dict: dict[str, Any], prefix: str, *args: Any
        ) -> object:
            return object()

        # E: not assignable
        model._register_load_state_dict_pre_hook(two_arguments)
        # E: No matching overload
        model._register_load_state_dict_pre_hook(two_arguments, with_module=True)
        # E: No matching overload
        model._register_load_state_dict_pre_hook(module_hook, with_module=False)
