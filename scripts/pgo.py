#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Build Pyrefly with profile-guided optimization (PGO) and LLVM BOLT.

Both optimizations train on the same workload: `pyrefly check` on a corpus of
pinned open-source projects, plus a language-server session with incremental
edits. The release workflows call this script; it also runs locally:

    python3 scripts/pgo.py pgo                  # PGO-optimized binary
    python3 scripts/pgo.py pgo --train-only     # Only the merged profile
    python3 scripts/pgo.py check --binary <pgo binary>
    python3 scripts/pgo.py bolt --llvm-dir <dir> --binary <pgo binary>

`pgo --train-only` writes `pyrefly.profdata` and `pyrefly.profile-hot-count`
to the target directory. A release build consumes them through `RUSTFLAGS`:

    -Cprofile-use=<target-dir>/pyrefly.profdata
    -Cllvm-args=--profile-summary-hot-count=<contents of pyrefly.profile-hot-count>
"""

from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import io
import json
import os
import queue
import re
import shlex
import shutil
import subprocess
import sys
import threading
import time
import zipfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
MANIFEST = REPOSITORY_ROOT / "pyrefly" / "Cargo.toml"
DEFAULT_TARGET_DIR = REPOSITORY_ROOT / "target" / "pgo"
# Training dependencies are resolved as of this date, so the corpus is
# reproducible without lock files.
DEPENDENCY_EXCLUDE_NEWER = "2026-11-01T00:00:00Z"
# Settings from the caller's environment that would change what Pyrefly checks.
EXCLUDED_ENVIRONMENT_VARIABLES = ("CONDA_PREFIX", "PYTHONPATH", "VIRTUAL_ENV")
# How many times to try a command that downloads something.
NETWORK_ATTEMPTS = 3


@dataclass(frozen=True, slots=True)
class Project:
    """A pinned project that Pyrefly checks during training.

    Pyrefly runs from the checkout root, so it uses the project's own
    configuration (`pyrefly.toml`, `[tool.pyrefly]`, or an imported mypy or
    Pyright configuration). `check_paths` is only needed for a project that
    does not configure the files to check.
    """

    name: str
    repository: str
    revision: str
    python_version: str
    sparse_paths: tuple[str, ...]
    dependencies: tuple[str, ...] = ()
    check_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if re.fullmatch(r"[0-9a-f]{40}", self.revision) is None:
            raise ValueError(f"{self.name} must be pinned to a full commit SHA")


# Notable projects that check their code with Pyrefly, plus Home Assistant, a
# large asynchronous application that does not. Together they cover machine
# learning, scientific computing, type stubs, a Django web application, and an
# application whose settings Pyrefly imports from `mypy.ini`.
CORPUS = (
    Project(
        name="pytorch",
        repository="pytorch/pytorch",
        revision="2ee7a63cc8feb9171418ac602c0434dbb88348a1",
        python_version="3.12",
        sparse_paths=("torch", "caffe2", "tools", "test"),
        dependencies=(
            "filelock",
            "fsspec",
            "jinja2",
            "networkx",
            "numpy",
            "optree",
            "sympy",
            "typing-extensions",
        ),
    ),
    Project(
        name="jax",
        repository="jax-ml/jax",
        revision="c0a062274d6f1bc7ff1186b6f8759cc4a8c2a161",
        python_version="3.13",
        sparse_paths=("jax",),
        dependencies=("ml-dtypes", "numpy", "opt-einsum", "scipy"),
    ),
    Project(
        name="scipy",
        repository="scipy/scipy",
        revision="1eef77e24fb85176e4ecbd61e614f7d4f2dd212b",
        python_version="3.12",
        sparse_paths=("scipy",),
        dependencies=("numpy",),
    ),
    Project(
        name="scikit-learn",
        repository="scikit-learn/scikit-learn",
        revision="71659d6675c353a8de107a3166a8ea60ef223be1",
        python_version="3.12",
        sparse_paths=("sklearn", "build_tools", "maint_tools"),
        dependencies=("joblib", "numpy", "scipy", "threadpoolctl"),
    ),
    Project(
        name="numpy",
        repository="numpy/numpy",
        revision="9a65034ff8e464565b4ef9fa64c5c0663b4af1e2",
        python_version="3.12",
        sparse_paths=("numpy",),
    ),
    Project(
        name="courtlistener",
        repository="freelawproject/courtlistener",
        revision="08cdd741d769855e7561f8d9ffb1faeeeb47a1e3",
        python_version="3.13",
        sparse_paths=("cl",),
        dependencies=(
            "celery",
            "django",
            "django-filter",
            "django-stubs",
            "djangorestframework",
            "djangorestframework-stubs",
            "httpx",
            "pydantic",
            "requests",
        ),
    ),
    Project(
        name="home-assistant",
        repository="home-assistant/core",
        revision="cbf2c02c9bff331537c19bff69da8dc11c3fbfbf",
        python_version="3.14",
        sparse_paths=("homeassistant", "stubs"),
        dependencies=(
            "aiohttp",
            "astral",
            "attrs",
            "awesomeversion",
            "httpx",
            "jinja2",
            "orjson",
            "propcache",
            "pyjwt",
            "python-slugify",
            "pyyaml",
            "requests",
            "voluptuous",
            "voluptuous-serialize",
            "yarl",
        ),
        check_paths=("homeassistant",),
    ),
)


@dataclass(frozen=True, slots=True)
class LanguageServerSession:
    """An editing session: the first open file is edited with `error_edit`,
    a `(old, new)` replacement that introduces a type error."""

    open_files: tuple[str, ...]
    error_edit: tuple[str, str]


# The session edits a module that much of PyTorch imports and queries the
# files that use it, the way an editor does after each keystroke.
LSP_PROJECT = "pytorch"
LSP_REQUEST_TIMEOUT = 600  # seconds
LSP_SESSION = LanguageServerSession(
    open_files=(
        "torch/nn/modules/linear.py",
        "torch/nn/modules/transformer.py",
        "torch/optim/adam.py",
    ),
    error_edit=("return F.linear(input, self.weight, self.bias)", "return 1 + ''"),
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--target-dir",
        type=Path,
        default=DEFAULT_TARGET_DIR,
        help=f"Working directory for builds, profiles and the corpus (default: {DEFAULT_TARGET_DIR})",
    )
    subcommands = parser.add_subparsers(dest="command", required=True)
    pgo = subcommands.add_parser("pgo", help="Train and build with PGO")
    pgo.add_argument("--target", help="Host-native Rust target triple")
    pgo.add_argument(
        "--train-only",
        action="store_true",
        help="Stop after writing the merged profile and its hot count",
    )
    pgo.add_argument(
        "--debug",
        action="store_true",
        help="Use debug builds to quickly validate the pipeline",
    )
    bolt = subcommands.add_parser("bolt", help="Optimize a Linux binary with BOLT")
    bolt.add_argument("--llvm-dir", type=Path, required=True)
    bolt.add_argument(
        "--binary",
        type=Path,
        required=True,
        help="Binary to optimize in place; link it with `-Wl,--emit-relocs`",
    )
    bolt.add_argument("--wheel", type=Path, help="Wheel whose binary to replace")
    check = subcommands.add_parser(
        "check", help="Check that a binary was built with the merged profile"
    )
    check.add_argument("--binary", type=Path, required=True)
    subcommands.add_parser("corpus", help="Only prepare the training corpus")
    args = parser.parse_args()

    target_dir = args.target_dir.resolve()
    if args.command == "check":
        check_profile_use(target_dir / "pyrefly.profdata", args.binary)
        return
    corpus = prepare_corpus(target_dir)
    if args.command == "pgo":
        build_with_pgo(target_dir, corpus, args.target, args.train_only, args.debug)
    elif args.command == "bolt":
        optimize_with_bolt(
            target_dir, corpus, args.llvm_dir, args.binary.resolve(), args.wheel
        )


def build_with_pgo(
    target_dir: Path,
    corpus: dict[str, tuple[Path, Path]],
    target: str | None,
    train_only: bool,
    debug: bool,
) -> None:
    host = rustc_host()
    target = target or host
    # A static musl executable also runs on a glibc host of the same architecture.
    if target not in (host, host.replace("-gnu", "-musl")):
        raise RuntimeError(f"PGO training cannot run {target} executables on {host}")
    profdata = llvm_tool("llvm-profdata")
    profile_dir = target_dir / "profiles"
    shutil.rmtree(profile_dir, ignore_errors=True)
    profile_dir.mkdir(parents=True)

    environment = os.environ | {"CARGO_INCREMENTAL": "0"}
    if target.endswith("-apple-darwin"):
        # Keep Clang from instrumenting C dependencies through the Rust flags.
        for variable in ("CFLAGS", "CXXFLAGS"):
            environment[variable] = append_flags(
                environment.get(variable), "-fno-profile-generate -fno-profile-use"
            )

    # Thin LTO makes the instrumented build much faster, and LTO runs after
    # the instrumentation pass, so the profile still matches the fat-LTO
    # release build. It is passed to the `pyrefly` crate alone: changing the
    # Cargo profile would change the hash in every symbol name, and LLVM matches
    # profile records to functions by symbol name.
    instrumented_dir = target_dir / "instrumented"
    instrument = f"-Cprofile-generate={profile_dir}"
    if target == "aarch64-pc-windows-msvc":
        # MSVC's linker leaves the function names out of the profiles that an
        # instrumented ARM64 executable writes, so LLVM cannot read them.
        instrument += " -Clinker=rust-lld"
    run(
        [
            *cargo_build(target, debug, "rustc"),
            *(() if debug else ("--", "-Clto=thin")),
        ],
        environment=environment
        | {
            "CARGO_TARGET_DIR": str(instrumented_dir),
            "RUSTFLAGS": append_flags(environment.get("RUSTFLAGS"), instrument),
        },
    )
    binary = built_binary(instrumented_dir, target, debug)

    train(
        binary,
        corpus,
        lambda name: {
            "LLVM_PROFILE_FILE": str(profile_dir / f"pyrefly-{name}-%m-%p.profraw")
        },
    )
    profiles = sorted(profile_dir.glob("*.profraw"))
    merged = target_dir / "pyrefly.profdata"
    merge_profiles(profdata, profiles, merged)

    # LLVM optimizes code outside the 95th percentile of the profile counts for
    # size, but treats all code within the 99th as hot and inlines it
    # aggressively. A hot threshold at the 97th percentile measured faster than
    # either and keeps the binary smaller than the default.
    summary = subprocess.run(
        [
            profdata,
            "show",
            "--detailed-summary",
            "--detailed-summary-cutoffs=970000",
            merged,
        ],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    match = re.search(
        r"with count >= (\d+) account for 97% of the total counts", summary
    )
    if match is None or int(match.group(1)) <= 0:
        raise RuntimeError(f"Could not find a hot count in:\n{summary}")
    hot_count = match.group(1)
    (target_dir / "pyrefly.profile-hot-count").write_text(f"{hot_count}\n")
    print(f"Merged {len(profiles)} profiles; hot count {hot_count}", flush=True)
    if train_only:
        return

    run(
        cargo_build(target, debug),
        environment=environment
        | {
            "CARGO_TARGET_DIR": str(target_dir),
            "RUSTFLAGS": append_flags(
                environment.get("RUSTFLAGS"),
                f"-Cprofile-use={merged} "
                f"-Cllvm-args=--profile-summary-hot-count={hot_count}",
            ),
        },
    )
    binary = built_binary(target_dir, target, debug)
    # PE executables have no symbol table to check.
    if "windows" not in target:
        check_profile_use(merged, binary)
    print(f"PGO-optimized binary: {binary}")


# No block runs this often during training, which takes minutes. Some compilers
# record such counts anyway: rustc 1.99 on x86-64 records about 10^15 for one
# block of `num_bigint`'s `to_radix_le`, which training calls twice. One such
# count dominates the profile summary, so LLVM would treat everything else as
# cold.
IMPLAUSIBLE_COUNT = 10**12
# More functions than this with implausible counts means the profile is broken.
MAX_IMPLAUSIBLE_FUNCTIONS = 5


def merge_profiles(profdata: Path, profiles: list[Path], merged: Path) -> None:
    """Merge raw profiles, leaving out functions with implausible counts."""
    excluded: list[str] = []
    while True:
        # `--no-function` takes one pattern: a repeated flag replaces it.
        pattern = "|".join(re.escape(name) for name in excluded)
        run(
            [
                profdata,
                "merge",
                "--output",
                merged,
                *([f"--no-function=^({pattern})$"] if excluded else []),
                *profiles,
            ],
            environment=dict(os.environ),
        )
        top = subprocess.run(
            [profdata, "show", "--topn=1", merged],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        match = re.search(r"^  (\S+), max count = (\d+)$", top, re.MULTILINE)
        if match is None or int(match.group(2)) < IMPLAUSIBLE_COUNT:
            return
        if len(excluded) == MAX_IMPLAUSIBLE_FUNCTIONS:
            raise RuntimeError(
                f"Too many functions with implausible counts: {excluded}"
            )
        print(f"Leaving out implausible counts for {match.group(1)}", flush=True)
        excluded.append(match.group(1))


def check_profile_use(profile: Path, binary: Path) -> None:
    """Fail unless most functions that ran during training exist in `binary`.

    LLVM matches profile records to functions by symbol name, and silently
    ignores the profile for a function it cannot match. A build whose crate
    hashes differ from the instrumented build's, e.g. because its Cargo profile
    or target differs, ends up with almost no profile. Functions that were
    inlined everywhere have no symbol, so even a matching build finds only most
    of them.
    """
    covered = subprocess.run(
        [llvm_tool("llvm-profdata"), "show", "--covered", profile],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.split()
    # Local functions are recorded as `<module>;<symbol>`.
    names = [name.rpartition(";")[2] for name in covered]
    if not names:
        raise RuntimeError(f"{profile} records no functions that ran")
    symbols = subprocess.run(
        [llvm_tool("llvm-nm"), "--just-symbol-name", "--defined-only", binary],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.split()
    # Mach-O symbols carry an extra leading underscore.
    if sys.platform == "darwin":
        symbols = [symbol.removeprefix("_") for symbol in symbols]
    matched = len(set(names) & set(symbols)) / len(set(names))
    print(f"{matched:.0%} of the trained functions are in {binary}")
    if matched < 0.5:
        raise RuntimeError("The binary was not built with the trained profile")


def optimize_with_bolt(
    target_dir: Path,
    corpus: dict[str, tuple[Path, Path]],
    llvm_dir: Path,
    binary: Path,
    wheel: Path | None,
) -> None:
    bolt = llvm_dir / "bin" / "llvm-bolt"
    work = target_dir / "bolt"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    instrumented = work / "pyrefly-instrumented"
    run(
        [
            bolt,
            binary,
            "--instrument",
            f"--instrumentation-file={work / 'pyrefly.fdata'}",
            "--instrumentation-file-append-pid",
            f"--runtime-instrumentation-lib={llvm_dir / 'lib' / 'libbolt_rt_instr.a'}",
            "-o",
            instrumented,
        ],
        environment=dict(os.environ),
    )
    train(instrumented, corpus, lambda name: {})
    profiles = sorted(work.glob("pyrefly.fdata.*"))
    if not profiles:
        raise RuntimeError("BOLT training wrote no profiles")
    data = work / "merged.fdata"
    with data.open("wb") as stream:
        subprocess.run(
            [llvm_dir / "bin" / "merge-fdata", "-q", *profiles],
            stdout=stream,
            check=True,
        )

    optimized = work / "pyrefly-optimized"
    result = subprocess.run(
        [
            bolt,
            binary,
            f"--data={data}",
            "--reorder-blocks=ext-tsp",
            "--reorder-functions=cdsort",
            # Splitting functions into hot and cold parts measured no faster,
            # and it grows the executable by 3 MB: BOLT then keeps the original
            # unwind tables next to rewritten ones.
            "--jump-tables=move",
            "--icf=all",
            # Without padding, the rewritten code fits in the original `.text`
            # section rather than a new segment that grows the binary.
            "--align-functions=1",
            "--align-functions-max-bytes=0",
            "--align-blocks=false",
            "--use-old-text",
            "--no-huge-pages",
            "--hugify=false",
            # BOLT rewrites the symbol table to match the new layout, and panic
            # backtraces need it for function names. Do not strip the output
            # either: that would also close the gap that BOLT leaves in front of
            # the program headers, which older kernels need to find them.
            "--bolt-info=false",
            "-o",
            optimized,
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    print(result.stdout, flush=True)
    result.check_returncode()
    # Without room in the original `.text`, BOLT appends a second copy of the
    # code and still succeeds.
    if "using original .text for new code" not in result.stdout:
        raise RuntimeError("BOLT did not reuse the original .text section")
    check_program_header_address(llvm_tool("llvm-readobj"), optimized)
    symbols = subprocess.run(
        [llvm_tool("llvm-nm"), "--defined-only", optimized],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    if not symbols:
        raise RuntimeError("The BOLT output has no symbols for panic backtraces")
    # A miscompile in the rewritten code would ship in every channel, so the
    # rewritten executable must report what the one it came from reports.
    for project in CORPUS:
        checkout, venv = corpus[project.name]
        outputs = [
            subprocess.run(
                check_command(executable, project, venv),
                cwd=checkout,
                env=corpus_environment(venv),
                capture_output=True,
                text=True,
            )
            for executable in (binary, optimized)
        ]
        if outputs[0].returncode not in (0, 1) or (
            (outputs[0].returncode, outputs[0].stdout)
            != (outputs[1].returncode, outputs[1].stdout)
        ):
            raise RuntimeError(f"The BOLT output checks {project.name} differently")
    if wheel is not None:
        replace_wheel_binary(wheel, optimized)
    optimized.replace(binary)
    print(f"BOLT-optimized binary: {binary}")


def check_program_header_address(readobj: Path, binary: Path) -> None:
    """Fail unless the program headers are mapped at their file offset.

    Older kernels compute `AT_PHDR` from the file offset of the program header
    table, so a binary that maps it elsewhere crashes in the dynamic loader, or,
    for a static PIE, which has no `PT_PHDR`, in its own startup code.
    """
    (elf,) = json.loads(
        subprocess.check_output(
            [
                readobj,
                "--elf-output-style=JSON",
                "--file-headers",
                "--program-headers",
                binary,
            ]
        )
    )
    offset = elf["ElfHeader"]["ProgramHeaderOffset"]
    headers = [entry["ProgramHeader"] for entry in elf["ProgramHeaders"]]
    phdr = [h for h in headers if h["Type"]["Name"] == "PT_PHDR"]
    if phdr:
        address = phdr[0]["VirtualAddress"]
    else:
        loads = [
            h
            for h in headers
            if h["Type"]["Name"] == "PT_LOAD"
            and h["Offset"] <= offset < h["Offset"] + h["FileSize"]
        ]
        if len(loads) != 1:
            raise RuntimeError(
                f"Expected one loadable segment to contain the program headers "
                f"at {offset:#x}, found {len(loads)}"
            )
        address = loads[0]["VirtualAddress"] + offset - loads[0]["Offset"]
    if address != offset:
        raise RuntimeError(
            f"Program headers are mapped at {address:#x}, not {offset:#x}"
        )


def replace_wheel_binary(wheel: Path, binary: Path) -> None:
    """Replace the `pyrefly` executable in a wheel and update its `RECORD`."""
    payload = binary.read_bytes()
    digest = base64.urlsafe_b64encode(hashlib.sha256(payload).digest()).rstrip(b"=")
    staged = wheel.with_suffix(".tmp")
    with zipfile.ZipFile(wheel) as source, zipfile.ZipFile(staged, "w") as destination:
        names = source.namelist()
        (script,) = (n for n in names if n.endswith(".data/scripts/pyrefly"))
        (record,) = (n for n in names if n.endswith(".dist-info/RECORD"))
        rows = list(csv.reader(io.StringIO(source.read(record).decode())))
        (script_row,) = (row for row in rows if row[0] == script)
        script_row[1:] = [f"sha256={digest.decode()}", str(len(payload))]
        updated = io.StringIO()
        csv.writer(updated, lineterminator="\n").writerows(rows)
        replacements = {script: payload, record: updated.getvalue().encode()}
        for entry in source.infolist():
            destination.writestr(
                entry, replacements.get(entry.filename) or source.read(entry)
            )
    staged.replace(wheel)


def prepare_corpus(target_dir: Path) -> dict[str, tuple[Path, Path]]:
    """Check out each corpus project and create a virtual environment for it.

    Returns the checkout and the virtual environment of each project by name.
    A checkout that is already at its pinned revision is reused, so
    `<target_dir>/corpus` can be cached between runs. The environments are
    recreated every time, because they refer to the interpreter by its path.
    """
    uv = shutil.which("uv")
    if uv is None:
        raise RuntimeError("uv is required to install the corpus dependencies")
    git_environment = os.environ | {
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_LFS_SKIP_SMUDGE": "1",
    }
    corpus = {}
    for project in CORPUS:
        checkout = target_dir / "corpus" / project.name
        # Some PyTorch paths exceed the default Windows path length limit.
        git = ["git", "-c", "core.longpaths=true", "-C", str(checkout)]
        if not (checkout / ".git").is_dir():
            checkout.mkdir(parents=True, exist_ok=True)
            run([*git, "init", "--quiet"], environment=git_environment)
            run(
                [
                    *git,
                    "remote",
                    "add",
                    "origin",
                    f"https://github.com/{project.repository}.git",
                ],
                environment=git_environment,
            )
        # The checkout is a partial clone, so updating the working tree
        # downloads the files it is missing.
        run(
            [*git, "sparse-checkout", "set", "--cone", *project.sparse_paths],
            environment=git_environment,
            attempts=NETWORK_ATTEMPTS,
        )
        head = subprocess.run(
            [*git, "rev-parse", "--verify", "--quiet", "HEAD"],
            capture_output=True,
            text=True,
        ).stdout.strip()
        if head != project.revision:
            run(
                [
                    *git,
                    "fetch",
                    "--quiet",
                    "--depth=1",
                    "--filter=blob:none",
                    "origin",
                    project.revision,
                ],
                environment=git_environment,
                attempts=NETWORK_ATTEMPTS,
            )
        # Forced even at the pinned revision, so that a restored cache matches
        # the revision exactly.
        run(
            [*git, "checkout", "--quiet", "--force", "--detach", project.revision],
            environment=git_environment,
            attempts=NETWORK_ATTEMPTS,
        )

        venv = target_dir / "environments" / project.name
        run(
            [
                uv,
                "venv",
                "--quiet",
                "--clear",
                "--python",
                project.python_version,
                venv,
            ],
            environment=dict(os.environ),
            attempts=NETWORK_ATTEMPTS,
        )
        if project.dependencies:
            run(
                [
                    uv,
                    "pip",
                    "install",
                    "--quiet",
                    "--python",
                    venv,
                    "--only-binary",
                    ":all:",
                    "--exclude-newer",
                    DEPENDENCY_EXCLUDE_NEWER,
                    *project.dependencies,
                ],
                environment=dict(os.environ),
                attempts=NETWORK_ATTEMPTS,
            )
        corpus[project.name] = (checkout, venv)
    return corpus


def check_command(binary: Path, project: Project, venv: Path) -> list[str | Path]:
    """Return the `pyrefly check` command for a corpus project.

    Pyrefly reads the project's own configuration, in the job that builds the
    release executables, so that configuration must not make Pyrefly run a
    command. The interpreter is forced to the project's virtual environment;
    review the Pyrefly configuration of a new corpus revision for other options
    that run commands. The language-server session cannot force it.
    """
    python = venv / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
    return [
        binary,
        "check",
        "--output-format=min-text",
        "--summary=none",
        f"--python-interpreter-path={python}",
        *project.check_paths,
    ]


def corpus_environment(venv: Path) -> dict[str, str]:
    """Return the environment for running Pyrefly on a corpus project."""
    return {
        k: v
        for k, v in os.environ.items()
        if k not in EXCLUDED_ENVIRONMENT_VARIABLES and not k.startswith("PYREFLY_")
    } | {"VIRTUAL_ENV": str(venv)}


def train(
    binary: Path,
    corpus: dict[str, tuple[Path, Path]],
    profile_environment: Callable[[str], dict[str, str]],
) -> None:
    """Run the training workload with `binary`.

    `profile_environment` gives the extra environment variables for each
    workload, keyed by a name that is unique among the workloads.
    """
    for project in CORPUS:
        checkout, venv = corpus[project.name]
        print(f"Training on {project.name}", flush=True)
        started = time.monotonic()
        # Pyrefly exits with 1 when it finds type errors, which every project has.
        run(
            check_command(binary, project, venv),
            environment=corpus_environment(venv) | profile_environment(project.name),
            cwd=checkout,
            allowed_exit_codes=(0, 1),
            quiet=True,
        )
        print(f"  {time.monotonic() - started:.1f}s", flush=True)

    print("Training on a language-server session", flush=True)
    started = time.monotonic()
    checkout, venv = corpus[LSP_PROJECT]
    language_server_session(
        binary,
        checkout,
        corpus_environment(venv) | profile_environment("lsp"),
        LSP_SESSION,
    )
    print(f"  {time.monotonic() - started:.1f}s", flush=True)


def language_server_session(  # noqa: C901
    binary: Path,
    root: Path,
    environment: dict[str, str],
    session: LanguageServerSession,
) -> None:
    """Open files, query them, and repeatedly introduce and fix a type error."""
    process = subprocess.Popen(
        [binary, "lsp"],
        cwd=root,
        env=environment,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    assert process.stdin is not None and process.stdout is not None
    stdin, stdout = process.stdin, process.stdout
    # A message, the error that made one unreadable, or `None` once the server
    # has exited.
    messages: queue.Queue[dict[str, object] | ValueError | None] = queue.Queue()

    def read_messages() -> None:
        try:
            while True:
                length = None
                while (line := stdout.readline()) not in (b"\r\n", b"\n"):
                    if not line:
                        messages.put(None)
                        return
                    name, _, value = line.decode().partition(":")
                    if name.lower() == "content-length":
                        length = int(value)
                if length is None:
                    raise ValueError("no Content-Length header")
                messages.put(json.loads(stdout.read(length)))
        except ValueError as error:
            messages.put(error)

    def send(message: dict[str, object]) -> None:
        payload = json.dumps({"jsonrpc": "2.0", **message}).encode()
        stdin.write(b"Content-Length: %d\r\n\r\n" % len(payload) + payload)
        stdin.flush()

    next_id = 0

    def request(method: str, params: Mapping[str, object] | None) -> object:
        nonlocal next_id
        next_id += 1
        send({"id": next_id, "method": method, "params": params})
        deadline = time.monotonic() + LSP_REQUEST_TIMEOUT
        while True:
            try:
                message = messages.get(timeout=max(0, deadline - time.monotonic()))
            except queue.Empty:
                raise RuntimeError(
                    f"The language server did not answer {method} within "
                    f"{LSP_REQUEST_TIMEOUT} seconds"
                ) from None
            if message is None:
                raise RuntimeError(f"The language server exited during {method}")
            if isinstance(message, ValueError):
                raise RuntimeError(
                    f"The language server sent an unreadable message during {method}: "
                    f"{message}"
                )
            if "method" in message and "id" in message:
                # Requests from the server, e.g. `workspace/configuration`.
                send({"id": message["id"], "result": None})
            elif message.get("id") == next_id:
                if "error" in message:
                    raise RuntimeError(f"{method} failed: {message['error']}")
                return message.get("result")

    def notify(method: str, params: Mapping[str, object]) -> None:
        send({"method": method, "params": params})

    def document(path: str) -> dict[str, str]:
        return {"uri": (root / path).as_uri()}

    def diagnostics(path: str) -> list[object]:
        result = request("textDocument/diagnostic", {"textDocument": document(path)})
        assert isinstance(result, dict)
        return result["items"]

    threading.Thread(target=read_messages, daemon=True).start()
    try:
        request(
            "initialize",
            {
                "processId": os.getpid(),
                "rootUri": root.as_uri(),
                "workspaceFolders": [{"uri": root.as_uri(), "name": root.name}],
                "capabilities": {"textDocument": {"diagnostic": {}}},
            },
        )
        notify("initialized", {})
        texts = {
            path: (root / path).read_text(encoding="utf-8")
            for path in session.open_files
        }
        for path, text in texts.items():
            notify(
                "textDocument/didOpen",
                {
                    "textDocument": document(path)
                    | {"languageId": "python", "version": 1, "text": text}
                },
            )
            diagnostics(path)

        edited = session.open_files[0]
        original = texts[edited]
        old, new = session.error_edit
        if original.count(old) != 1:
            raise RuntimeError(f"Expected one {old!r} in {edited}")
        lines = original.splitlines()
        classes = [i for i, line in enumerate(lines) if line.startswith("class ")]
        if not classes:
            raise RuntimeError(f"Expected a class in {edited}")
        for path, text in texts.items():
            for line_number in range(0, len(text.splitlines()), 40):
                position = {
                    "textDocument": document(path),
                    "position": {"line": line_number, "character": 8},
                }
                request("textDocument/hover", position)
                request("textDocument/definition", position)
                request("textDocument/completion", position)
        position = {
            "textDocument": document(edited),
            "position": {
                "line": classes[0],
                "character": 7,
            },
        }
        request(
            "textDocument/references",
            position | {"context": {"includeDeclaration": True}},
        )
        request("workspace/symbol", {"query": "Module"})

        baseline = len(diagnostics(edited))
        for version in range(2, 14):
            broken = version % 2 == 0
            notify(
                "textDocument/didChange",
                {
                    "textDocument": document(edited) | {"version": version},
                    "contentChanges": [
                        {"text": original.replace(old, new) if broken else original}
                    ],
                },
            )
            if (len(diagnostics(edited)) > baseline) != broken:
                raise RuntimeError("Diagnostics did not follow the edit")
            for path in session.open_files:
                diagnostics(path)

        request("shutdown", None)
        notify("exit", {})
        if process.wait(timeout=60) != 0:
            raise RuntimeError(f"The language server exited with {process.returncode}")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def cargo_build(target: str, debug: bool, command: str = "build") -> list[str]:
    return [
        "cargo",
        command,
        *(() if debug else ("--release",)),
        "--locked",
        "--manifest-path",
        str(MANIFEST),
        "--package",
        "pyrefly",
        "--bin",
        "pyrefly",
        "--target",
        target,
    ]


def built_binary(target_dir: Path, target: str, debug: bool) -> Path:
    name = "pyrefly.exe" if "windows" in target else "pyrefly"
    return target_dir / target / ("debug" if debug else "release") / name


def rustc_host() -> str:
    output = subprocess.check_output(["rustc", "-vV"], cwd=REPOSITORY_ROOT, text=True)
    match = re.search(r"^host: (\S+)", output, re.MULTILINE)
    if match is None:
        raise RuntimeError(f"No host in `rustc -vV` output:\n{output}")
    return match.group(1)


def llvm_tool(name: str) -> Path:
    """Find an LLVM tool from the `llvm-tools` component of the Rust toolchain."""
    sysroot = subprocess.check_output(
        ["rustc", "--print", "sysroot"], cwd=REPOSITORY_ROOT, text=True
    ).strip()
    tool = Path(sysroot) / "lib" / "rustlib" / rustc_host() / "bin" / name
    tool = tool.with_suffix(".exe") if sys.platform == "win32" else tool
    if not tool.is_file():
        raise RuntimeError(f"{tool} not found; run `rustup component add llvm-tools`")
    return tool


def append_flags(existing: str | None, additional: str) -> str:
    return f"{existing} {additional}" if existing else additional


def run(
    command: Sequence[str | Path],
    *,
    environment: dict[str, str],
    cwd: Path = REPOSITORY_ROOT,
    allowed_exit_codes: tuple[int, ...] = (0,),
    quiet: bool = False,
    attempts: int = 1,
) -> None:
    print(f"> {shlex.join(map(str, command))}", flush=True)
    output = subprocess.DEVNULL if quiet else None
    for attempt in range(1, attempts + 1):
        completed = subprocess.run(command, cwd=cwd, env=environment, stdout=output)
        if completed.returncode in allowed_exit_codes:
            return
        if attempt == attempts:
            raise subprocess.CalledProcessError(completed.returncode, command)
        delay = 5 * 2 ** (attempt - 1)
        print(f"Exit code {completed.returncode}; retrying in {delay}s", flush=True)
        time.sleep(delay)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"error: {error}") from error
