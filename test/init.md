# Tests for `pyrefly init --non-interactive`

## Non-interactive mode creates config in empty directory

```scrut {output_stream: stderr}
$ mkdir $TMPDIR/init_test && \
> $PYREFLY init --non-interactive $TMPDIR/init_test
 INFO New config written to `*/pyrefly.toml` (glob)
* (glob*)
[0]
```

## Non-interactive mode migrates mypy config

```scrut {output_stream: stderr}
$ mkdir $TMPDIR/init_mypy && \
> echo -e "[mypy]\nignore_missing_imports = True" > $TMPDIR/init_mypy/mypy.ini && \
> $PYREFLY init --non-interactive $TMPDIR/init_mypy
 INFO Found an existing type checking configuration - setting up pyrefly ...
* (glob*)
[0]
```

## Non-interactive mode leaves the config unchanged when a basedpyright baseline exists

```scrut {output_stream: stderr}
$ mkdir -p $TMPDIR/init_baseline/.basedpyright && \
> echo '[tool.basedpyright]' > $TMPDIR/init_baseline/pyproject.toml && \
> echo '{}' > $TMPDIR/init_baseline/.basedpyright/baseline.json && \
> $PYREFLY init --non-interactive $TMPDIR/init_baseline && \
> test ! -e $TMPDIR/init_baseline/pyrefly_baseline.json && \
> ! grep -q 'baseline = "pyrefly_baseline.json"' $TMPDIR/init_baseline/pyproject.toml
 INFO Found an existing type checking configuration - setting up pyrefly ...
* (glob*)
 INFO Running pyrefly check...
* (glob*)
[0]
```

## Answering yes records the baseline and succeeds when the project has errors

```scrut {output_stream: stderr}
$ mkdir -p $TMPDIR/init_baseline_yes/.basedpyright && \
> printf '%s\n' '[tool.basedpyright]' 'include = ["a.py"]' > $TMPDIR/init_baseline_yes/pyproject.toml && \
> echo '{}' > $TMPDIR/init_baseline_yes/.basedpyright/baseline.json && \
> echo 'x: int = "no"' > $TMPDIR/init_baseline_yes/a.py && \
> printf 'y\n' | $PYREFLY init $TMPDIR/init_baseline_yes && \
> test -s $TMPDIR/init_baseline_yes/pyrefly_baseline.json && \
> grep -q 'baseline = "pyrefly_baseline.json"' $TMPDIR/init_baseline_yes/pyproject.toml
 INFO Found an existing type checking configuration - setting up pyrefly ...
* (glob*)
 INFO Running pyrefly check with --update-baseline...
* (glob*)
[0]
```

## Non-interactive mode won't overwrite existing pyrefly config

```scrut {output_stream: stderr}
$ mkdir $TMPDIR/init_existing && \
> echo "" > $TMPDIR/init_existing/pyrefly.toml && \
> $PYREFLY init --non-interactive $TMPDIR/init_existing
[1]
```
