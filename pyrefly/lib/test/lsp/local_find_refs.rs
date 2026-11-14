/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fs;
use std::slice;

use itertools::Itertools;
use pretty_assertions::assert_eq;
use pyrefly_build::handle::Handle;
use pyrefly_python::module::TextRangeWithModule;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use tempfile::TempDir;

use crate::state::lsp::FindPreference;
use crate::state::lsp::ReferenceOptions;
use crate::state::require::Require;
use crate::state::state::State;
use crate::test::util::TestEnv;
use crate::test::util::code_frame_of_source_at_range;
use crate::test::util::get_batched_lsp_operations_report;

fn get_test_report(
    state: &State,
    handle: &Handle,
    position: TextSize,
    include_declaration: bool,
) -> String {
    let transaction = state.transaction();
    let ranges = transaction.find_local_references(
        handle,
        position,
        ReferenceOptions::all(include_declaration),
    );
    let module_info = transaction.get_module_info(handle).unwrap();
    format!(
        "References:\n{}",
        ranges
            .into_iter()
            .map(|range| code_frame_of_source_at_range(module_info.contents(), range))
            .join("\n")
    )
}

#[test]
fn no_references_test() {
    let code = r#"
1234567
# ^

"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | 1234567
      ^
References:
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn basic_variables_test() {
    let code = r#"
foo = 3
# ^
foo + 4 + foo
#          ^

def ff():
  foo = 4
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | foo = 3
      ^
References:
2 | foo = 3
    ^^^
4 | foo + 4 + foo
    ^^^
4 | foo + 4 + foo
              ^^^

4 | foo + 4 + foo
               ^
References:
2 | foo = 3
    ^^^
4 | foo + 4 + foo
    ^^^
4 | foo + 4 + foo
              ^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn dunder_all_entries_are_symbol_references() {
    let code = r#"
foo = 1
# ^
__all__ = ["foo"]
#            ^
__all__.remove("foo")
#                ^
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | foo = 1
      ^
References:
2 | foo = 1
    ^^^
4 | __all__ = ["foo"]
                ^^^
6 | __all__.remove("foo")
                    ^^^

4 | __all__ = ["foo"]
                 ^
References:
2 | foo = 1
    ^^^
4 | __all__ = ["foo"]
                ^^^
6 | __all__.remove("foo")
                    ^^^

6 | __all__.remove("foo")
                     ^
References:
2 | foo = 1
    ^^^
4 | __all__ = ["foo"]
                ^^^
6 | __all__.remove("foo")
                    ^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn dunder_all_references_use_string_contents() {
    let code = r#"
foo = 1
# ^
__all__ = ["foo", r"foo", """foo"""]
__all__.append("foo")
__all__.extend(["foo"])
__all__.remove("foo")
other = "foo"
def f():
    foo = 2
    return foo
"#;
    get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        let transaction = state.transaction();
        let module = transaction.get_module_info(handle).unwrap();
        for include_declaration in [false, true] {
            let references = transaction.find_local_references(
                handle,
                position,
                ReferenceOptions::all(include_declaration),
            );
            let expected_lines = if include_declaration {
                vec![1, 3, 3, 3, 4, 5, 6]
            } else {
                vec![3, 3, 3, 4, 5, 6]
            };
            assert_eq!(
                references
                    .iter()
                    .map(|range| module.to_lsp_range(*range).start.line)
                    .collect::<Vec<_>>(),
                expected_lines,
            );
            for range in references {
                assert_eq!(module.code_at(range), "foo");
            }
        }
        String::new()
    });
}

#[test]
fn dunder_all_references_respect_import_aliases() {
    let code = r#"
from lib import foo as alias
#                      ^
from other import foo
#                 ^
__all__ = ["alias", "foo"]
alias
foo
text = "foo"
"#;
    get_batched_lsp_operations_report(
        &[("main", code), ("lib", "foo = 1\n"), ("other", "foo = 2\n")],
        |state, handle, position| {
            let transaction = state.transaction();
            let module = transaction.get_module_info(handle).unwrap();
            let references =
                transaction.find_local_references(handle, position, ReferenceOptions::all(true));
            let lines = references
                .iter()
                .map(|range| module.to_lsp_range(*range).start.line)
                .collect::<Vec<_>>();
            let expected_name = if module.to_lsp_position(position).line == 1 {
                assert_eq!(lines, vec![1, 5, 6]);
                "alias"
            } else {
                assert_eq!(lines, vec![3, 5, 7]);
                "foo"
            };
            for range in references {
                assert_eq!(module.code_at(range), expected_name);
            }
            String::new()
        },
    );
}

#[test]
fn dunder_slots_entries_are_attribute_references() {
    let code = r#"
class C:
    __slots__ = ("foo",)
#                  ^

    def __init__(self) -> None:
        self.foo = 1
#            ^

    def get(self) -> int:
        return self.foo
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
3 |     __slots__ = ("foo",)
                       ^
References:
3 |     __slots__ = ("foo",)
                      ^^^
7 |         self.foo = 1
                 ^^^
11 |         return self.foo
                         ^^^

7 |         self.foo = 1
                 ^
References:
3 |     __slots__ = ("foo",)
                      ^^^
7 |         self.foo = 1
                 ^^^
11 |         return self.foo
                         ^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn dunder_slots_references_use_string_contents() {
    for name in ["foo", "café", "变量"] {
        for assignment in [
            "__slots__ = \"foo\"",
            "__slots__ = (\"foo\",)",
            "__slots__ = [r'foo']",
            "__slots__ = {\"foo\": \"Field documentation.\"}",
            "__slots__: tuple[str, ...] = (\"\"\"foo\"\"\",)",
        ] {
            let assignment = assignment.replace("foo", name);
            let code = format!(
                "class C:\n    {assignment}\n    {name}: int\n    def read(self) -> int:\n        return self.{name}\n    text = \"{name}\"\n"
            );
            let (state, handle) = TestEnv::one("main", &code).to_state();
            let handle = handle("main");
            let transaction = state.transaction();
            let module = transaction.get_module_info(&handle).unwrap();
            for position in [
                code.find(name).unwrap(),
                code.find(&format!("self.{name}")).unwrap() + "self.".len(),
            ] {
                for include_declaration in [false, true] {
                    let references = transaction.find_local_references(
                        &handle,
                        TextSize::try_from(position).unwrap(),
                        ReferenceOptions::all(include_declaration),
                    );
                    let expected_lines = if include_declaration {
                        vec![1, 2, 4]
                    } else {
                        vec![1, 4]
                    };
                    assert_eq!(
                        references
                            .iter()
                            .map(|range| module.to_lsp_range(*range).start.line)
                            .collect::<Vec<_>>(),
                        expected_lines,
                        "code: {code}",
                    );
                    for range in references {
                        assert_eq!(module.code_at(range), name);
                    }
                }
            }
        }
    }
}

#[test]
fn dunder_slots_references_respect_class_identity() {
    let code = r#"
class C:
    __slots__ = {"foo": "foo"}
    text = "foo"
    def __init__(self) -> None:
        __slots__ = ("foo",)
        self.foo = 1
    def read(self) -> int:
        return self.foo

    class Inner:
        __slots__ = ("foo",)
        foo: str

class Other:
    __slots__ = ("foo",)
    foo: str
"#;
    let (state, handle) = TestEnv::one("main", code).to_state();
    let handle = handle("main");
    let expected = [
        code.find("\"foo\"").unwrap() + 1,
        code.find("self.foo").unwrap() + "self.".len(),
        code.rfind("self.foo").unwrap() + "self.".len(),
    ]
    .into_iter()
    .map(|start| TextRange::at(TextSize::try_from(start).unwrap(), TextSize::of("foo")))
    .collect::<Vec<_>>();
    for range in &expected {
        assert_eq!(
            state.transaction().find_local_references(
                &handle,
                range.start(),
                ReferenceOptions::all(true),
            ),
            expected,
        );
    }
}

#[test]
fn dunder_slots_global_references_in_indexed_files() {
    let root = TempDir::new().unwrap();
    let mut env = TestEnv::new()
        .with_default_require_level(Require::Indexing)
        .with_run_require(Require::Indexing);
    let base = "class Base:\n    foo: int\n";
    for (name, code) in [
        ("base", base),
        (
            "child",
            "from base import Base\nclass Child(Base):\n    __slots__ = (\"foo\",)\n",
        ),
        (
            "unrelated",
            "from base import Base\nclass Other:\n    __slots__ = (\"foo\",)\n    foo: str\n",
        ),
        ("plain", "from base import Base\nclass Plain(Base): pass\n"),
    ] {
        let path = root.path().join(format!("{name}.py"));
        fs::write(&path, code).unwrap();
        env.add_real_path(name, path);
    }
    let (state, handle) = env.to_state();
    let base_handle = handle("base");
    let child_handle = handle("child");
    let mut transaction = state.transaction();
    assert!(transaction.get_ast(&child_handle).is_none());
    assert!(transaction.get_answers(&child_handle).is_none());
    transaction.run(slice::from_ref(&base_handle), Require::Everything, None);
    let definitions = transaction
        .find_definition(
            &base_handle,
            TextSize::try_from(base.find("foo").unwrap()).unwrap(),
            FindPreference::default(),
        )
        .unwrap();
    let definition = definitions.first();
    let mut references = transaction
        .find_global_references_from_definition(
            *base_handle.sys_info(),
            definition.metadata.clone(),
            TextRangeWithModule::new(definition.module.clone(), definition.definition_range),
            ReferenceOptions::all(true),
        )
        .unwrap()
        .into_iter()
        .map(|(module, ranges)| {
            for range in &ranges {
                assert_eq!(module.code_at(*range), "foo");
            }
            (module.name().to_string(), ranges.len())
        })
        .collect::<Vec<_>>();
    references.sort();
    assert_eq!(
        references,
        vec![("base".to_owned(), 1), ("child".to_owned(), 1)]
    );
    for name in ["child", "unrelated", "plain"] {
        assert!(transaction.get_ast(&handle(name)).is_none());
        assert!(transaction.get_answers(&handle(name)).is_none());
    }
}

#[test]
fn references_to_aliased_submodule_stop_at_import() {
    let main = r#"
from xxx import yyy as zzz
#                      ^

zzz.f()
"#;
    let report = get_batched_lsp_operations_report(
        &[("main", main), ("xxx", ""), ("xxx.yyy", "def f(): ...\n")],
        |state, handle, position| get_test_report(state, handle, position, true),
    );
    assert_eq!(
        r#"
# main.py
2 | from xxx import yyy as zzz
                           ^
References:
2 | from xxx import yyy as zzz
                           ^^^
5 | zzz.f()
    ^^^


# xxx.py

# xxx.yyy.py
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn references_to_aliased_module_stop_at_import() {
    let main = r#"
import xxx.yyy as zzz
#                 ^

zzz.f()
"#;
    let report = get_batched_lsp_operations_report(
        &[("main", main), ("xxx", ""), ("xxx.yyy", "def f(): ...\n")],
        |state, handle, position| get_test_report(state, handle, position, true),
    );
    assert_eq!(
        r#"
# main.py
2 | import xxx.yyy as zzz
                      ^
References:
2 | import xxx.yyy as zzz
                      ^^^
5 | zzz.f()
    ^^^


# xxx.py

# xxx.yyy.py
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn references_to_module_alias_after_submodule_imports() {
    // Each `import xxx.yyy` rebinds `zzz`, so the use at the end is a hundred rebindings
    // away from the alias.
    let main = format!(
        "\nimport xxx as zzz\n#             ^\n{}zzz.yyy.f()\n# ^\n",
        "import xxx.yyy\n".repeat(100)
    );
    let report = get_batched_lsp_operations_report(
        &[("main", &main), ("xxx", ""), ("xxx.yyy", "def f(): ...\n")],
        |state, handle, position| get_test_report(state, handle, position, true),
    );
    assert_eq!(
        r#"
# main.py
2 | import xxx as zzz
                  ^
References:
2 | import xxx as zzz
                  ^^^
104 | zzz.yyy.f()
      ^^^

104 | zzz.yyy.f()
        ^
References:
2 | import xxx as zzz
                  ^^^
104 | zzz.yyy.f()
      ^^^


# xxx.py

# xxx.yyy.py
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn references_to_module_alias_in_function() {
    let main = r#"
import xxx as zzz
#             ^
import xxx.yyy

def g():
    zzz.yyy.f()
#   ^
"#;
    let report = get_batched_lsp_operations_report(
        &[("main", main), ("xxx", ""), ("xxx.yyy", "def f(): ...\n")],
        |state, handle, position| get_test_report(state, handle, position, true),
    );
    assert_eq!(
        r#"
# main.py
2 | import xxx as zzz
                  ^
References:
2 | import xxx as zzz
                  ^^^
7 |     zzz.yyy.f()
        ^^^

7 |     zzz.yyy.f()
        ^
References:
2 | import xxx as zzz
                  ^^^
7 |     zzz.yyy.f()
        ^^^


# xxx.py

# xxx.yyy.py
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn pytest_fixture_references_include_injected_parameters() {
    let code = r#"
import pytest  # type: ignore

@pytest.fixture
def data():
#   ^
    return 1

def test_data(data):
    assert data == 1
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
5 | def data():
        ^
References:
5 | def data():
        ^^^^
9 | def test_data(data):
                  ^^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn basic_attribute_test() {
    let code = r#"
class MyClass:
  x = 5
MyClass().x
#         ^
c = MyClass()
c.x
# ^

class AnotherClass:
  x = 5
AnotherClass().x
#              ^

class ExtendsBase(MyClass):
  pass

ExtendsBase().x
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
4 | MyClass().x
              ^
References:
3 |   x = 5
      ^
4 | MyClass().x
              ^
7 | c.x
      ^
18 | ExtendsBase().x
                   ^

7 | c.x
      ^
References:
3 |   x = 5
      ^
4 | MyClass().x
              ^
7 | c.x
      ^
18 | ExtendsBase().x
                   ^

12 | AnotherClass().x
                    ^
References:
11 |   x = 5
       ^
12 | AnotherClass().x
                    ^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn attribute_reference_triggered_from_attribute_definition_test() {
    let code = r#"
class MyClass:
    attribute = 5
#    ^

    def method(self) -> None:
        self.attribute


obj = MyClass()
print(obj.attribute)
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
3 |     attribute = 5
         ^
References:
3 |     attribute = 5
        ^^^^^^^^^
7 |         self.attribute
                 ^^^^^^^^^
11 | print(obj.attribute)
               ^^^^^^^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn synthetic_reference_regression_test() {
    let code = r#"
foo = 3
# ^
try:
  pass
except Exception:
  foo
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | foo = 3
      ^
References:
2 | foo = 3
    ^^^
7 |   foo
      ^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn dunder_init() {
    let code = r#"
class Foo:
    def __init__(self): ...
    #   ^
    
Foo()
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
3 |     def __init__(self): ...
            ^
References:
3 |     def __init__(self): ...
            ^^^^^^^^
6 | Foo()
    ^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn narrowing() {
    let code = r#"
xyz = "test"
# ^
if isinstance(xyz, int):
    print(xyz)
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | xyz = "test"
      ^
References:
2 | xyz = "test"
    ^^^
4 | if isinstance(xyz, int):
                  ^^^
5 |     print(xyz)
              ^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn method() {
    let code = r#"
class C:
    def f(self) -> str:
    #   ^
        ...

    def g(self) -> str:
        return self.f()
        #           ^
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
3 |     def f(self) -> str:
            ^
References:
3 |     def f(self) -> str:
            ^
8 |         return self.f()
                        ^

8 |         return self.f()
                        ^
References:
3 |     def f(self) -> str:
            ^
8 |         return self.f()
                        ^
"#
        .trim(),
        report.trim(),
    );
}

// todo(kylei): reference on parent method should find child method
#[test]
fn child_method() {
    let code = r#"
class C:
    def f(self) -> str: ...
    #   ^

class Concrete(C):
    def f(self) -> str:
        return "test"
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
3 |     def f(self) -> str: ...
            ^
References:
3 |     def f(self) -> str: ...
            ^
"#
        .trim(),
        report.trim(),
    );
}

// todo(kylei): reference on child method should find parent method
#[test]
fn parent_method() {
    let code = r#"
class C:
    def f(self) -> str: ...
    
class Concrete(C):
    def f(self) -> str:
    #   ^
        return "test"
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
6 |     def f(self) -> str:
            ^
References:
6 |     def f(self) -> str:
            ^
"#
        .trim(),
        report.trim(),
    );
}

// todo(kylei): reference on child method should find parent + other child methods
#[test]
fn sibling_method() {
    let code = r#"
class C:
    def f(self) -> str: ...
    
class Concrete(C):
    def f(self) -> str:
    #   ^
        return "test"

class Concrete2(C):
    def f(self) -> str:
        return "test"

"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
6 |     def f(self) -> str:
            ^
References:
6 |     def f(self) -> str:
            ^
"#
        .trim(),
        report.trim(),
    );
}

// todo(kylei): references on renames should find lhs of assignment
#[test]
fn reassigned_local() {
    let code = r#"
xy = 5
#^
xy = xy + 1
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | xy = 5
     ^
References:
2 | xy = 5
    ^^
4 | xy = xy + 1
         ^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn include_declaration() {
    let code = r#"
xy = 1
#^
xy + xy
"#;
    let with_declaration =
        get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
            get_test_report(state, handle, position, true)
        });
    assert_eq!(
        r#"
# main.py
2 | xy = 1
     ^
References:
2 | xy = 1
    ^^
4 | xy + xy
    ^^
4 | xy + xy
         ^^
"#
        .trim(),
        with_declaration.trim(),
    );

    let without_declaration =
        get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
            get_test_report(state, handle, position, false)
        });
    assert_eq!(
        r#"
# main.py
2 | xy = 1
     ^
References:
4 | xy + xy
    ^^
4 | xy + xy
         ^^
"#
        .trim(),
        without_declaration.trim(),
    );
}

#[test]
fn references_inside_quoted_annotations() {
    let code = r#"
from typing import Literal

class Model: ...
#       ^
a: "Model"
#   ^
b: "dict[str, list[Model]]"
c: list["Model"]
d: Literal["Model"]
e = "Model"
f: "\x6cist[Model]"
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
4 | class Model: ...
            ^
References:
4 | class Model: ...
          ^^^^^
6 | a: "Model"
        ^^^^^
8 | b: "dict[str, list[Model]]"
                       ^^^^^
9 | c: list["Model"]
             ^^^^^

6 | a: "Model"
        ^
References:
4 | class Model: ...
          ^^^^^
6 | a: "Model"
        ^^^^^
8 | b: "dict[str, list[Model]]"
                       ^^^^^
9 | c: list["Model"]
             ^^^^^
"#
        .trim(),
        report.trim(),
    );
}

#[test]
fn references_to_normalized_identifier() {
    // Python applies NFKC normalization to identifiers, so `𝒞` and `C` are the same name.
    let code = r#"
CCC = 1
# ^
print(𝒞𝒞𝒞)
"#;
    let report = get_batched_lsp_operations_report(&[("main", code)], |state, handle, position| {
        get_test_report(state, handle, position, true)
    });
    assert_eq!(
        r#"
# main.py
2 | CCC = 1
      ^
References:
2 | CCC = 1
    ^^^
4 | print(𝒞𝒞𝒞)
          ^^^
"#
        .trim(),
        report.trim(),
    );
}
