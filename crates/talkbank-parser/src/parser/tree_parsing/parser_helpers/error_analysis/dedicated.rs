//! Shared classification rules for the dedicated malformation codes
//! E759 / E760 (adopted 2026-07-23 from the CHECK-parity adjudication of
//! CLAN CHECK errors 52 and 11).
//!
//! Unparsable regions are reported from several independent sites (the
//! file-level error analysis, the main-tier contents loop, the
//! dependent-tier analyzer, the whole-tree recovery backstop). Each site
//! calls these PURE text rules so the classification cannot drift
//! between them; the caller supplies whatever position/typing gate its
//! context affords (e.g. "this fragment is the FIRST content item").

/// CHECK-52 family: a bracket code whose first inner character is one of
/// `/`, `<`, `>`, `:`, `"` (retraces, overlap markers, replacements, the
/// quotation marker). Returns the code token for the message (through
/// the first `]` when present). The caller is responsible for asserting
/// the LEADING position; this rule only recognizes the shape.
pub(crate) fn leading_postfix_annotation(content: &str) -> Option<&str> {
    let rest = content.strip_prefix('[')?;
    if rest.starts_with(['/', '<', '>', ':', '"']) {
        Some(match content.find(']') {
            Some(close) => &content[..=close],
            None => "[",
        })
    } else {
        None
    }
}

/// E760 shape: a whitespace-delimited %mor item beginning with the `|`
/// separator (`|we`): its part-of-speech field is empty. Returns the
/// offending item. The caller supplies the mor-tier gate.
///
/// The FIRST token is only an empty-POS item when the analyzed text
/// itself starts at an item boundary: tree-sitter splits malformations
/// like `n|dog|cat` (CHECK 79, two pipes) or malformed compounds
/// (CHECK 87) into a parsed head (`n|dog`) plus an ERROR fragment
/// (`|cat`) whose leading pipe is a SPLIT TAIL, not an empty POS field.
/// Callers with only a fragment must pass `starts_at_item_boundary`
/// accordingly (see [`at_item_boundary`]); tokens after the first are
/// whitespace-delimited by construction and always eligible.
pub(crate) fn mor_item_with_empty_pos(text: &str, starts_at_item_boundary: bool) -> Option<&str> {
    text.split_whitespace()
        .enumerate()
        .find(|(index, token)| {
            (starts_at_item_boundary || *index > 0) && token.starts_with('|') && token.len() > 1
        })
        .map(|(_, token)| token)
}

/// Whether byte offset `start` sits at an ITEM boundary on its line:
/// preceded by whitespace (space or tab) or at the very start of the
/// line. A fragment whose preceding byte is any other character is the
/// tail of a split item, never a free-standing item.
pub(crate) fn at_item_boundary(source: &str, start: usize) -> bool {
    let Some(prefix) = source.get(..start) else {
        return false;
    };
    matches!(prefix.chars().next_back(), None | Some(' ' | '\t' | '\n'))
}

/// Whether byte offset `start` sits at the START of a main tier's
/// content: its line begins with `*`, has the `:<tab>` separator, and
/// everything between that tab and `start` is (at most) spaces. Used to
/// distinguish a LEADING annotation fragment (E759) from one glued after
/// a word (E757/E375 territory) when the analyzer has only the fragment
/// node and no traversal context.
pub(crate) fn at_main_tier_content_start(source: &str, start: usize) -> bool {
    let Some(prefix) = source.get(..start) else {
        return false;
    };
    let line_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let line_prefix = &prefix[line_start..];
    if !line_prefix.starts_with('*') {
        return false;
    }
    match line_prefix.find(":\t") {
        Some(sep) => line_prefix[sep + 2..].chars().all(|c| c == ' '),
        None => false,
    }
}

/// Whether byte offset `start` sits on a `%mor` / `%trn` tier line. Same
/// no-context situation as [`at_main_tier_content_start`]: the analyzer
/// may hold only the fragment (`|we`), so the tier is derived from the
/// enclosing source line.
pub(crate) fn on_mor_tier_line(source: &str, start: usize) -> bool {
    let Some(prefix) = source.get(..start) else {
        return false;
    };
    let line_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let line_prefix = &prefix[line_start..];
    line_prefix.starts_with("%mor:") || line_prefix.starts_with("%trn:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_check_52_family() {
        assert_eq!(leading_postfix_annotation("[/] we go"), Some("[/]"));
        assert_eq!(leading_postfix_annotation("[//] no"), Some("[//]"));
        assert_eq!(leading_postfix_annotation("[<] hi"), Some("[<]"));
        assert_eq!(
            leading_postfix_annotation("[: because] x"),
            Some("[: because]")
        );
        assert_eq!(leading_postfix_annotation("[\"] said"), Some("[\"]"));
        // Legal leading codes and non-codes do not match.
        assert_eq!(leading_postfix_annotation("[- heb] word"), None);
        assert_eq!(leading_postfix_annotation("[^ note] word"), None);
        assert_eq!(leading_postfix_annotation("word [/] ."), None);
        // Unclosed code still names the bracket.
        assert_eq!(leading_postfix_annotation("[/ ."), Some("["));
    }

    #[test]
    fn recognizes_empty_pos_items() {
        assert_eq!(mor_item_with_empty_pos("|we v|go .", true), Some("|we"));
        assert_eq!(
            mor_item_with_empty_pos("pro|we v|go |home .", true),
            Some("|home")
        );
        assert_eq!(mor_item_with_empty_pos("pro|we v|go .", true), None);
        // A lone pipe is a different malformation, not an empty-POS item.
        assert_eq!(mor_item_with_empty_pos("| we", true), None);
        // A fragment NOT at an item boundary is a split tail (CHECK 79
        // `n|dog|cat` splits to head `n|dog` + fragment `|cat`): its
        // leading pipe must not classify as empty POS...
        assert_eq!(mor_item_with_empty_pos("|cat .", false), None);
        // ...but a genuine empty-POS item LATER in the same fragment
        // still classifies.
        assert_eq!(mor_item_with_empty_pos("|cat |we .", false), Some("|we"));
    }

    #[test]
    fn item_boundary_is_whitespace_or_line_start() {
        let source = "%mor:	n|dog|cat |we .";
        let at = |needle: &str| source.find(needle).map(|i| at_item_boundary(source, i));
        assert_eq!(at("n|dog"), Some(true), "after tab = boundary");
        assert_eq!(at("|cat"), Some(false), "mid-item split tail");
        assert_eq!(at("|we"), Some(true), "after space = boundary");
        assert!(at_item_boundary(source, 0), "line start = boundary");
    }
}
