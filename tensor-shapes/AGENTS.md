# Shape-stub package guidance

## Versions and dependencies

Published shape-stub packages and shape-extensions use `version = "0.0.0"` in
their source `pyproject.toml`. Dependencies on other packages maintained here
use exact `==0.0.0` pins, including dependencies in `[project.optional-dependencies]`.
Runtime-library requirements are separate from these same-release stub pins.

`.github/workflows/ci_shape_stubs.yml` copies the packages to temporary build
directories and stamps their versions and inter-package pins with the version
from `version.bzl`. `publish_shape_stubs.yml` reuses this workflow. Keep the
source placeholders; do not replace them with a release version in the checkout.

When adding or changing a dependency, update the corresponding `stamp(...)`
dependency list, including every optional extra. The replacement requires each
listed pin to occur exactly once. Verify both wheel `METADATA` and sdist
`PKG-INFO`: package versions and inter-package pins must match the release
version, with no remaining `==0.0.0` requirements.

## Package layouts

A distribution name, source directory, and installed stub directory need not
have the same name. When changing a layout, check Hatch's wheel/sdist inclusion
and path mapping, CI archive-path assertions and smoke tests, and the shape-test
runner's import/search paths together.

Run the workflow's package builds and artifact verification after packaging
changes. Successful `--no-deps` smoke installs alone do not validate dependency
pins or runtime dependency resolution.
