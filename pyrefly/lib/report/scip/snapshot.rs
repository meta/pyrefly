/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A port of scip-python's `formatSnapshot` (`packages/pyright-scip/src/lib.ts`), so that
//! snapshots ported from scip-python keep the same layout.

use std::collections::HashMap;
use std::collections::HashSet;

use scip::types::Document;
use scip::types::Relationship;
use scip::types::SymbolInformation;
use scip::types::SymbolRole;

use crate::report::scip::MANAGER;
use crate::report::scip::SCHEME;

const FORMAT_OPTIONS_PREFIX: &str = "# format-options:";

#[derive(Default)]
struct FormatOptions {
    show_docs: bool,
    show_ranges: bool,
}

impl FormatOptions {
    fn parse(lines: &[&str]) -> Self {
        let mut options = Self::default();
        if let Some(line) = lines
            .iter()
            .find_map(|line| line.strip_prefix(FORMAT_OPTIONS_PREFIX))
        {
            for option in line.trim().split(',') {
                match option.trim() {
                    "showDocs" => options.show_docs = true,
                    "showRanges" => options.show_ranges = true,
                    option => panic!("Invalid format option: {option}"),
                }
            }
        }
        options
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Position {
    line: usize,
    character: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Range {
    start: Position,
    end: Position,
}

impl Range {
    fn from_scip(range: &[i32]) -> Self {
        let (end_line, end_character) = if range.len() == 3 {
            (range[0], range[2])
        } else {
            (range[2], range[3])
        };
        Self {
            start: Position {
                line: range[0] as usize,
                character: range[1] as usize,
            },
            end: Position {
                line: end_line as usize,
                character: end_character as usize,
            },
        }
    }
}

fn is_definition(roles: i32) -> bool {
    roles & SymbolRole::Definition as i32 != 0
}

struct EnclosingRange<'a> {
    range: Range,
    symbol: &'a str,
}

struct Formatter<'a> {
    out: String,
    options: FormatOptions,
    symbols: HashMap<&'a str, &'a SymbolInformation>,
    external_symbols: HashMap<&'a str, &'a SymbolInformation>,
    symbols_with_definitions: HashSet<&'a str>,
    emitted_docs: HashSet<&'a str>,
}

impl<'a> Formatter<'a> {
    fn push_doc(&mut self, range: Range, symbol: &'a str, definition: bool, start_of_line: bool) {
        if self.emitted_docs.contains(symbol)
            || (!definition && self.symbols_with_definitions.contains(symbol))
        {
            self.out.push('\n');
            return;
        }
        self.emitted_docs.insert(symbol);

        let mut prefix = "\n#".to_owned();
        if !start_of_line {
            prefix.push_str(&" ".repeat(range.start.character.saturating_sub(1)));
        }
        let (info, external) = match self.external_symbols.get(symbol) {
            Some(info) => (Some(*info), true),
            None => (self.symbols.get(symbol).copied(), false),
        };
        if let Some(info) = info {
            self.push_docs(&prefix, &info.documentation, external);
            self.push_relationships(&prefix, &info.relationships);
        }
        self.out.push('\n');
    }

    fn push_docs(&mut self, prefix: &str, docs: &[String], external: bool) {
        if !self.options.show_docs {
            return;
        }
        for doc in docs {
            for (i, line) in doc.split('\n').enumerate() {
                self.out.push_str(prefix);
                if i == 0 {
                    if external {
                        self.out.push_str("external ");
                    }
                    self.out.push_str("documentation ");
                } else {
                    self.out.push_str("            > ");
                }
                // scip-python truncates by UTF-16 code unit; chars are close enough here.
                let truncated: String = line.chars().take(40).collect();
                self.out.push_str(&truncated);
                if line.chars().count() > 40 {
                    self.out.push_str("...");
                }
            }
        }
    }

    fn push_relationships(&mut self, prefix: &str, relationships: &[Relationship]) {
        let mut relationships: Vec<_> = relationships.iter().collect();
        relationships.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        for relationship in relationships {
            self.out.push_str(prefix);
            self.out.push_str("relationship");
            if relationship.is_implementation {
                self.out.push_str(" implementation");
            }
            if relationship.is_reference {
                self.out.push_str(" reference");
            }
            if relationship.is_type_definition {
                self.out.push_str(" type_definition");
            }
            self.out.push(' ');
            self.out.push_str(&relationship.symbol);
        }
    }

    fn push_enclosing_range(&mut self, enclosing: &EnclosingRange, end: bool) {
        let character = enclosing.range.start.character;
        self.out.push('#');
        self.out
            .push_str(&" ".repeat(character.saturating_sub(1).max(1)));
        self.out.push(if character < 2 {
            '<'
        } else if end {
            '^'
        } else {
            '⌄'
        });
        self.out.push_str(if end { " end " } else { " start " });
        self.out.push_str("enclosing_range ");
        self.out.push_str(enclosing.symbol);
        self.out.push('\n');
    }
}

pub fn format_snapshot(
    input: &str,
    doc: &Document,
    external_symbols: &[SymbolInformation],
) -> String {
    let lines: Vec<&str> = input.split('\n').collect();
    let mut formatter = Formatter {
        out: String::new(),
        options: FormatOptions::parse(&lines),
        symbols: doc.symbols.iter().map(|s| (s.symbol.as_str(), s)).collect(),
        external_symbols: external_symbols
            .iter()
            .map(|s| (s.symbol.as_str(), s))
            .collect(),
        symbols_with_definitions: HashSet::new(),
        emitted_docs: HashSet::new(),
    };

    let mut enclosing_ranges = Vec::new();
    for occurrence in &doc.occurrences {
        if is_definition(occurrence.symbol_roles) {
            formatter
                .symbols_with_definitions
                .insert(occurrence.symbol.as_str());
        }
        if formatter.options.show_ranges && !occurrence.enclosing_range.is_empty() {
            enclosing_ranges.push(EnclosingRange {
                range: Range::from_scip(&occurrence.enclosing_range),
                symbol: &occurrence.symbol,
            });
        }
    }
    // The range that starts first, and for equal starts, the one that ends last.
    enclosing_ranges.sort_by(|a, b| {
        a.range
            .cmp(&b.range)
            .then(b.range.end.line.cmp(&a.range.end.line))
    });
    let mut starts: Vec<Vec<&EnclosingRange>> = (0..lines.len()).map(|_| Vec::new()).collect();
    let mut ends: Vec<Vec<&EnclosingRange>> = (0..lines.len()).map(|_| Vec::new()).collect();
    for enclosing in &enclosing_ranges {
        starts[enclosing.range.start.line].push(enclosing);
        ends[enclosing.range.end.line].insert(0, enclosing);
    }

    let mut occurrences: Vec<_> = doc.occurrences.iter().collect();
    occurrences.sort_by_key(|o| Range::from_scip(&o.range));
    let package_prefix = format!("{SCHEME} {MANAGER}");
    let mut index = 0;

    for (line_number, line) in lines.iter().enumerate() {
        // A zero-width occurrence at the start of the file is a file-level item, written above
        // the first line.
        if index == 0
            && let Some(occurrence) = occurrences.first()
        {
            let range = Range::from_scip(&occurrence.range);
            if range.start.character == 0 && range.end.character == 0 {
                let definition = is_definition(occurrence.symbol_roles);
                formatter.out.push_str("# < ");
                formatter.out.push_str(if definition {
                    "definition"
                } else {
                    "reference"
                });
                formatter.out.push(' ');
                formatter.out.push_str(&occurrence.symbol);
                formatter.push_doc(range, &occurrence.symbol, definition, true);
                formatter.out.push('\n');
                index += 1;
            }
        }

        if formatter.options.show_ranges {
            for enclosing in &starts[line_number] {
                formatter.push_enclosing_range(enclosing, false);
            }
        }

        formatter.out.push_str(line);
        formatter.out.push('\n');
        while let Some(occurrence) = occurrences.get(index)
            && occurrence.range[0] as usize == line_number
        {
            index += 1;
            assert!(
                occurrence.range.len() == 3,
                "multi-line occurrence ranges are not supported"
            );
            let range = Range::from_scip(&occurrence.range);
            let start_of_line = range.start.character == 0;
            formatter.out.push('#');
            if !start_of_line {
                formatter
                    .out
                    .push_str(&" ".repeat(range.start.character - 1));
            }
            let caret_length = (range.end.character - range.start.character)
                .checked_sub(start_of_line as usize)
                .expect("negative length occurrence");
            formatter.out.push_str(&"^".repeat(caret_length));
            let definition = is_definition(occurrence.symbol_roles);
            formatter.out.push_str(if definition {
                " definition "
            } else {
                " reference "
            });
            let symbol = occurrence
                .symbol
                .strip_prefix(&package_prefix)
                .unwrap_or(&occurrence.symbol);
            formatter.out.push_str(&symbol.replacen('\n', "|", 1));
            formatter.push_doc(range, &occurrence.symbol, definition, start_of_line);
        }

        if formatter.options.show_ranges {
            for enclosing in &ends[line_number] {
                formatter.push_enclosing_range(enclosing, true);
            }
        }
    }
    formatter.out
}

#[cfg(test)]
mod tests {
    use scip::types::Occurrence;

    use super::*;

    fn occurrence(range: Vec<i32>, symbol: &str, roles: i32, enclosing: Vec<i32>) -> Occurrence {
        Occurrence {
            range,
            symbol: symbol.to_owned(),
            symbol_roles: roles,
            enclosing_range: enclosing,
            ..Default::default()
        }
    }

    #[test]
    fn test_docs_relationships_and_ranges() {
        let input = "# format-options: showDocs, showRanges\nclass C:\n    def f(self):\n        return self\n";
        let def = SymbolRole::Definition as i32;
        let module = "scip-python python p 1 m/__init__:";
        let f = "scip-python python p 1 m/C#f().";
        let param = "scip-python python p 1 m/C#f().(self)";
        let doc = Document {
            occurrences: vec![
                occurrence(vec![0, 0, 0], module, def, vec![]),
                occurrence(vec![2, 8, 9], f, def, vec![2, 4, 3, 19]),
                occurrence(vec![2, 10, 14], param, def, vec![]),
                occurrence(vec![3, 15, 19], param, 0, vec![]),
            ],
            symbols: vec![SymbolInformation {
                symbol: f.to_owned(),
                documentation: vec!["```python\ndef f(self)\n```".to_owned()],
                relationships: vec![Relationship {
                    symbol: "scip-python python p 1 m/B#f().".to_owned(),
                    is_implementation: true,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let expected = r#"# < definition scip-python python p 1 m/__init__:

# format-options: showDocs, showRanges
class C:
#   ⌄ start enclosing_range scip-python python p 1 m/C#f().
    def f(self):
#       ^ definition  p 1 m/C#f().
#       documentation ```python
#                   > def f(self)
#                   > ```
#       relationship implementation scip-python python p 1 m/B#f().
#         ^^^^ definition  p 1 m/C#f().(self)
        return self
#              ^^^^ reference  p 1 m/C#f().(self)
#   ^ end enclosing_range scip-python python p 1 m/C#f().

"#;
        pretty_assertions::assert_eq!(expected, format_snapshot(input, &doc, &[]));
    }
}
