//! Find all references for speaker codes.
//!
//! Implements `textDocument/references`, when the cursor is on a speaker code,
//! returns parsed occurrences of that speaker code in the document:
//! - `@Participants` entries when `includeDeclaration` is enabled
//! - `@ID` header speaker fields
//! - `*SPEAKER:` main tier prefixes
//! - `@Birth of SPEAKER`, `@L1 of SPEAKER` headers

use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

use super::super::utils;
use super::speaker_occurrence::{SpeakerOccurrence, SpeakerRole};
use crate::backend::requests::{find_ancestor_kind, walk_nodes};
use talkbank_parser::node_types::{ID_SPEAKER, SPEAKER};

/// Finds all references to the speaker code at the given position.
pub fn references(
    tree: &Tree,
    uri: &Url,
    doc: &str,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let scope = match include_declaration {
        true => ReferenceScope::WithDeclaration,
        false => ReferenceScope::ReferencesOnly,
    };
    let speaker = find_speaker_at_position(tree, doc, position)?;
    let locations = collect_speaker_locations(tree, uri, doc, speaker.code(), scope);
    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

/// Wire inclusion is admitted once; consumers exhaustively match the scope.
#[derive(Clone, Copy)]
enum ReferenceScope {
    ReferencesOnly,
    WithDeclaration,
}

impl ReferenceScope {
    fn includes(self, role: SpeakerRole) -> bool {
        match (self, role) {
            (Self::ReferencesOnly, SpeakerRole::Declaration) => false,
            (Self::ReferencesOnly, SpeakerRole::Reference)
            | (Self::WithDeclaration, SpeakerRole::Declaration | SpeakerRole::Reference) => true,
        }
    }
}

fn find_speaker_at_position<'tree, 'source>(
    tree: &'tree Tree,
    doc: &'source str,
    position: Position,
) -> Option<SpeakerOccurrence<'tree, 'source>> {
    let offset = utils::position_to_offset(doc, position);
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;
    let speaker =
        find_ancestor_kind(node, SPEAKER).or_else(|| find_ancestor_kind(node, ID_SPEAKER))?;
    SpeakerOccurrence::read(speaker, doc)
}

/// Collects all locations of a speaker code in the document.
fn collect_speaker_locations(
    tree: &Tree,
    uri: &Url,
    doc: &str,
    speaker_name: &str,
    scope: ReferenceScope,
) -> Vec<Location> {
    let mut locations = Vec::new();
    let root = tree.root_node();

    walk_nodes(root, |node| {
        if let Some(occurrence) = SpeakerOccurrence::read(node, doc)
            && occurrence.code() == speaker_name
            && scope.includes(occurrence.role())
        {
            locations.push(Location {
                uri: uri.clone(),
                range: occurrence.range(),
            });
        }
        None::<()>
    });

    locations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_tree_incremental as parse_tree;

    const SOURCE: &str =
        include_str!("../../../../../corpus/reference/core/basic-conversation.cha");

    #[test]
    fn test_references_finds_all_speaker_occurrences() {
        let input = SOURCE;
        let tree = parse_tree(input);
        let uri = Url::parse("file:///test.cha").unwrap();

        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let refs = references(&tree, &uri, input, Position { line, character: 1 }, true);
        assert!(refs.is_some());
        // @Participants CHI + @ID CHI + 2x *CHI: = 4
        assert_eq!(refs.unwrap().len(), 4);
    }

    #[test]
    fn test_references_returns_none_for_non_speaker() {
        let input = SOURCE;
        let tree = parse_tree(input);
        let uri = Url::parse("file:///test.cha").unwrap();

        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let refs = references(&tree, &uri, input, Position { line, character: 6 }, true);
        assert!(refs.is_none());
    }
}
