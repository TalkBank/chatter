//! Speaker code rename support.
//!
//! Implements `textDocument/rename` and `textDocument/prepareRename` for CHAT
//! speaker codes. Renaming a speaker code updates all occurrences:
//! - `@Participants` header entries
//! - `@ID` header speaker fields
//! - `*SPEAKER:` main tier prefixes
//! - `@Birth of SPEAKER`, `@L1 of SPEAKER` headers

use talkbank_transform::speaker_id::{SpeakerRename, SpeakerRenameDecision, SpeakerRenameError};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

use super::super::utils;
use crate::backend::capabilities::VersionedEdits;
use crate::backend::diagnostics::{CurrentRevisionDiagnostics, RevisionDiagnostics};
use crate::backend::requests::{find_ancestor_kind, walk_nodes};

use talkbank_parser::node_types::{
    BIRTH_OF_HEADER, BIRTHPLACE_OF_HEADER, ID_SPEAKER, L1_OF_HEADER, SPEAKER,
};

/// Validates that the cursor is on a renameable speaker code and returns
/// its range and current text. Used for `textDocument/prepareRename`.
pub fn prepare_rename(tree: &Tree, doc: &str, position: Position) -> Option<PrepareRenameResponse> {
    if tree.root_node().has_error() {
        return None;
    }
    let (range, text) = find_speaker_at_position(tree, doc, position)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range,
        placeholder: text,
    })
}

/// Proposed speaker edits bound to the exact open revision that produced them.
/// Only a current-revision capability and negotiated client support can turn
/// these proposals into edits an editor may apply.
pub(crate) struct PreparedRename<'a> {
    revision: &'a RevisionDiagnostics,
    edits: Vec<TextEdit>,
}

impl PreparedRename<'_> {
    pub(crate) fn finish(
        self,
        current: CurrentRevisionDiagnostics<'_>,
        _support: &VersionedEdits,
    ) -> Option<WorkspaceEdit> {
        if !std::ptr::eq(self.revision, current.snapshot()) {
            return None;
        }
        Some(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    uri: self.revision.uri().clone(),
                    version: Some(self.revision.version()),
                },
                edits: self.edits.into_iter().map(OneOf::Left).collect(),
            }])),
            ..Default::default()
        })
    }
}

/// Refusal to produce a complete identity-preserving edit.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RenameRefusal {
    #[error("repair syntax errors before renaming: speaker references may be hidden")]
    IncompleteSyntax,
    #[error("speaker source cannot be read from this syntax tree")]
    UnreadableSource,
    #[error(transparent)]
    Admission(#[from] SpeakerRenameError),
}

/// Prepare edits from one revision; this does not authorize publication.
pub(crate) fn prepare_rename_edit<'a>(
    tree: &Tree,
    revision: &'a RevisionDiagnostics,
    position: Position,
    new_name: &str,
) -> Result<Option<PreparedRename<'a>>, RenameRefusal> {
    Ok(rename_edits(tree, revision.source(), position, new_name)?
        .map(|edits| PreparedRename { revision, edits }))
}

fn rename_edits(
    tree: &Tree,
    doc: &str,
    position: Position,
    new_name: &str,
) -> Result<Option<Vec<TextEdit>>, RenameRefusal> {
    if tree.root_node().has_error() {
        return Err(RenameRefusal::IncompleteSyntax);
    }
    let Some((_range, old_name)) = find_speaker_at_position(tree, doc, position) else {
        return Ok(None);
    };
    let mut occurrences = Vec::new();
    if let Some(error) = walk_nodes(tree.root_node(), |node| {
        if matches!(node.kind(), SPEAKER | ID_SPEAKER) {
            match node.utf8_text(doc.as_bytes()) {
                Ok(text) => occurrences.push((text, node_to_range(node, doc))),
                Err(_) => return Some(RenameRefusal::UnreadableSource),
            }
        }
        None
    }) {
        return Err(error);
    }
    match SpeakerRename::admit(
        &old_name,
        new_name,
        occurrences.iter().map(|(text, _)| *text),
    )? {
        SpeakerRenameDecision::Unchanged => Ok(None),
        SpeakerRenameDecision::Rename(rename) => Ok(Some(
            occurrences
                .into_iter()
                .filter(|(text, _)| *text == rename.original())
                .map(|(_, range)| TextEdit {
                    range,
                    new_text: rename.replacement().to_owned(),
                })
                .collect(),
        )),
    }
}

/// Finds the speaker code node at the given cursor position and returns its
/// LSP range and text content. Returns `None` if the cursor is not on a
/// speaker code.
fn find_speaker_at_position(tree: &Tree, doc: &str, position: Position) -> Option<(Range, String)> {
    let offset = utils::position_to_offset(doc, position);
    if utils::offset_to_position(doc, offset) != position {
        return None;
    }
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;

    // Check if we're on a SPEAKER node (main tier or @Participants)
    if let Some(speaker_node) = find_ancestor_kind(node, SPEAKER) {
        let text = speaker_node.utf8_text(doc.as_bytes()).ok()?;
        let range = node_to_range(speaker_node, doc);
        return Some((range, text.to_string()));
    }

    // Check if we're on an ID_SPEAKER node (@ID header)
    if let Some(id_speaker_node) = find_ancestor_kind(node, ID_SPEAKER) {
        let text = id_speaker_node.utf8_text(doc.as_bytes()).ok()?;
        let range = node_to_range(id_speaker_node, doc);
        return Some((range, text.to_string()));
    }

    // Check if we're on a speaker code in @Birth of / @L1 of headers
    if let Some(speaker_text) = find_of_header_speaker(node, doc) {
        let range = node_to_range(node, doc);
        return Some((range, speaker_text));
    }

    None
}

/// Checks if a node is the speaker code portion of a `@Birth of SPEAKER` or
/// `@L1 of SPEAKER` header. These headers have a prefix node followed by a
/// speaker node.
fn find_of_header_speaker(node: Node, doc: &str) -> Option<String> {
    // The speaker code in "@Birth of CHI" is a SPEAKER child of the header node
    if node.kind() == SPEAKER
        && let Some(parent) = node.parent()
        && (parent.kind() == BIRTH_OF_HEADER
            || parent.kind() == L1_OF_HEADER
            || parent.kind() == BIRTHPLACE_OF_HEADER)
    {
        return node.utf8_text(doc.as_bytes()).ok().map(String::from);
    }
    None
}

/// Converts a tree-sitter node to an LSP `Range`.
fn node_to_range(node: Node, doc: &str) -> Range {
    Range {
        start: utils::offset_to_position(doc, node.start_byte()),
        end: utils::offset_to_position(doc, node.end_byte()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_tree_incremental as parse_tree;

    /// Valid CHAT header preamble for tests.
    const PREAMBLE: &str =
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n";

    #[test]
    fn test_prepare_rename_on_main_tier_speaker() {
        let input = format!("{PREAMBLE}@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n");
        let tree = parse_tree(&input);

        // Position on CHI in *CHI:
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = prepare_rename(&tree, &input, Position { line, character: 1 });
        assert!(result.is_some());

        if let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) = result {
            assert_eq!(placeholder, "CHI");
        }
    }

    #[test]
    fn test_prepare_rename_not_on_speaker() {
        let input = format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let tree = parse_tree(&input);

        // Position on "hello", not a speaker code
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = prepare_rename(&tree, &input, Position { line, character: 6 });
        assert!(result.is_none());
    }

    #[test]
    fn test_rename_speaker_all_locations() {
        let input = format!(
            "{PREAMBLE}@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n*MOT:\thi .\n*CHI:\tmore .\n@End\n"
        );
        let tree = parse_tree(&input);

        // Rename CHI → KID from the main tier
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = rename_edits(&tree, &input, Position { line, character: 1 }, "KID").unwrap();
        assert!(result.is_some());

        let edits = result.unwrap();

        // Should find: @Participants CHI, @ID CHI, *CHI: (twice) = 4 edits
        assert_eq!(
            edits.len(),
            4,
            "Expected 4 edits (participants + ID + 2 main tiers), got {}",
            edits.len()
        );

        // All edits should replace with "KID"
        for e in edits {
            assert_eq!(e.new_text, "KID");
        }
    }

    #[test]
    fn test_rename_does_not_touch_other_speakers() {
        let input = format!("{PREAMBLE}*CHI:\thello .\n*MOT:\thi .\n@End\n");
        let tree = parse_tree(&input);

        // Rename CHI → KID
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = rename_edits(&tree, &input, Position { line, character: 1 }, "KID").unwrap();

        let edits = result.unwrap();

        // Verify MOT is not in any edit range
        for e in edits {
            let start_offset = utils::position_to_offset(&input, e.range.start);
            let end_offset = utils::position_to_offset(&input, e.range.end);
            let replaced = &input[start_offset..end_offset];
            assert_eq!(
                replaced, "CHI",
                "Edit should only replace CHI, not '{}'",
                replaced
            );
        }
    }

    #[test]
    fn rename_covers_speaker_headers_and_other_spoken_events() {
        for (source, original, expected) in [
            (
                include_str!("../../../../../corpus/reference/core/headers-speaker-info.cha"),
                "MOT",
                5,
            ),
            (
                include_str!("../../../../../corpus/reference/core/headers-speaker-info.cha"),
                "F_A_T",
                4,
            ),
            (
                include_str!(
                    "../../../../../corpus/reference/edge-cases/other-spoken-event-no-terminator-space.cha"
                ),
                "INV",
                12,
            ),
        ] {
            let tree = parse_tree(source);
            let position = utils::offset_to_position(source, source.find(original).unwrap());
            for target in ["A_B", "A+B", "A-B", "A'B", "kid", "123"] {
                let mut edits = rename_edits(&tree, source, position, target)
                    .unwrap()
                    .unwrap();
                assert_eq!(edits.len(), expected);
                edits.sort_by_key(|edit| {
                    std::cmp::Reverse(utils::position_to_offset(source, edit.range.start))
                });
                let mut applied = source.to_owned();
                for edit in edits {
                    let range = utils::position_to_offset(source, edit.range.start)
                        ..utils::position_to_offset(source, edit.range.end);
                    assert_eq!(&source[range.clone()], original);
                    applied.replace_range(range, &edit.new_text);
                }
                assert!(!parse_tree(&applied).root_node().has_error(), "{target}");
            }
        }
    }

    #[test]
    fn prepared_rename_cannot_cross_open_document_instances() {
        use crate::backend::diagnostics::document_diagnostics;
        use crate::backend::documents::OpenDocument;
        use crate::backend::state::Backend;
        use std::sync::Arc;
        let (service, _socket) = tower_lsp::LspService::new(Backend::new);
        let backend = service.inner();
        let uri = Url::parse("file:///rename-revision.cha").unwrap();
        let source: Arc<str> =
            include_str!("../../../../../corpus/reference/core/basic-conversation.cha").into();
        let tree = parse_tree(&source);
        let position = utils::offset_to_position(&source, source.find("*CHI:").unwrap() + 1);
        let capabilities = serde_json::from_value(serde_json::json!({
            "workspace":{"workspaceEdit":{"documentChanges":true}}
        }))
        .unwrap();
        let support = VersionedEdits::negotiate(&capabilities).unwrap();
        backend.documents.insert(
            uri.clone(),
            crate::backend::documents::DocumentSession::Synchronized(Arc::new(OpenDocument {
                source: Arc::clone(&source),
                version: 1,
            })),
        );
        let original = document_diagnostics(backend, &uri).unwrap().unwrap();
        let prepared = prepare_rename_edit(&tree, &original, position, "KID")
            .unwrap()
            .unwrap();
        assert!(
            original
                .with_current(backend, |current| prepared.finish(current, &support))
                .unwrap()
                .is_some()
        );
        let stale = prepare_rename_edit(&tree, &original, position, "KID")
            .unwrap()
            .unwrap();
        backend.documents.remove(&uri);
        backend.documents.insert(
            uri.clone(),
            crate::backend::documents::DocumentSession::Synchronized(Arc::new(OpenDocument {
                source,
                version: 1,
            })),
        );
        assert!(original.with_current(backend, |_| ()).is_none());
        let reopened = document_diagnostics(backend, &uri).unwrap().unwrap();
        assert!(
            reopened
                .with_current(backend, |current| stale.finish(current, &support))
                .unwrap()
                .is_none()
        );
    }
}
