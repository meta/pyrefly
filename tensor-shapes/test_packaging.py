#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Build packaging regressions with `python -m unittest discover -s tensor-shapes -p test_packaging.py`.

The executing interpreter must have build and hatchling installed.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path


class ShapeStubPackagingTest(unittest.TestCase):
    def build(self, source: Path, output: Path, kind: str) -> Path:
        """Build one artifact without downloading dependencies or touching source."""
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "build",
                "--no-isolation",
                f"--{kind}",
                "--outdir",
                str(output),
                str(source),
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        artifacts = list(output.glob("*.whl" if kind == "wheel" else "*.tar.gz"))
        self.assertEqual(len(artifacts), 1, artifacts)
        return artifacts[0]

    def stub_files(self, wheel: Path, root: str, representative: str) -> set[str]:
        """Check the installed stub namespace and representative package contents."""
        with zipfile.ZipFile(wheel) as archive:
            names = {name for name in archive.namelist() if ".dist-info/" not in name}
            self.assertEqual({name.split("/")[0] for name in names}, {root})
            for member in ("__init__.pyi", "_shapes.pyi", "py.typed", representative):
                self.assertIn(f"{root}/{member}", names)
            return names

    def test_wheel_and_sdist_rebuild_use_the_same_stub_namespace(self) -> None:
        for package, module, representative in (
            ("pyrefly-numpy-stubs", "numpy", "linalg/__init__.pyi"),
            ("pyrefly-torch-stubs", "torch", "nn/__init__.pyi"),
            ("jax-pyrefly-stubs", "jax", "numpy/__init__.pyi"),
            ("pyrefly-einops-stubs", "einops", "layers/torch.pyi"),
        ):
            with self.subTest(package=package), tempfile.TemporaryDirectory() as tmp:
                source = Path(__file__).resolve().parent / package
                work = Path(tmp)
                project = work / "project"
                project.mkdir()
                for filename in ("pyproject.toml", "README.md", "LICENSE"):
                    shutil.copyfile(source / filename, project / filename)
                shutil.copytree(source / f"{module}-stubs", project / f"{module}-stubs")
                wheel = self.build(project, work / "wheel", "wheel")
                sdist = self.build(project, work / "sdist", "sdist")
                unpacked = work / "unpacked"
                with tarfile.open(sdist) as archive:
                    archive.extractall(unpacked, filter="data")
                projects = list(unpacked.iterdir())
                self.assertEqual(len(projects), 1, projects)
                rebuilt = self.build(projects[0], work / "rebuilt", "wheel")
                root = f"{module}-shape-stubs"
                self.assertEqual(
                    self.stub_files(wheel, root, representative),
                    self.stub_files(rebuilt, root, representative),
                )
