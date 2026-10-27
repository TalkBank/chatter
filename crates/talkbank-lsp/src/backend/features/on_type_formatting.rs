//! On-type formatting for CHAT files.
//!
//! Auto-inserts a tab character after typing `:` when the line starts with
//! `*SPEAKER:` or `%tier:`, matching CHAT format conventions.

use tower_lsp::lsp_types::*;

/// A tab insertion admitted against the current buffer and exact wire cursor.
/// This is a typing convention, not admission of a valid CHAT tier.
struct TierSeparatorInsertion(Position);

impl TierSeparatorInsertion {
    fn at(document: &str, position: Position, ch: &str) -> Option<Self> {
        if ch != ":" {
            return None;
        }
        let line = crate::backend::utils::protocol_lines(document)
            .map(|line| line.text())
            .nth(position.line as usize)?;
        if line.encode_utf16().count() != position.character as usize {
            return None;
        }
        let prefix = line.strip_suffix(':')?.strip_prefix(['*', '%'])?;
        // A colon in content, an empty name or whitespace is not the unfinished
        // prefix this convenience formats. An existing tab also prevents entry.
        if prefix.is_empty() || prefix.chars().any(|ch| ch.is_whitespace() || ch == ':') {
            return None;
        }
        Some(Self(position))
    }

    fn into_edit(self) -> TextEdit {
        TextEdit {
            range: Range {
                start: self.0,
                end: self.0,
            },
            new_text: "\t".to_owned(),
        }
    }
}

/// Insert a tab only immediately after the newly typed tier-prefix colon.
/// Positions inconsistent with that prefix, including out-of-range and
/// mid-character columns, do not authorize an edit.
pub fn on_type_formatting(document: &str, position: Position, ch: &str) -> Option<Vec<TextEdit>> {
    Some(vec![
        TierSeparatorInsertion::at(document, position, ch)?.into_edit(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_tab_after_main_tier() {
        let doc = "*CHI:";
        let pos = Position {
            line: 0,
            character: 5,
        };
        let edits = on_type_formatting(doc, pos, ":");
        assert!(edits.is_some());
        assert_eq!(edits.unwrap()[0].new_text, "\t");
    }

    #[test]
    fn test_insert_tab_after_dep_tier() {
        let doc = "%mor:";
        let pos = Position {
            line: 0,
            character: 5,
        };
        let edits = on_type_formatting(doc, pos, ":");
        assert!(edits.is_some());
    }

    #[test]
    fn test_no_insert_on_header() {
        let doc = "@Languages:";
        let pos = Position {
            line: 0,
            character: 11,
        };
        let edits = on_type_formatting(doc, pos, ":");
        assert!(edits.is_none());
    }

    #[test]
    fn test_no_insert_on_non_colon() {
        let doc = "*CHI:";
        let pos = Position {
            line: 0,
            character: 5,
        };
        let edits = on_type_formatting(doc, pos, "a");
        assert!(edits.is_none());
    }
}
