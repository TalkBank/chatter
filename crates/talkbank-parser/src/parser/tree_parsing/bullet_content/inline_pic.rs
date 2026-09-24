//! Parser for inline picture references (`%pic`) inside tier text.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{AsRawNode, InlinePicNode, SourceBound};
use talkbank_model::ParseOutcome;

/// A nonempty filename admitted from delimited inline-picture text.
///
/// The inline_pic node is now a single token matching:
///   `\u{15}%pic:"FILENAME"\u{15}`
///
/// Construction checks the delimiters and filename before ownership conversion.
struct InlinePictureFilename<'text>(&'text str);

impl<'text> InlinePictureFilename<'text> {
    fn admit(text: &'text str) -> Option<Self> {
        let inner = text.strip_prefix("\u{15}%pic:\"")?;
        let filename = inner.strip_suffix("\"\u{15}")?;
        if filename.is_empty() {
            None
        } else {
            Some(Self(filename))
        }
    }

    fn into_owned(self) -> String {
        self.0.to_owned()
    }
}

/// Converts one `inline_pic` node into a picture filename.
///
/// **Grammar Rule (coarsened):**
/// ```text
/// inline_pic: $ => token(/\u0015%pic:"[a-zA-Z0-9][a-zA-Z0-9\/\-_'.]*"\u0015/)
/// ```
///
/// Returns the admitted filename or rejection, streaming errors via `ErrorSink`.
pub(super) fn parse_inline_pic<'tree>(
    typed: SourceBound<'tree, '_, InlinePicNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<String> {
    let node = typed.raw_node();
    let source = typed.source();
    let text = typed.text();

    match InlinePictureFilename::admit(text) {
        Some(filename) => ParseOutcome::parsed(filename.into_owned()),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "inline_pic"),
                "Missing filename in inline picture reference".to_string(),
            ));
            ParseOutcome::rejected()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::error::ErrorCollector;
    use crate::generated_traversal::FromNodeKind;

    #[test]
    fn real_picture_tokens_admit_filenames_and_refuse_malformed_delimiters() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/media-bullets.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(_) = InlinePicNode::from_node(node) else {
                continue;
            };
            let text = &source[node.byte_range()];
            let admitted = InlinePictureFilename::admit(text).expect("reference picture");
            let errors = ErrorCollector::new();
            assert_eq!(
                parse_inline_pic(
                    parsed
                        .bind(node)
                        .expect("canonical picture range")
                        .typed::<InlinePicNode>()
                        .expect("picture kind"),
                    &errors
                )
                .into_option(),
                Some(admitted.into_owned())
            );
            assert!(errors.into_vec().is_empty());
            // Retained-token delimiter mutations exercise the admission boundary,
            // not fabricated tree-sitter nodes or production recovery witnesses.
            assert!(InlinePictureFilename::admit(&text[1..]).is_none());
            assert!(InlinePictureFilename::admit(&text[..text.len() - 1]).is_none());
            assert!(InlinePictureFilename::admit("\u{15}%pic:\"\"\u{15}").is_none());
            // The leaf cannot receive a different source after admission.
            witnessed += 1;
        }
        assert!(witnessed > 0);
    }
}
