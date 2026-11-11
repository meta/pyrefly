/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_python::ignore::TypeIgnoreUnknownTagBehavior;

use crate::test::util::TestEnv;

pub fn pydantic_env() -> TestEnv {
    let path = std::env::var("PYDANTIC_TEST_PATH").expect("PYDANTIC_TEST_PATH must be set");
    // Modules omitted from this partial fixture are intentionally approximated as gradual.
    let env = TestEnv::new_with_site_package_paths(&[&path]).with_replace_imports_with_any(&[
        "annotated_types",
        "annotated_types.*",
        "pydantic_core",
        "pydantic_core.*",
        "typing_inspection",
        "typing_inspection.*",
        "pydantic._migration",
        "pydantic.aliases",
        "pydantic.errors",
        "pydantic.warnings",
        "pydantic._internal._generate_schema",
        "pydantic._internal._utils",
        "pydantic._internal._config",
        "pydantic._internal._decorators",
        "pydantic._internal._mock_val_ser",
        "pydantic._internal._namespace_utils",
        "pydantic._internal._typing_extra",
        "pydantic._internal._dataclasses",
    ]);
    let behavior = TypeIgnoreUnknownTagBehavior::Suppress;
    env.with_type_ignore_unknown_tag_behavior(behavior)
}

#[macro_export]
macro_rules! pydantic_testcase {
    (bug = $explanation:literal, $name:ident, $contents:literal,) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            $crate::test::util::testcase_for_macro(
                $crate::test::pydantic::util::pydantic_env(),
                $contents,
                file!(),
                line!(),
            )
        }
    };
    ($name:ident, $contents:literal,) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            $crate::test::util::testcase_for_macro(
                $crate::test::pydantic::util::pydantic_env(),
                $contents,
                file!(),
                line!() - 1,
            )
        }
    };
}
