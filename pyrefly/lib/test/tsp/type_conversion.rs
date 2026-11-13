/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for transaction-aware TSP type conversion.

use dupe::Dupe;
use lsp_types::Uri;
use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::sys_info::PythonPlatform;
use pyrefly_python::sys_info::PythonVersion;
use pyrefly_python::sys_info::SysInfo;
use pyrefly_util::thread_pool::TEST_THREAD_COUNT;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use tempfile::TempDir;
use tsp_types::Declaration;
use tsp_types::DeclarationCategory;
use tsp_types::DeclarationKind;
use tsp_types::Node;
use tsp_types::Position;
use tsp_types::Range;
use tsp_types::RegularDeclaration;
use tsp_types::Type as TspType;

use crate::lsp::module_helpers::to_real_path;
use crate::module::finder::DirEntryCache;
use crate::module::finder::FindImportOptions;
use crate::module::finder::find_import;
use crate::state::require::Require;
use crate::state::state::State;
use crate::test::util::TestEnv;
use crate::tsp::convert_type_in_transaction;

/// Re-exported declarations resolve to their original definition, using the
/// source handle's `SysInfo` when the transaction holds more than one.
#[test]
fn test_converted_reexports_keep_declarations_and_source_sysinfo() {
    let mut env = TestEnv::new();
    let directory = TempDir::new().unwrap();
    let source = "from facade import Widget, identity\nitem = Widget()\nchosen = identity\n";
    for (module, contents) in [
        (
            "library",
            "class Widget:\n    pass\n\ndef identity(value: Widget) -> Widget:\n    return value\n",
        ),
        ("facade", "from library import Widget, identity\n"),
        ("main", source),
    ] {
        let path = directory.path().join(format!("{module}.py"));
        env.add_with_path(module, path.to_str().unwrap(), contents);
    }
    let config = env.config();
    let handle = |module: &str, sys_info: &SysInfo| {
        let name = ModuleName::from_str(module);
        let path = find_import(&config, name, FindImportOptions::new(&DirEntryCache::new()))
            .finding()
            .unwrap();
        Handle::new(name, path, sys_info.dupe())
    };
    let handles = [PythonPlatform::windows(), PythonPlatform::mac()]
        .map(|platform| handle("main", &SysInfo::new(PythonVersion::default(), platform)));
    let state = State::new(env.config_finder(), TEST_THREAD_COUNT);
    let mut transaction = state.new_transaction(Require::Exports, None);
    transaction.set_memory(env.get_memory());
    transaction.run(&handles, Require::Everything, None);
    assert!(
        transaction
            .get_errors(handles.iter())
            .collect_display_errors()
            .is_empty()
    );

    for source_handle in &handles {
        let library_handle = handle("library", source_handle.sys_info());
        let uri = Uri::from_file_path(to_real_path(library_handle.path()).unwrap())
            .unwrap()
            .to_string();
        let convert = |variable: &str| {
            let offset = TextSize::new(source.find(variable).unwrap() as u32);
            let range = TextRange::at(offset, TextSize::new(variable.len() as u32));
            let ty = transaction
                .get_computed_type_at_range(source_handle, range)
                .unwrap();
            convert_type_in_transaction(&transaction, source_handle, &ty)
        };
        let declaration = |category, name: &str, line, character| {
            Declaration::Regular(RegularDeclaration {
                category,
                kind: DeclarationKind::Regular,
                name: Some(name.to_owned()),
                node: Node {
                    range: Range {
                        start: Position { line, character },
                        end: Position {
                            line,
                            character: character + name.len() as u32,
                        },
                    },
                    uri: uri.clone(),
                },
            })
        };

        let TspType::Class(widget) = convert("item") else {
            panic!("`item` should convert to a class type");
        };
        assert_eq!(
            widget.declaration,
            declaration(DeclarationCategory::Class, "Widget", 0, 6)
        );
        let TspType::Function(identity) = convert("chosen") else {
            panic!("`chosen` should convert to a function type");
        };
        assert_eq!(
            identity.declaration,
            declaration(DeclarationCategory::Function, "identity", 3, 4)
        );
    }
}
