/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::env;

use crate::test::util::TestEnv;

pub fn django_env() -> TestEnv {
    django_env_with_infer_with_first_use(true)
}

pub fn django_env_with_infer_with_first_use(infer_with_first_use: bool) -> TestEnv {
    let path = env::var("DJANGO_TEST_PATH").expect("DJANGO_TEST_PATH must be set");
    TestEnv::new_with_infer_with_first_use(infer_with_first_use)
        .with_site_package_paths(vec![path.into()])
}

#[macro_export]
macro_rules! django_testcase {
    (bug = $explanation:literal, $name:ident, $contents:literal,) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            $crate::test::util::testcase_for_macro(
                $crate::test::django::util::django_env(),
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
                $crate::test::django::util::django_env(),
                $contents,
                file!(),
                line!() - 1,
            )
        }
    };
}
