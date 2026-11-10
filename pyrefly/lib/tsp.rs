/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Internal TSP support shared by the native server and WASI hosts.
//!
//! The Rust interface is unstable. WASI hosts can use transaction-aware type
//! conversion without linking the native server or its transports.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod requests;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod server;
pub(crate) mod type_conversion;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod type_facts;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod validation;

pub use type_conversion::convert_type_in_transaction;
