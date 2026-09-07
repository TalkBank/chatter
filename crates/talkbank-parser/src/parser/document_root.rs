//! One classification owns document lowering and whole-input diagnostics.
//!
//! Tree-sitter can place recovery siblings before or after a complete document.
//! Selecting the first source child loses a later document; examining only the
//! selected document loses errors outside it. Keep both scopes in one owner.

use crate::generated_traversal::{
    FromNodeKind, FullDocumentChildren, FullDocumentNode, extract_full_document,
    extract_full_document_from_error_recovery,
};
use crate::node_types::SOURCE_FILE;
use tree_sitter::{Node, Tree};

/// The document position and its complete syntax-error scope, classified once.
///
/// Private fields prevent combining a document with an unrelated syntax root.
/// Construct through [`Self::classify`]; use [`Self::node`] for document-local
/// traversal and [`Self::into_children`] for lowering.
#[derive(Debug)]
pub struct DocumentRoot<'tree> {
    syntax_root: Node<'tree>,
    document: DocumentShape<'tree>,
}

// This short-lived classification is built once and consumed immediately.
// Boxing the recovered carrier would add a per-document allocation to avoid
// moving it once; it is not stored in a collection.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum DocumentShape<'tree> {
    Complete {
        node: Node<'tree>,
        document: FullDocumentNode<'tree>,
    },
    Recovered {
        node: Node<'tree>,
        children: FullDocumentChildren<'tree>,
    },
    NotADocument {
        node: Node<'tree>,
    },
}

impl<'tree> DocumentRoot<'tree> {
    /// Locate a complete document even when recovery precedes it.
    ///
    /// The source grammar selects one document or fragment. A complete
    /// document takes precedence over a recovery sibling; otherwise preserve
    /// the existing recovery classification at the document position. Every
    /// path retains the original syntax root for whole-input diagnostics.
    #[must_use]
    pub fn classify(tree: &'tree Tree) -> Self {
        let syntax_root = tree.root_node();
        if syntax_root.kind() == SOURCE_FILE {
            let mut cursor = syntax_root.walk();
            for child in syntax_root.children(&mut cursor) {
                if let Some(document) = FullDocumentNode::from_node(child) {
                    return Self {
                        syntax_root,
                        document: DocumentShape::Complete {
                            node: child,
                            document,
                        },
                    };
                }
            }
        }
        let node = match syntax_root.child(0) {
            Some(child) if syntax_root.kind() == SOURCE_FILE => child,
            Some(_) | None => syntax_root,
        };
        Self {
            syntax_root,
            document: Self::of_node(node),
        }
    }

    fn of_node(node: Node<'tree>) -> DocumentShape<'tree> {
        if let Some(document) = FullDocumentNode::from_node(node) {
            return DocumentShape::Complete { node, document };
        }
        match extract_full_document_from_error_recovery(node) {
            Some(children) => DocumentShape::Recovered { node, children },
            None => DocumentShape::NotADocument { node },
        }
    }

    /// The selected document or recovery node for document-local traversal.
    /// This scope deliberately excludes recovery siblings; diagnostics use the
    /// separately retained whole syntax root.
    #[must_use]
    pub fn node(&self) -> Node<'tree> {
        match &self.document {
            DocumentShape::Complete { node, .. }
            | DocumentShape::Recovered { node, .. }
            | DocumentShape::NotADocument { node } => *node,
        }
    }

    /// The original tree root, including recovery outside the document.
    pub(crate) fn syntax_root(&self) -> Node<'tree> {
        self.syntax_root
    }

    /// Whether the document position required structural recovery.
    #[must_use]
    pub fn recovered_at_root(&self) -> bool {
        matches!(self.document, DocumentShape::Recovered { .. })
    }

    /// The admitted document children to lower, or no document-shaped content.
    #[must_use]
    pub fn into_children(self) -> Option<FullDocumentChildren<'tree>> {
        match self.document {
            DocumentShape::Complete { document, .. } => Some(extract_full_document(document)),
            DocumentShape::Recovered { children, .. } => Some(children),
            DocumentShape::NotADocument { .. } => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use talkbank_model::{ErrorCode, ErrorCollector, Span};

    const DOCUMENT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|test|CHI|||||Child|||\n*CHI:\thello .\n@End\n";

    #[test]
    fn recovery_siblings_preserve_the_document_and_remain_diagnostic() {
        let cases = [
            (
                format!("{DOCUMENT}oops"),
                DOCUMENT.len(),
                DOCUMENT.len() + 4,
            ),
            (format!("@End\n{DOCUMENT}"), 0, 5),
        ];
        let parser = TreeSitterParser::new().expect("grammar loads");
        for (input, start, end) in cases {
            let tree = parser.parse_tree_incremental(&input, None).expect("tree");
            let root = DocumentRoot::classify(&tree);
            assert_eq!(root.node().kind(), "full_document");
            assert_eq!(root.syntax_root().byte_range(), 0..input.len());
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(&input, &errors);
            assert_eq!(file.utterances().count(), 1, "{input:?}");
            let errors = errors.to_vec();
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code, ErrorCode::UnparsableContent);
            assert_eq!(
                errors[0].location.span,
                Span::new(
                    start.try_into().expect("small fixture"),
                    end.try_into().expect("small fixture")
                ),
            );
        }
    }

    #[test]
    fn genuine_document_recovery_still_defers_missing_end_to_validation() {
        let parser = TreeSitterParser::new().expect("grammar loads");
        for input in [DOCUMENT, DOCUMENT.trim_end_matches("@End\n")] {
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(input, &errors);
            assert_eq!(file.utterances().count(), 1);
            assert!(errors.to_vec().is_empty(), "{:?}", errors.to_vec());
        }
    }

    // Spec boundary: a missing document terminator must not discard speech.
    #[test]
    fn truncated_spec_retains_speech_without_final_newline() {
        let source = include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E502_1.cha"
        );
        let parser = TreeSitterParser::new().expect("grammar loads");
        for input in [source, source.trim_end_matches('\n')] {
            let errors = ErrorCollector::new();
            let mut file = parser.parse_chat_file_streaming(input, &errors);
            assert_eq!(file.utterances().count(), 1, "speech retained: {input:?}");
            assert!(errors.to_vec().is_empty(), "{:?}", errors.to_vec());
            file.validate_with_alignment(&errors, talkbank_model::model::TranscriptName::Anonymous);
            let errors = errors.into_vec();
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code, ErrorCode::MissingEndHeader);
            assert_eq!(errors[0].location.span.start as usize, input.len());
        }
    }

    #[test]
    fn recovered_terminal_speech_keeps_caller_coordinates() {
        use talkbank_model::ChatParser;
        let source = include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E502_1.cha"
        )
        .trim_end_matches('\n');
        let parser = TreeSitterParser::new().expect("grammar loads");
        for origin in [0, 37, u32::MAX as usize - source.len()] {
            let errors = ErrorCollector::new();
            let file = ChatParser::parse_chat_file(&parser, source, origin, &errors)
                .into_option()
                .expect("recovered document");
            let utterance = file.utterances().next().expect("retained speech");
            assert_eq!(
                utterance.main.span.start as usize,
                origin + source.find("*CHI").expect("spec speaker")
            );
            assert_eq!(utterance.main.span.end as usize, origin + source.len());
            assert_eq!(
                utterance.main.speaker_span.start,
                utterance.main.span.start + 1
            );
            assert!(errors.into_vec().is_empty());
        }
    }
}
