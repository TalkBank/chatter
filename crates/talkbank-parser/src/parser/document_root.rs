//! One classification owns document lowering and whole-input diagnostics.
//!
//! Tree-sitter can place recovery siblings before or after a complete document.
//! Selecting the first source child loses a later document; examining only the
//! selected document loses errors outside it. Keep both scopes in one owner.

use crate::generated_traversal::{
    AsRawNode, FullDocumentChildren, FullDocumentNode, ParsedSource, SourceBindingError,
    SourceBound, SourceChildren, SourceSlice,
};
use crate::node_types::SOURCE_FILE;
use tree_sitter::Node;

/// The document position and its complete syntax-error scope, classified once.
///
/// Private fields prevent combining a document with an unrelated syntax root.
/// Construct through [`Self::classify`]; use [`Self::node`] for document-local
/// traversal and the consuming part visitor for lowering.
#[derive(Debug)]
pub struct DocumentRoot<'tree> {
    parsed: &'tree ParsedSource<'tree>,
    document: DocumentShape<'tree>,
}

// This short-lived classification is built once and consumed immediately.
// Boxing the recovered carrier would add a per-document allocation to avoid
// moving it once; it is not stored in a collection.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum DocumentShape<'tree> {
    Complete {
        document: SourceBound<'tree, 'tree, FullDocumentNode<'tree>>,
    },
    Recovered {
        children: SourceChildren<'tree, 'tree, FullDocumentChildren<'tree>>,
    },
    NotADocument {
        node: SourceSlice<'tree, 'tree>,
    },
}

/// A source-ordered part of the classified root. Recovery nodes are siblings
/// of the selected document from the same immutable producer, not arbitrary
/// nodes supplied alongside a separately selected document.
#[allow(clippy::large_enum_variant)] // Borrowed generated carrier; keep root iteration allocation-free.
pub(crate) enum DocumentPart<'tree> {
    Document(SourceChildren<'tree, 'tree, FullDocumentChildren<'tree>>),
    Recovery(SourceSlice<'tree, 'tree>),
}

impl<'tree> DocumentRoot<'tree> {
    /// Locate a complete document even when recovery precedes it.
    ///
    /// The source grammar selects one document or fragment. A complete
    /// document takes precedence over a recovery sibling; otherwise preserve
    /// the existing recovery classification at the document position. Every
    /// path retains the original syntax root for whole-input diagnostics.
    pub fn classify(parsed: &'tree ParsedSource<'tree>) -> Result<Self, SourceBindingError> {
        let syntax_root = parsed.root()?;
        if syntax_root.raw_node().kind() == SOURCE_FILE {
            let mut cursor = syntax_root.raw_node().walk();
            for child in syntax_root.children(&mut cursor) {
                if let Some(document) = child?.typed::<FullDocumentNode>() {
                    return Ok(Self {
                        parsed,
                        document: DocumentShape::Complete { document },
                    });
                }
            }
        }
        let node = if syntax_root.raw_node().kind() == SOURCE_FILE {
            let mut cursor = syntax_root.raw_node().walk();
            syntax_root
                .children(&mut cursor)
                .next()
                .transpose()?
                .unwrap_or(syntax_root)
        } else {
            syntax_root
        };
        Ok(Self {
            parsed,
            document: Self::of_node(node),
        })
    }

    fn of_node(node: SourceSlice<'tree, 'tree>) -> DocumentShape<'tree> {
        if let Some(document) = node.typed::<FullDocumentNode>() {
            return DocumentShape::Complete { document };
        }
        match node.extract_full_document_from_error_recovery() {
            Some(children) => DocumentShape::Recovered { children },
            None => DocumentShape::NotADocument { node },
        }
    }

    /// The selected document or recovery node for document-local traversal.
    /// This scope deliberately excludes recovery siblings; diagnostics use the
    /// separately retained whole syntax root.
    #[must_use]
    pub fn node(&self) -> Node<'tree> {
        match &self.document {
            DocumentShape::Complete { document } => document.raw_node(),
            DocumentShape::Recovered { children } => children.parent().raw_node(),
            DocumentShape::NotADocument { node } => node.raw_node(),
        }
    }

    /// The original tree root, including recovery outside the document.
    pub(crate) fn syntax_root(&self) -> Node<'tree> {
        self.parsed.root_node()
    }

    /// The producer-owned input and tree for this document classification.
    pub(crate) fn parsed_source(&self) -> &'tree ParsedSource<'tree> {
        self.parsed
    }

    /// Whether the document position required structural recovery.
    #[must_use]
    pub fn recovered_at_root(&self) -> bool {
        matches!(self.document, DocumentShape::Recovered { .. })
    }

    /// Consume the classification without discarding outer recovery. The
    /// whole-tree backstop remains responsible for other missing/nested nodes.
    pub(crate) fn for_each_part(
        self,
        mut visit: impl FnMut(DocumentPart<'tree>),
    ) -> Result<(), SourceBindingError> {
        let selected = self.node();
        let syntax_root = self.parsed.root()?;
        let mut document = Some(match self.document {
            DocumentShape::Complete { document } => document.extract(),
            DocumentShape::Recovered { children, .. } => {
                // A reconstructed ERROR wrapper does not establish a complete
                // document boundary. Its siblings remain whole-input recovery;
                // e.g. a lone End after a malformed Begin is not a duplicate.
                visit(DocumentPart::Document(children));
                return Ok(());
            }
            // Without admitted document structure, no sibling is an outer
            // document region. The whole-input backstop retains that failure.
            DocumentShape::NotADocument { .. } => return Ok(()),
        });
        if selected == syntax_root.raw_node() {
            if let Some(children) = document {
                visit(DocumentPart::Document(children));
            }
            return Ok(());
        }
        let mut cursor = syntax_root.raw_node().walk();
        for child in syntax_root.children(&mut cursor) {
            let child = child?;
            if child.raw_node() == selected {
                if let Some(children) = document.take() {
                    visit(DocumentPart::Document(children));
                }
            } else if child.raw_node().is_error() {
                visit(DocumentPart::Recovery(child));
            }
        }
        Ok(())
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
                ErrorCode::UnparsableContent,
            ),
            (
                format!("@End\n{DOCUMENT}"),
                0,
                5,
                ErrorCode::DuplicateHeader,
            ),
        ];
        let parser = TreeSitterParser::new().expect("grammar loads");
        for (input, start, end, expected_code) in cases {
            let tree = parser.parse_source_incremental(&input, None).expect("tree");
            let root = DocumentRoot::classify(&tree).expect("canonical document ranges");
            assert_eq!(root.node().kind(), "full_document");
            assert_eq!(root.syntax_root().byte_range(), 0..input.len());
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(&input, &errors);
            assert_eq!(file.utterances().count(), 1, "{input:?}");
            let errors = errors.to_vec();
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].code, expected_code);
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
