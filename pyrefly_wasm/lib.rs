/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The `wasm32-wasip1` module behind the Pyrefly sandbox.
//!
//! `pyrefly_wasm.js` wraps these exports in the JavaScript `State` class. Each
//! call passes a JSON request in guest memory and receives a JSON response in
//! guest memory. The host frees the response with `pyrefly_free`.
//!
//! A panic aborts, which traps. A trap can stop Rust code in the middle of any
//! operation, so the instance is not usable after it.

use std::cell::RefCell;

use pyrefly::playground::Playground;
use pyrefly::playground::Position;
use pyrefly::playground::Range;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use starlark_map::small_map::SmallMap;

#[derive(Deserialize)]
#[serde(tag = "method", content = "args", rename_all = "camelCase")]
enum Request {
    UpdateSandboxFiles(SmallMap<String, String>, bool),
    UpdateSingleFile(String, String),
    SetActiveFile(String),
    GetErrors,
    Hover(i32, i32),
    SemanticTokens(Option<Range>),
    SemanticTokensLegend,
    GotoDefinition(i32, i32, bool),
    AutoComplete(i32, i32),
    InlayHint(bool),
}

thread_local! {
    /// Live playgrounds, indexed by the handle that `pyrefly_new` returns.
    /// Handles are never reused. A slot is `None` after `pyrefly_drop`.
    static PLAYGROUNDS: RefCell<Vec<Option<Playground>>> = const { RefCell::new(Vec::new()) };
}

fn dispatch(playground: &mut Playground, request: Request) -> serde_json::Result<Value> {
    match request {
        Request::UpdateSandboxFiles(files, force_update) => {
            serde_json::to_value(playground.update_sandbox_files(files, force_update))
        }
        Request::UpdateSingleFile(filename, content) => {
            playground.update_single_file(filename, content);
            Ok(Value::Null)
        }
        Request::SetActiveFile(filename) => {
            playground.set_active_file(&filename);
            Ok(Value::Null)
        }
        Request::GetErrors => serde_json::to_value(playground.get_errors()),
        Request::Hover(line, column) => {
            serde_json::to_value(playground.hover(Position::new(line, column)))
        }
        Request::SemanticTokens(range) => serde_json::to_value(playground.semantic_tokens(range)),
        Request::SemanticTokensLegend => serde_json::to_value(playground.semantic_tokens_legend()),
        Request::GotoDefinition(line, column, with_contents) => {
            let results = playground.goto_definition(Position::new(line, column), with_contents);
            if results.is_empty() {
                Ok(Value::Null)
            } else {
                serde_json::to_value(results)
            }
        }
        Request::AutoComplete(line, column) => {
            serde_json::to_value(playground.autocomplete(Position::new(line, column)))
        }
        Request::InlayHint(call_argument_names) => {
            serde_json::to_value(playground.inlay_hint(call_argument_names))
        }
    }
}

/// Moves `bytes` to the host as a packed `(pointer << 32) | length`.
fn into_host(bytes: Vec<u8>) -> u64 {
    let bytes = bytes.into_boxed_slice();
    let len = bytes.len() as u64;
    (Box::into_raw(bytes) as *mut u8 as u64) << 32 | len
}

/// Returns the packed JSON response `{"ok": version}` with the Pyrefly version.
#[unsafe(no_mangle)]
pub extern "C" fn pyrefly_version() -> u64 {
    into_host(json!({ "ok": pyrefly::VERSION }).to_string().into_bytes())
}

/// Allocates `len` bytes for the host to write a request into.
#[unsafe(no_mangle)]
pub extern "C" fn pyrefly_alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// Frees memory from `pyrefly_alloc` or a response from `pyrefly_call`.
///
/// # Safety
///
/// `ptr` and `len` must describe a buffer that this module gave to the host.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyrefly_free(ptr: *mut u8, len: usize) {
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
}

/// Creates a playground for the Python version in the UTF-8 string at
/// `ptr`/`len`, and frees that string. Returns the packed JSON response
/// `{"ok": handle}` or `{"err": message}`.
///
/// # Safety
///
/// `ptr` and `len` must describe a buffer from `pyrefly_alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyrefly_new(ptr: *mut u8, len: usize) -> u64 {
    let version = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) };
    let response = match Playground::new(Some(&String::from_utf8_lossy(&version))) {
        Ok(playground) => PLAYGROUNDS.with_borrow_mut(|playgrounds| {
            playgrounds.push(Some(playground));
            json!({ "ok": playgrounds.len() - 1 })
        }),
        Err(e) => json!({ "err": e }),
    };
    into_host(response.to_string().into_bytes())
}

/// Drops the playground for `handle`.
#[unsafe(no_mangle)]
pub extern "C" fn pyrefly_drop(handle: usize) {
    PLAYGROUNDS.with_borrow_mut(|playgrounds| playgrounds[handle] = None);
}

/// Runs the JSON `Request` at `ptr`/`len` against the playground for `handle`,
/// and frees the request. Returns the packed JSON response `{"ok": value}` or
/// `{"err": message}`.
///
/// # Safety
///
/// `ptr` and `len` must describe a buffer from `pyrefly_alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyrefly_call(handle: usize, ptr: *mut u8, len: usize) -> u64 {
    let request = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) };
    let result = serde_json::from_slice(&request).and_then(|request| {
        PLAYGROUNDS.with_borrow_mut(|playgrounds| {
            let playground = playgrounds[handle]
                .as_mut()
                .expect("the host should not call a playground that it dropped");
            dispatch(playground, request)
        })
    });
    let response = match result {
        Ok(value) => json!({ "ok": value }),
        Err(e) => json!({ "err": e.to_string() }),
    };
    into_host(response.to_string().into_bytes())
}
