# Pyrefly einops shape stubs

This package is a PEP 561 stub-only distribution for `einops`. It provides
shape-aware annotations for `rearrange`, `reduce`, `repeat`, and `einsum` using
Pyrefly's type-level shape DSL. The `einops.layers.torch` `Rearrange` and `Reduce`
modules preserve the constructor pattern and axis lengths across calls.

The precise annotations target PyTorch, NumPy, and JAX. Shape computations are
shared; backend overloads describe their different result types. NumPy `rearrange`
and `repeat` preserve `ndarray` and its dtype. NumPy reductions and `einsum` may
return array scalars, so their result protocol preserves shape and array conversion
without promising `ndarray` or a dtype that reductions may promote. The precise
NumPy reduction and `einsum` overloads cover numeric and boolean dtypes; object-dtype
operations can return arbitrary Python objects and use the gradual fallback.

Backend shape stubs are optional dependencies: install the `torch`, `numpy`, or
`jax` extra, or combine them as `[torch,numpy,jax]`. The base package does not
install an array library. Install matching shape stubs for each of these backends
present in your environment. Existing Torch users upgrading from the base package
must add `[torch]` to retain Torch shape stubs.

This expansion requires a Pyrefly build with `unresolved-import-signature` support;
older checkers can silently select an unavailable backend and lose shape precision.
A numeric checker version floor can be set once that feature has a released version.
Do not configure `replace-imports-with-any` or `ignore-missing-imports` for absent
backends: those settings erase the provenance needed to reject their overloads.
If warnings are unwanted, set `unresolved-import-signature` severity to `ignore`
instead; overload selection still rejects unavailable signatures.

Literal `axes_lengths` keyword arguments are carried into the shape rules, so named
input splits and repeated output axes remain tracked. Dynamically unpacked mappings
preserve the known rank but use gradual dimensions for unknown axis lengths.

Run the static tests with:

```bash
python3 tensor-shapes/pyrefly-einops-stubs/run_pyrefly.py --buck
```
