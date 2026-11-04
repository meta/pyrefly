/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Open documents shared by native and WebAssembly hosts.

use lsp_types::TextDocumentContentChangeEvent;
use ruff_source_file::LineIndex;
use ruff_source_file::OneIndexed;
use ruff_source_file::SourceLocation;

pub fn apply_change_events(original: &str, changes: Vec<TextDocumentContentChangeEvent>) -> String {
    /// Convert lsp_types::Position to usize index for a given text.
    fn position_to_usize(
        position: lsp_types::Position,
        index: &LineIndex,
        source_text: &str,
    ) -> usize {
        let source_location = SourceLocation {
            line: OneIndexed::from_zero_indexed(position.line as usize),
            character_offset: OneIndexed::from_zero_indexed(position.character as usize),
        };
        let text_size = index.offset(
            source_location,
            source_text,
            ruff_source_file::PositionEncoding::Utf16,
        );
        text_size.to_usize()
    }

    let mut result = original.to_owned();
    for change in changes {
        let (range, text) = match change {
            TextDocumentContentChangeEvent::TextDocumentContentChangePartial(partial) => {
                (Some(partial.range), partial.text)
            }
            TextDocumentContentChangeEvent::TextDocumentContentChangeWholeDocument(whole) => {
                (None, whole.text)
            }
        };
        // If no range is given, we can full text replace.
        match range {
            None => result = text,
            Some(range) => {
                let index = LineIndex::from_source_text(&result);
                let start = position_to_usize(range.start, &index, &result);
                let end = position_to_usize(range.end, &index, &result);
                result.replace_range(start..end, &text);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_document_changes_are_applied_in_order() {
        let changes = serde_json::from_value(serde_json::json!([
            { "text": "first\n" },
            { "text": "second\r\n" }
        ]))
        .unwrap();
        assert_eq!(apply_change_events("original\n", changes), "second\r\n");
        assert_eq!(
            apply_change_events("unchanged\n", Vec::new()),
            "unchanged\n"
        );
    }

    #[test]
    fn sequential_utf16_changes_preserve_crlf() {
        let changes = serde_json::from_value(serde_json::json!([
            {
                "range": { "start": { "line": 0, "character": 5 }, "end": { "line": 0, "character": 7 } },
                "text": "hello"
            },
            {
                "range": { "start": { "line": 1, "character": 4 }, "end": { "line": 1, "character": 5 } },
                "text": "2"
            },
            {
                "range": { "start": { "line": 0, "character": 5 }, "end": { "line": 0, "character": 10 } },
                "text": "ok"
            }
        ]))
        .unwrap();
        assert_eq!(
            apply_change_events("x = '\u{1f600}'\r\ny = 1\r\n", changes),
            "x = 'ok'\r\ny = 2\r\n"
        );
    }
}
