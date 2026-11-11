/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

fn main() {
    // Rust gives a wasm module a 1 MiB stack, which about 400 levels of nested
    // expressions overflow. With 8 MiB, the stack of the JavaScript engine,
    // which also holds the wasm call frames, overflows first.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg-cdylib=-zstack-size=8388608");
    }
}
