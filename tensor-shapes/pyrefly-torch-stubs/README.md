# pyrefly-torch-stubs

PyTorch type stubs with tensor shape information for Pyrefly.

This package installs `torch-shape-stubs`, a Pyrefly-specific stub package for
runtime `torch`. Pyrefly discovers it by default; set
`use-shape-comments-and-stubs = false` to opt out. Other type checkers ignore these
stubs and continue using PyTorch's ordinary types.

The package is versioned in lockstep with Pyrefly and depends on the matching
`pyrefly-shape-extensions` package. Installing it also installs `torch`, which
provides upstream modules used by these partial stubs. The `torch` dependency is
unpinned, so projects can choose their own PyTorch version.

Static checks use the installed Torch package for modules that this package
intentionally leaves unshaped:

```bash
python3 tensor-shapes/pyrefly-torch-stubs/run_pyrefly.py
```

The runner uses `~/.tensor-shapes-venv` by default. Set
`$TENSOR_SHAPES_VENV` to select another virtualenv, or pass `--python` with an
interpreter from a virtualenv that has Torch installed.
