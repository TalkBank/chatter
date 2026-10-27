//! Parsed speaker occurrences own source projection and declaration identity.

use talkbank_parser::node_types::{ID_SPEAKER, PARTICIPANT, PARTICIPANTS_HEADER, SPEAKER};
use tower_lsp::lsp_types::Range;
use tree_sitter::Node;

use crate::backend::requests::find_ancestor_kind;
use crate::backend::utils;

#[derive(Clone, Copy)]
pub(crate) enum SpeakerRole {
    Declaration,
    Reference,
}

/// Only a recognized speaker token with readable source enters navigation.
pub(crate) struct SpeakerOccurrence<'tree, 'source> {
    node: Node<'tree>,
    source: &'source str,
    code: &'source str,
    role: SpeakerRole,
}

impl<'tree, 'source> SpeakerOccurrence<'tree, 'source> {
    pub(crate) fn read(node: Node<'tree>, source: &'source str) -> Option<Self> {
        let role = match node.kind() {
            SPEAKER => match node.parent() {
                Some(parent)
                    if parent.kind() == PARTICIPANT
                        && find_ancestor_kind(parent, PARTICIPANTS_HEADER).is_some() =>
                {
                    SpeakerRole::Declaration
                }
                _ => SpeakerRole::Reference,
            },
            ID_SPEAKER => SpeakerRole::Reference,
            _ => return None,
        };
        Some(Self {
            node,
            source,
            code: node.utf8_text(source.as_bytes()).ok()?,
            role,
        })
    }

    pub(crate) fn code(&self) -> &'source str {
        self.code
    }

    pub(crate) fn role(&self) -> SpeakerRole {
        self.role
    }

    pub(crate) fn range(&self) -> Range {
        Range::new(
            utils::offset_to_position(self.source, self.node.start_byte()),
            utils::offset_to_position(self.source, self.node.end_byte()),
        )
    }
}
