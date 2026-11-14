# Semantic query CLI tests

## Queries select the target project, not the current directory

The project contains a typing error and an unrelated symbol with the same name.
Canonical paths also account for temporary directories under `/private/var` on macOS.

```scrut
$ mkdir -p "$TMPDIR/query project/project" && \
> cd "$TMPDIR/query project" && root=$(pwd -P) && \
> printf 'project-includes = []\n' > pyrefly.toml && \
> printf 'project-includes = ["**/*.py"]\nproject-excludes = []\nskip-interpreter-query = true\n' > project/pyrefly.toml && \
> printf 'def leaf(x: int) -> int:\n    return x\n\ndef target(x: int) -> int:\n    return leaf(x)\n' > project/core.py && \
> printf 'from core import target\n\ndef caller() -> int:\n    return target(1)\n\nbad: str = 1\n' > project/caller.py && \
> printf 'def target() -> str:\n    return "unrelated"\n\nresult = target()\n' > project/unrelated.py && \
> "$PYREFLY" --threads 1 query find-references "$root/project/core.py" 4 5 --include-declaration > references.json && \
> (cd project && "$PYREFLY" --threads 1 query find-references "$root/project/core.py" 4 5 --include-declaration) > local.json && \
> cmp references.json local.json && \
> "$JQ" -c --arg root "$root" 'map(.uri |= ltrimstr("file://" + ($root | @uri | gsub("%2F"; "/")) + "/")) | map([.uri, .range.start.line, .range.start.character, .range.end.line, .range.end.character])' references.json
[["project/caller.py",0,17,0,23],["project/caller.py",3,11,3,17],["project/core.py",3,4,3,10]]
[0]
```

## References omit the declaration unless requested

```scrut
$ cd "$TMPDIR/query project" && \
> "$PYREFLY" --threads 1 query find-references project/core.py 4 5 > uses.json && \
> "$JQ" -e --slurpfile all references.json '. == $all[0][0:2]' uses.json
true
[0]
```

## Nested and explicit configurations select the candidate files

```scrut
$ cd "$TMPDIR/query project" && root=$(pwd -P) && \
> mkdir project/nested && \
> printf 'project-includes = ["*.py"]\nskip-interpreter-query = true\n' > project/nested/pyrefly.toml && \
> printf 'value: int = 1\nprint(value)\n' > project/nested/inner.py && \
> "$PYREFLY" --threads 1 query find-references project/nested/inner.py 1 1 --include-declaration > nested.json && \
> "$JQ" -c --arg root "$root" 'map(.uri |= ltrimstr("file://" + ($root | @uri | gsub("%2F"; "/")) + "/")) | map([.uri, .range.start.line, .range.start.character, .range.end.line, .range.end.character])' nested.json && \
> printf 'project-includes = ["core.py"]\nskip-interpreter-query = true\n' > project/explicit.toml && \
> "$PYREFLY" --threads 1 query find-references project/core.py 4 5 --include-declaration --config project/explicit.toml > explicit.json && \
> "$PYREFLY" --threads 1 query --config project/explicit.toml find-references project/core.py 4 5 --include-declaration > explicit-before.json && \
> cmp explicit.json explicit-before.json && \
> "$JQ" -e --slurpfile all references.json '. == $all[0][2:3]' explicit.json
[["project/nested/inner.py",0,0,0,5],["project/nested/inner.py",1,6,1,11]]
true
[0]
```

## Candidate files keep their own configurations

```scrut
$ cd "$TMPDIR/query project" && \
> printf 'replace-imports-with-any = ["core"]\nskip-interpreter-query = true\n' > project/nested/pyrefly.toml && \
> printf 'from core import target\n\ntarget(2)\n' > project/nested/other.py && \
> "$PYREFLY" --threads 1 query find-references project/core.py 4 5 --include-declaration > candidates.json && \
> cmp references.json candidates.json
[0]
```

## Hover returns the signature as clean JSON

```scrut
$ cd "$TMPDIR/query project" && \
> "$PYREFLY" --threads 1 query hover project/core.py 4 5 > hover.json && \
> "$JQ" -c . hover.json
{"contents":{"kind":"markdown","value":"```python\n(function) target: def target(x: int) -> int: ...\n```"}}
[0]
```

## Call hierarchy resolves an imported definition and exact source ranges

```scrut
$ cd "$TMPDIR/query project" && root=$(pwd -P) && \
> "$PYREFLY" --threads 1 query call-hierarchy project/caller.py 4 12 > calls.json && \
> "$JQ" -c --arg root "$root" 'walk(if type == "object" and has("uri") then .uri |= ltrimstr("file://" + ($root | @uri | gsub("%2F"; "/")) + "/") else . end)' calls.json
{"incoming":[{"from":{"detail":"caller.caller","kind":12,"name":"caller","range":{"end":{"character":20,"line":3},"start":{"character":0,"line":2}},"selectionRange":{"end":{"character":10,"line":2},"start":{"character":4,"line":2}},"uri":"project/caller.py"},"fromRanges":[{"end":{"character":20,"line":3},"start":{"character":11,"line":3}}]}],"item":{"detail":"core.target","kind":12,"name":"target","range":{"end":{"character":18,"line":4},"start":{"character":0,"line":3}},"selectionRange":{"end":{"character":10,"line":3},"start":{"character":4,"line":3}},"uri":"project/core.py"},"outgoing":[{"fromRanges":[{"end":{"character":18,"line":4},"start":{"character":11,"line":4}}],"to":{"detail":"core.leaf","kind":12,"name":"leaf","range":{"end":{"character":8,"line":0},"start":{"character":4,"line":0}},"selectionRange":{"end":{"character":8,"line":0},"start":{"character":4,"line":0}},"uri":"project/core.py"}}]}
[0]
```

## Configuration overrides work before and after the subcommand

```scrut
$ cd "$TMPDIR/query project" && \
> "$PYREFLY" --threads 1 query hover project/core.py 4 5 --python-version 3.12 --skip-interpreter-query --check-unannotated-defs=false > after.json && \
> "$PYREFLY" --threads 1 query --python-version 3.12 --skip-interpreter-query --check-unannotated-defs=false hover project/core.py 4 5 > before.json && \
> cmp before.json after.json && cmp hover.json after.json
[0]
```

## Builtin and standard-library definitions use materialized stub files

```scrut
$ cd "$TMPDIR/query project" && \
> printf 'import json\n\ndef encode() -> str:\n    return json.dumps(len([]))\n' > project/stdlib.py && \
> for column in 17 23; do
>   "$PYREFLY" --threads 1 query find-references project/stdlib.py 4 "$column" --include-declaration > stubs.json || exit 1
>   "$JQ" -e 'any(.[]; .uri | endswith(".pyi")) and all(.[]; .uri | startswith("file:///"))' stubs.json > /dev/null || exit 1
>   "$JQ" -r '.[].uri | ltrimstr("file://") | gsub("%20"; " ")' stubs.json > paths.txt || exit 1
>   while IFS= read -r path; do test -f "$path" || exit 1; done < paths.txt || exit 1
>   "$PYREFLY" --threads 1 query call-hierarchy project/stdlib.py 4 "$column" > stub-calls.json || exit 1
>   path=$("$JQ" -er '.item.uri | select(startswith("file:///")) | ltrimstr("file://") | gsub("%20"; " ")' stub-calls.json) || exit 1
>   test -f "$path" || exit 1
> done && \
> "$PYREFLY" --threads 1 query call-hierarchy project/stdlib.py 3 5 > encode.json && \
> "$JQ" -e 'any(.outgoing[]; .to.name == "dumps") and all(.outgoing[]; .to.uri | endswith(".pyi"))' encode.json
true
[0]
```

## A source database supplies project imports without a Python executable

```scrut
$ cd "$TMPDIR/query project" && root=$(pwd -P) && \
> cp project/pyrefly.toml saved.toml && \
> platform=$(uname -s) && case "$platform" in Darwin) platform=darwin;; *) platform=linux;; esac && \
> "$JQ" -n --arg root "$root/project" --arg platform "$platform" '{root: $root, db: {"//:library": {srcs: {core: ["core.py"], caller: ["caller.py"]}, python_version: "3.13", python_platform: $platform}}}' > project/manifest.json && \
> printf 'project-includes = ["core.py", "caller.py"]\nskip-interpreter-query = true\n[build-system]\ntype = "custom"\ncommand = ["/bin/sh", "-c", "cat manifest.json"]\n' > project/pyrefly.toml && \
> "$PYREFLY" --threads 1 query find-references project/core.py 4 5 --include-declaration > sourcedb.json && \
> cmp references.json sourcedb.json
[0]
```

## Source database failures produce an error and no JSON

```scrut
$ cd "$TMPDIR/query project" && \
> printf 'invalid JSON\n' > project/manifest.json && \
> if "$PYREFLY" --threads 1 query find-references project/core.py 4 5 > failed.json 2> error.txt; then exit 1; fi && \
> test ! -s failed.json && grep -q 'TargetManifestDatabase' error.txt && \
> grep -q 'Fatal configuration error' error.txt && \
> cp saved.toml project/pyrefly.toml
[0]
```

## Invalid targets and positions produce an error and no JSON

```scrut
$ cd "$TMPDIR/query project" && \
> printf "text = '🐍'; value = 1\r\n    \r\n" > project/unicode.py && \
> printf '\377' > project/broken.py && \
> while IFS='|' read -r file line column message; do
>   if "$PYREFLY" --threads 1 query hover "project/$file" "$line" "$column" > failed.json 2> error.txt; then exit 1; fi
>   test ! -s failed.json && grep -q "$message" error.txt || exit 1
>   printf '%s:%s:%s: %s\n' "$file" "$line" "$column" "$message"
> done <<'CASES'
> missing.py|1|1|cannot access target
> .|1|1|is not a file
> broken.py|1|1|cannot load target
> core.py|0|1|line must be greater than zero
> core.py|1|0|column must be greater than zero
> core.py|99|1|invalid source position
> core.py|1|99|invalid source position
> unicode.py|1|10|invalid source position
> CASES
missing.py:1:1: cannot access target
.:1:1: is not a file
broken.py:1:1: cannot load target
core.py:0:1: line must be greater than zero
core.py:1:0: column must be greater than zero
core.py:99:1: invalid source position
core.py:1:99: invalid source position
unicode.py:1:10: invalid source position
[0]
```

## Whitespace hover is null and UTF-16 positions after a surrogate pair are valid

```scrut
$ cd "$TMPDIR/query project" && \
> "$PYREFLY" --threads 1 query hover project/unicode.py 2 1 > whitespace.json && \
> "$JQ" -c . whitespace.json && \
> "$PYREFLY" --threads 1 query hover project/unicode.py 1 14 > unicode.json && \
> "$JQ" -e '. != null' unicode.json
null
true
[0]
```

## Invalid configuration produces an error and no JSON

```scrut
$ cd "$TMPDIR/query project" && \
> printf 'invalid config\n' > project/pyrefly.toml && \
> if "$PYREFLY" --threads 1 query hover project/core.py 4 5 > failed.json 2> error.txt; then exit 1; fi && \
> test ! -s failed.json && grep -q 'Fatal configuration error' error.txt && \
> cp saved.toml project/pyrefly.toml
[0]
```

## Call hierarchy uses the definition's Python version

```scrut
$ cd "$TMPDIR/query project" && \
> printf 'project-includes = ["**/*.py"]\npython-version = "3.13"\nskip-interpreter-query = true\n' > project/pyrefly.toml && \
> mkdir project/library && touch project/library/__init__.py && \
> printf 'python-version = "3.10"\nskip-interpreter-query = true\n' > project/library/pyrefly.toml && \
> printf 'import sys\n\ndef old() -> int:\n    return 0\n\ndef new() -> int:\n    return 1\n\ndef target() -> int:\n    if sys.version_info >= (3, 12):\n        return new()\n    else:\n        return old()\n' > project/library/callee.py && \
> printf 'from library.callee import target\n\ntarget()\n' > project/caller.py && \
> "$PYREFLY" --threads 1 query call-hierarchy project/caller.py 3 1 > version.json && \
> "$JQ" -c '[.outgoing[].to.name]' version.json
["old"]
[0]
```

## Calls are sorted by URI and range, not by name

```scrut
$ cd "$TMPDIR/query project" && \
> printf 'def zebra() -> int:\n    return 1\n\ndef alpha() -> int:\n    return 2\n\ndef target() -> int:\n    first = alpha()\n    second = zebra()\n    return first + second\n' > project/core.py && \
> printf 'from core import target\n\ndef zebra() -> int:\n    return target()\n\ndef alpha() -> int:\n    return target()\n' > project/caller.py && \
> "$PYREFLY" --threads 1 query call-hierarchy project/core.py 7 5 > sorted.json && \
> "$JQ" -c '[.incoming[].from.name], [.outgoing[].to.name]' sorted.json
["zebra","alpha"]
["zebra","alpha"]
[0]
```

## Help describes the experimental coordinate contract

```scrut
$ "$PYREFLY" query --help > "$TMPDIR/query-help.txt" && \
> for text in Experimental one-based UTF-16 zero-based 'file URIs'; do
>   grep -q "$text" "$TMPDIR/query-help.txt" || exit 1
> done
[0]
```
