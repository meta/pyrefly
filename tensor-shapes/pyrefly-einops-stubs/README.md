# Pyrefly einops shape stubs

This package is a PEP 561 stub-only distribution for `einops`. It provides
shape-aware annotations for `rearrange`, `reduce`, `repeat`, and `einsum` using
Pyrefly's type-level shape DSL. The `einops.layers.torch` `Rearrange` and `Reduce`
modules preserve the constructor pattern and axis lengths across calls.

The precise annotations target PyTorch and JAX. The runtime corpus also exercises
NumPy to validate that the shared einops pattern semantics agree across backends.

Backend shape stubs are optional dependencies: install the `torch` or `jax` extra
for the libraries your project uses, or combine them as `[torch,jax]`. The base
package does not install an array library. Install matching shape stubs for every
installed backend used in these annotations.

Optional-backend overload selection requires Pyrefly's `unresolved-import-signature`
support. Hide that diagnostic if desired rather than replacing missing imports
with `Any`, which would make an unavailable backend a broad overload again.

Literal `axes_lengths` keyword arguments are carried into the shape rules, so named
input splits and repeated output axes remain tracked. Dynamically unpacked mappings
preserve the known rank but use gradual dimensions for unknown axis lengths.

Run the static tests with:

```bash
python3 tensor-shapes/pyrefly-einops-stubs/run_pyrefly.py --buck
```
