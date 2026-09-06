//! Text diff and byte↔point conversion for incremental parsing.
//!
//! Computes the minimal changed byte range between old and new document text,
//! then converts it to tree-sitter `Range` / `Point` values so the incremental
//! CST edit and re-parse can focus on just the touched region.
/// Describe the single replacement taking `old_text` to `new_text`.
///
/// The common prefix and suffix end at UTF-8 character boundaries. Tree-sitter
/// points use byte columns, independently of the LSP's UTF-16 wire positions.
/// No edit is needed when the source bytes are identical.
pub(crate) fn compute_input_edit(old_text: &str, new_text: &str) -> Option<tree_sitter::InputEdit> {
    if old_text == new_text {
        return None;
    }
    let old_bytes = old_text.as_bytes();
    let new_bytes = new_text.as_bytes();
    let mut start = old_bytes
        .iter()
        .zip(new_bytes)
        .take_while(|(old, new)| old == new)
        .count();
    while !old_text.is_char_boundary(start) || !new_text.is_char_boundary(start) {
        start -= 1;
    }
    let mut old_end = old_bytes.len();
    let mut new_end = new_bytes.len();
    while old_end > start && new_end > start && old_bytes[old_end - 1] == new_bytes[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }
    while !old_text.is_char_boundary(old_end) || !new_text.is_char_boundary(new_end) {
        old_end += 1;
        new_end += 1;
    }
    Some(tree_sitter::InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: byte_to_point(old_text, start),
        old_end_position: byte_to_point(old_text, old_end),
        new_end_position: byte_to_point(new_text, new_end),
    })
}

/// Convert a source byte offset to tree-sitter's byte-based row and column.
fn byte_to_point(text: &str, byte: usize) -> tree_sitter::Point {
    let mut point = tree_sitter::Point::new(0, 0);
    for byte in text.bytes().take(byte) {
        if byte == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cross-library contract: tree-sitter uses UTF-8 byte columns, while LSP
    // positions use UTF-16. Both the changed span and its points must agree.
    #[test]
    fn unicode_edit_range_matches_tree_sitter_coordinates() {
        let range = compute_input_edit("😀éx", "😀êx").expect("changed range");
        assert_eq!(range.start_byte, 4);
        assert_eq!(range.new_end_byte, 6);
        assert_eq!(range.start_position, tree_sitter::Point::new(0, 4));
        assert_eq!(range.new_end_position, tree_sitter::Point::new(0, 6));
    }

    // Named examples specify replacement policy; replay checks that each edit
    // reconstructs the new source, including shared UTF-8 prefix/suffix bytes.
    #[test]
    fn replacement_cases_reconstruct_source() {
        for (name, old, new, expected) in [
            ("unchanged", "hello", "hello", None),
            ("insert", "abc", "abXc", Some((2, 2, 3))),
            ("delete", "abXc", "abc", Some((2, 3, 2))),
            ("replace", "hello world", "hello rust", Some((6, 11, 10))),
            ("shared UTF-8 prefix", "aéx", "aêx", Some((1, 3, 3))),
            ("shared UTF-8 suffix", "aéx", "aĩx", Some((1, 3, 3))),
            ("empty old", "", "😀", Some((0, 0, 4))),
            ("empty new", "😀", "", Some((0, 4, 0))),
        ] {
            let edit = compute_input_edit(old, new);
            assert_eq!(
                edit.as_ref()
                    .map(|e| (e.start_byte, e.old_end_byte, e.new_end_byte)),
                expected,
                "{name}"
            );
            if let Some(edit) = edit {
                let mut replay = old.to_owned();
                replay.replace_range(
                    edit.start_byte..edit.old_end_byte,
                    &new[edit.start_byte..edit.new_end_byte],
                );
                assert_eq!(replay, new, "{name}");
            }
        }
    }

    #[test]
    fn changed_range_uses_new_text_coordinates() {
        let range = compute_input_edit("abc", "abXc").expect("changed range");
        assert_eq!(range.start_byte, 2);
        assert_eq!(range.new_end_byte, 3);
        assert_eq!(range.start_position.row, 0);
        assert_eq!(range.start_position.column, 2);
        assert_eq!(range.new_end_position.row, 0);
        assert_eq!(range.new_end_position.column, 3);
    }

    #[test]
    fn byte_to_point_handles_multibyte_characters() {
        let text = "a\n你b";
        let point = byte_to_point(text, 5);
        assert_eq!(point.row, 1);
        assert_eq!(point.column, 3);
    }
}
