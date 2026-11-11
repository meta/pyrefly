# Tests for checking the projects of a workspace

## Set up a workspace

The root config excludes every `vendor` directory. `packages/a` has no Pyrefly
config, so the root config governs it. `packages/b` has its own config, which
does not exclude `vendor` and makes `bad-assignment` a warning. Each file has
one `bad-assignment`, so the output shows which files are checked, and `WARN`
shows that a file is checked with the config of `packages/b`.

```scrut
$ mkdir -p $TMPDIR/workspace/packages/a/vendor $TMPDIR/workspace/packages/b/vendor && \
> cd $TMPDIR/workspace && \
> printf '[tool.pyrefly]\nproject-excludes = ["**/vendor"]\n' > pyproject.toml && \
> printf '[project]\nname = "a"\n' > packages/a/pyproject.toml && \
> printf '[project]\nname = "b"\n\n[tool.pyrefly.errors]\nbad-assignment = "warn"\n' > packages/b/pyproject.toml && \
> for d in . packages/a packages/a/vendor packages/b packages/b/vendor; do echo "x: int = ''" > $d/m.py; done
[0]
```

## `--project-excludes` replaces the excludes of the config

The root config excludes `vendor`, but `packages/a/vendor/m.py` is checked.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check --project-excludes=packages/b --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
*packages/a/m.py:1:* (glob)
*packages/a/vendor/m.py:1:* (glob)
[1]
```

## `--extend-project-excludes` adds to the excludes of the config

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check --extend-project-excludes=packages/b --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
*packages/a/m.py:1:* (glob)
[1]
```

## A directory runs project mode for the files under it

The root config governs `packages/a`, so its `vendor` directory is excluded.

Bug: a directory runs single-file mode, so `packages/a/vendor` is checked.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

## A directory uses its own config if it has one

The config of `packages/b` does not exclude `vendor`.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/b --python-version 3.13.0 --output-format=min-text
 WARN packages/b/m.py:1:* (glob)
 WARN packages/b/vendor/m.py:1:* (glob)
[0]
```

A directory under `packages/b` uses the config of `packages/b`.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/b/vendor --python-version 3.13.0 --output-format=min-text
 WARN packages/b/vendor/m.py:1:* (glob)
[0]
```

## `pyrefly check .` checks the same files as `pyrefly check`

`packages/b` is below `.`, so the root config governs it, the same as in
`pyrefly check`: the config of `packages/b` is not used.

Bug: `.` runs single-file mode, so `vendor` is checked, and `packages/b` is
checked with its own config.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check . --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
 WARN packages/b/m.py:1:* (glob)
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
 WARN packages/b/vendor/m.py:1:* (glob)
[1]
```

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
ERROR packages/a/m.py:1:* (glob)
ERROR packages/b/m.py:1:* (glob)
[1]
```

## `--config` gives every directory the same config

Bug: the directory runs single-file mode, so `packages/b/vendor` is checked.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check --config pyproject.toml packages/b --python-version 3.13.0 --output-format=min-text
ERROR packages/b/m.py:1:* (glob)
ERROR packages/b/vendor/m.py:1:* (glob)
[1]
```

## Directories with different configs are checked in separate runs

`packages/b` has its own config, so it is a different project from `.`. The
error lists every directory with its config.

Bug: the directories run single-file mode, so there is no error.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check . packages/b --python-version 3.13.0 --output-format=min-text 2>&1 | grep -v INFO
ERROR m.py:1:* (glob)
 WARN packages/b/m.py:1:* (glob)
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
 WARN packages/b/vendor/m.py:1:* (glob)
[0]
```

## A file runs single-file mode

A file is checked even if the config excludes it.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a/vendor/m.py --python-version 3.13.0 --output-format=min-text
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

A file uses the config that an upward search finds from it.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/b/vendor/m.py --python-version 3.13.0 --output-format=min-text
 WARN packages/b/vendor/m.py:1:* (glob)
[0]
```

## A file given with a directory runs single-file mode

The directory still runs project mode, and the file is checked even if the
config excludes it.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a packages/a/vendor/m.py --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

The file keeps the config that an upward search finds from it.

Bug: the directory runs single-file mode, so `packages/a/vendor` is checked.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a packages/b/m.py --python-version 3.13.0 --output-format=min-text
 WARN packages/b/m.py:1:* (glob)
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

## A glob is checked the same as the paths that it matches

Pyrefly expands a glob as a shell does, so a quoted glob and an unquoted glob
give the same result. A glob that matches a directory runs project mode for it:

Bug: a glob runs single-file mode, so `packages/a/vendor` is checked.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check 'packages/a*' --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a* --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

A glob that matches a file runs single-file mode for it, so the file is checked
even if the config excludes it:

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check 'packages/a/vendor/*.py' --python-version 3.13.0 --output-format=min-text
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a/vendor/*.py --python-version 3.13.0 --output-format=min-text
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

A glob that matches directories with different configs is an error:

Bug: a glob runs single-file mode, so there is no error.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check 'packages/*' --python-version 3.13.0 --output-format=min-text 2>&1 | grep -v INFO
 WARN packages/b/m.py:1:* (glob)
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
 WARN packages/b/vendor/m.py:1:* (glob)
[0]
```

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/* --python-version 3.13.0 --output-format=min-text 2>&1 | grep -v INFO
 WARN packages/b/m.py:1:* (glob)
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
 WARN packages/b/vendor/m.py:1:* (glob)
[0]
```

A recursive glob matches every file under a directory, so each file is checked
even if the config excludes it:

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check 'packages/a/**' --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

A glob matches no files that are not Python files, but a shell passes them:

Bug: `packages/a/vendor` runs single-file mode, so `packages/a/vendor/m.py` is
checked.

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check 'packages/a/*' --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

```scrut
$ cd $TMPDIR/workspace && $PYREFLY check packages/a/* --python-version 3.13.0 --output-format=min-text
ERROR packages/a/m.py:1:* (glob)
ERROR packages/a/pyproject.toml:1:* (glob)
ERROR packages/a/vendor/m.py:1:* (glob)
[1]
```

A glob that matches nothing is an error:

```scrut {output_stream: stderr}
$ cd $TMPDIR/workspace && $PYREFLY check 'missing/*.py' --python-version 3.13.0 --output-format=min-text
No Python files matched pattern `*/workspace/missing/*.py` (glob)
[1]
```

## A directory with no config runs single-file mode

```scrut
$ mkdir -p $TMPDIR/loose/sub && echo 'x =' > $TMPDIR/loose/sub/m.py && \
> cd $TMPDIR/loose && $PYREFLY check sub --python-version 3.13.0 --output-format=min-text
ERROR sub/m.py:1:* (glob)
[1]
```

## A glob that matches a PEP 723 script checks it with `--exclude-scripts`

`--exclude-scripts` skips the scripts in a directory, but checks a file that is
given explicitly. A glob gives the files that it matches, as a shell does, so
it checks a script that it matches.

Bug: a quoted glob skips the script, but an unquoted glob checks it.

```scrut
$ mkdir -p $TMPDIR/scripts && cd $TMPDIR/scripts && touch pyrefly.toml && \
> printf '# /// script\n# dependencies = []\n# ///\nx: int = ""\n' > tool.py && \
> echo "x: int = ''" > m.py && \
> $PYREFLY check --exclude-scripts '*.py' --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
[1]
```

```scrut
$ cd $TMPDIR/scripts && $PYREFLY check --exclude-scripts *.py --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
ERROR tool.py:4:* (glob)
[1]
```

```scrut
$ cd $TMPDIR/scripts && $PYREFLY check --exclude-scripts . --python-version 3.13.0 --output-format=min-text
ERROR m.py:1:* (glob)
[1]
```
