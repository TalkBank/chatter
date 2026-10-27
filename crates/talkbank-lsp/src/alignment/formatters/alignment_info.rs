//! Markdown table formatter for [`AlignmentHoverInfo`].
//!
//! Produces a compact two-column Markdown table (field | value) suitable for
//! LSP hover popups. Fields with empty values are omitted to keep the card
//! minimal.

use crate::alignment::types::AlignmentHoverInfo;

/// Format alignment hover info as a compact Markdown table for LSP hover display.
///
/// Layout:
/// ```text
/// **element**: *Element Type*
///
/// | tier    | annotation |
/// |---------|------------|
/// | ← main  | …          |
/// | %mor    | …          |
/// | %gra    | …          |
///
/// **Key**: value · **Key**: value
/// ```
///
/// The table is omitted when no tier alignments are present.
/// Details (key-value pairs) are inlined as bold labels separated by ` · `.
pub fn format_alignment_info(info: &AlignmentHoverInfo) -> String {
    let mut md = String::new();

    // Header line: bold element content + italic element type label.
    md.push_str(&format!(
        "**{}**: *{}*\n\n",
        MarkdownText::from(info.element_content.as_str()),
        MarkdownText::from(info.element_type.as_str())
    ));

    // Alignment table, only emitted when at least one tier has data.
    let rows: Vec<(&str, &str)> = [
        ("← main", info.aligned_to_main.as_deref()),
        ("%mor", info.aligned_to_mor.as_deref()),
        ("%gra", info.aligned_to_gra.as_deref()),
        ("%pho", info.aligned_to_pho.as_deref()),
        ("%mod", info.aligned_to_mod.as_deref()),
        ("%sin", info.aligned_to_sin.as_deref()),
    ]
    .iter()
    .filter_map(|(label, opt)| opt.map(|val| (*label, val)))
    .collect();

    if !rows.is_empty() {
        md.push_str("| tier | annotation |\n");
        md.push_str("|------|------------|\n");
        for (label, val) in &rows {
            md.push_str(&format!("| {} | {} |\n", label, MarkdownText::from(*val)));
        }
        md.push('\n');
    }

    // Additional details as compact inline key · value pairs.
    if !info.details.is_empty() {
        let detail_parts: Vec<String> = info
            .details
            .iter()
            .map(|(k, v)| {
                format!(
                    "**{}**: {}",
                    MarkdownText::from(k.as_str()),
                    MarkdownText::from(v.as_str())
                )
            })
            .collect();
        md.push_str(&detail_parts.join(" · "));
        md.push('\n');
    }

    md
}

/// Text admitted for a single Markdown table cell or inline label. Escaping is
/// owned here so model renderers and JSON sidecars retain plain CHAT strings.
struct MarkdownText(String);

impl From<&str> for MarkdownText {
    fn from(text: &str) -> Self {
        let mut escaped = String::with_capacity(text.len());
        for ch in text.chars() {
            if matches!(ch, '\n' | '\r') {
                escaped.push(' ');
            } else {
                if ch.is_ascii_punctuation() {
                    escaped.push('\\');
                }
                escaped.push(ch);
            }
        }
        Self(escaped)
    }
}

impl std::fmt::Display for MarkdownText {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_markdown_keeps_payload_out_of_markup() {
        let info = AlignmentHoverInfo::new("Word", "<a> [*]")
            .with_mor("n|a")
            .with_details(vec![("x*".into(), "a\nb".into())]);
        let rendered = format_alignment_info(&info);
        assert!(rendered.contains(r"**\<a\> \[\*\]**"));
        assert!(rendered.contains(r"| %mor | n\|a |"));
        assert!(rendered.contains(r"**x\***: a b"));
    }
}
