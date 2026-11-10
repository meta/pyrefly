#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Take the `pyrefly` executables out of the release wheels.

The GitHub release and the VS Code extension ship the executables from the
PyPI wheels, byte for byte, rather than building their own.

    python3 scripts/wheel_binaries.py archive <wheel dir> <output dir>
    python3 scripts/wheel_binaries.py extract <wheel dir> <name> <output dir>

`archive` packages every executable as a GitHub release archive with a `.sha256`
file in the format of `sha256sum`, and fails unless every platform has a wheel.
`extract` copies one executable, chosen by its archive name (e.g.
`pyrefly-linux-x86_64`), into a directory, and needs only that platform's wheel.
"""

import hashlib
import io
import re
import sys
import tarfile
import zipfile
from pathlib import Path

# The first platform tag of each wheel, mapped to the name of its archive.
ARCHIVE_NAMES = {
    r"manylinux\w*_x86_64": "pyrefly-linux-x86_64",
    r"manylinux\w*_i686": "pyrefly-linux-i686",
    r"manylinux\w*_aarch64": "pyrefly-linux-arm64",
    r"manylinux\w*_ppc64le": "pyrefly-linux-ppc64le",
    r"manylinux\w*_armv7l": "pyrefly-linux-armv7",
    r"musllinux\w*_x86_64": "pyrefly-linux-x86_64-musl",
    r"musllinux\w*_aarch64": "pyrefly-linux-arm64-musl",
    r"macosx\w*_x86_64": "pyrefly-macos-x86_64",
    r"macosx\w*_arm64": "pyrefly-macos-arm64",
    r"win_amd64": "pyrefly-windows-x86_64",
    r"win32": "pyrefly-windows-i686",
    r"win_arm64": "pyrefly-windows-arm64",
}


def archive_name(wheel: Path) -> str | None:
    """Return the archive name for a wheel's first platform tag, if it has one."""
    platform = wheel.stem.split("-")[-1].split(".")[0]
    return next(
        (
            name
            for pattern, name in ARCHIVE_NAMES.items()
            if re.fullmatch(pattern, platform)
        ),
        None,
    )


def read_executable(wheel: Path) -> tuple[str, bytes]:
    """Return the file name and contents of the `pyrefly` executable in a wheel."""
    with zipfile.ZipFile(wheel) as source:
        scripts = [
            n
            for n in source.namelist()
            if re.search(r"\.data/scripts/pyrefly(\.exe)?$", n)
        ]
        if len(scripts) != 1:
            raise SystemExit(
                f"Expected one pyrefly executable in {wheel}, found {scripts}"
            )
        return Path(scripts[0]).name, source.read(scripts[0])


def archive(wheel_dir: Path, output_dir: Path) -> None:
    found: dict[str, Path] = {}
    for wheel in sorted(wheel_dir.glob("*.whl")):
        name = archive_name(wheel)
        if name is None:
            raise SystemExit(
                f"No archive name for {wheel.name}; add it to ARCHIVE_NAMES"
            )
        if name in found:
            raise SystemExit(f"More than one wheel for {name}")
        found[name] = wheel
    missing = sorted(set(ARCHIVE_NAMES.values()) - set(found))
    if missing:
        raise SystemExit(f"No wheels for {missing}")
    output_dir.mkdir(parents=True, exist_ok=True)
    for name, wheel in found.items():
        file_name, payload = read_executable(wheel)
        if file_name.endswith(".exe"):
            path = output_dir / f"{name}.zip"
            with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as destination:
                destination.writestr(file_name, payload)
        else:
            path = output_dir / f"{name}.tar.gz"
            info = tarfile.TarInfo(file_name)
            info.size = len(payload)
            info.mode = 0o755
            with tarfile.open(path, "w:gz") as destination:
                destination.addfile(info, io.BytesIO(payload))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        path.with_name(f"{path.name}.sha256").write_text(f"{digest}  {path.name}\n")
        print(path)


def extract(wheel_dir: Path, name: str, output_dir: Path) -> None:
    matching = [w for w in sorted(wheel_dir.glob("*.whl")) if archive_name(w) == name]
    if len(matching) != 1:
        raise SystemExit(
            f"Expected one wheel for {name} in {wheel_dir}, found {matching}"
        )
    file_name, payload = read_executable(matching[0])
    output_dir.mkdir(parents=True, exist_ok=True)
    path = output_dir / file_name
    path.write_bytes(payload)
    path.chmod(0o755)
    print(path)


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    if command == "archive":
        archive(*map(Path, args))
    elif command == "extract":
        wheel_dir, name, output_dir = args
        extract(Path(wheel_dir), name, Path(output_dir))
    else:
        raise SystemExit(__doc__)
