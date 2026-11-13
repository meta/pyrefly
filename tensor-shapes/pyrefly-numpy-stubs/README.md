# pyrefly-numpy-stubs

NumPy type stubs with array shape information for Pyrefly.

This package installs `numpy-shape-stubs`, a Pyrefly-specific stub package for
runtime `numpy`. Pyrefly discovers it by default; set
`use-shape-comments-and-stubs = false` to opt out. Other type checkers ignore these
stubs and continue using NumPy's ordinary types.

The package is versioned in lockstep with Pyrefly and depends on the matching
`pyrefly-shape-extensions` package. Installing it also installs `numpy`, which
provides upstream modules used by these partial stubs. The `numpy` dependency is
unpinned, so projects can choose their own NumPy version.

Static checks use NumPy's installed stubs for APIs that this package re-exports:

```bash
python3 tensor-shapes/pyrefly-numpy-stubs/run_pyrefly.py
```

The runner uses `~/.tensor-shapes-venv` by default. Set
`$TENSOR_SHAPES_VENV` to select another virtualenv, or pass `--python` with the
path to a virtualenv interpreter that has NumPy installed.
