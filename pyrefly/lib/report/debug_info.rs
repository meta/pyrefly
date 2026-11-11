/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;
use std::sync::Arc;

use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::display::DisplayWithCtx;
use pyrefly_util::prelude::SliceExt;
use pyrefly_util::visit::VisitMut;
use serde::Deserialize;
use serde::Serialize;
use starlark_map::small_map::SmallMap;

use crate::alt::answers::AnswerEntry;
use crate::alt::answers::AnswerTable;
use crate::alt::answers::Answers;
use crate::alt::answers::SolutionsEntry;
use crate::alt::answers::SolutionsTable;
use crate::binding::binding::Keyed;
use crate::binding::bindings::BindingEntry;
use crate::binding::bindings::BindingTable;
use crate::binding::table::TableKeyed;
use crate::error::collector::CollectedErrors;
use crate::module::module_info::ModuleInfo;
use crate::state::state::Transaction;
use crate::table_for_each;

pub fn debug_info(transaction: &Transaction, handles: &[Handle], is_javascript: bool) -> String {
    fn f(transaction: &Transaction, handles: &[Handle]) -> Option<Vec<Arc<Answers>>> {
        handles.iter().map(|x| transaction.get_answers(x)).collect()
    }

    let owned = f(transaction, handles).expect("Everything to be computed for debug info");
    let errors = transaction.get_errors(handles).collect_errors();
    let debug_info = DebugInfo::new(&owned.map(|x| (x.bindings().module(), &**x)), &errors);
    let mut output = serde_json::to_string(&debug_info).unwrap();

    // It's super handy to be able to diff the output, which we can do most easily if each binding is on its own line.
    // Doing that with Serde isn't possible (you can make it pretty, or you can make it compact, but not pretty on weird metrics),
    // therefore we mangle the code after with string manipulation.
    output = output
        .replace("{\"key\":", "\n{\"key\":")
        .replace("],\"errors\":", "\n],\"errors\":");

    if is_javascript {
        output = format!("var data = {output}");
    }
    output
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugInfo {
    modules: SmallMap<ModuleName, Module>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Module {
    bindings: Vec<Binding>,
    errors: Vec<Error>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Binding {
    key: String,
    location: String,
    binding: String,
    result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Error {
    location: String,
    message: String,
}

impl DebugInfo {
    pub fn new(modules: &[(&ModuleInfo, &Answers)], errors: &CollectedErrors) -> Self {
        fn f<K: Keyed>(
            _t: &AnswerEntry<K>,
            module_info: &ModuleInfo,
            answers: &Answers,
            res: &mut Vec<Binding>,
        ) where
            BindingTable: TableKeyed<K, Value = BindingEntry<K>>,
            AnswerTable: TableKeyed<K, Value = AnswerEntry<K>>,
            SolutionsTable: TableKeyed<K, Value = SolutionsEntry<K>>,
        {
            let bindings = answers.bindings();
            for idx in bindings.keys::<K>() {
                let key = bindings.idx_to_key(idx);
                res.push(Binding {
                    key: module_info.display(key).to_string(),
                    location: module_info
                        .display_range(K::range_with(idx, bindings))
                        .to_string(),
                    binding: bindings.get(idx).display_with(bindings).to_string(),
                    result: match answers.get_idx(idx) {
                        None => "None".to_owned(),
                        Some(v) => {
                            let mut r = (*v).clone();
                            r.visit_mut(&mut |t| {
                                answers.solver().expand_mut(t);
                            });
                            r.to_string()
                        }
                    },
                })
            }
        }

        let mut errors_by_module: HashMap<&ModulePath, Vec<Error>> = HashMap::new();
        for error in errors.ordinary.iter().chain(&errors.directives) {
            errors_by_module
                .entry(error.path())
                .or_default()
                .push(Error {
                    location: error.display_range().to_string(),
                    message: error.msg().to_owned(),
                });
        }

        Self {
            modules: modules
                .iter()
                .map(|(module_info, answers)| {
                    let mut res = Vec::new();
                    table_for_each!(answers.table(), |t| f(t, module_info, answers, &mut res));
                    (
                        module_info.name(),
                        Module {
                            bindings: res,
                            // A module without diagnostics has no entry.
                            errors: errors_by_module
                                .remove(module_info.path())
                                .unwrap_or_default(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::util::TestEnv;

    #[test]
    fn test_debug_info_includes_parse_errors() {
        let (state, handle) = TestEnv::one("main", "1 +\n").to_state();
        let output = debug_info(&state.transaction(), &[handle("main")], false);
        assert!(
            output.contains("Parse error"),
            "debug info should report the parse error:\n{output}"
        );
    }
}
