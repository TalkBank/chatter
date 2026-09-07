//! Admit the grammar's flattened simple main tier at document EOF.

use crate::TreeSitterParser;
use crate::generated_traversal::{
    ColonNode, ContentsNode, FromNodeKind, SpeakerNode, StarNode, TabNode, TerminatorChoice,
};
use talkbank_model::model::MainTier;
use talkbank_model::{ChatParser, ErrorSink, ParseOutcome};
use tree_sitter::Node;

/// A complete terminal sequence paired with its original source coordinates.
/// Both recovery diagnostics and document lowering use this same admission.
pub(crate) struct TerminalMainTier<'source> {
    input: &'source str,
    origin: usize,
}

impl<'source> TerminalMainTier<'source> {
    pub(crate) fn admit<'tree>(
        first: Node<'tree>,
        mut remaining: impl Iterator<Item = Node<'tree>>,
        source: &'source str,
    ) -> Option<Self> {
        StarNode::from_node(first)?;
        SpeakerNode::from_node(remaining.next()?)?;
        ColonNode::from_node(remaining.next()?)?;
        TabNode::from_node(remaining.next()?)?;
        ContentsNode::from_node(remaining.next()?)?;
        let ending = remaining.next()?;
        TerminatorChoice::from_node(ending)?;
        if remaining.next().is_some() || ending.end_byte() != source.len() {
            return None;
        }
        Some(Self {
            input: source.get(first.start_byte()..ending.end_byte())?,
            origin: first.start_byte(),
        })
    }

    /// Reuse the normal fragment parser, which clips its synthetic newline and
    /// admits the complete coordinate range before rebasing model/error spans.
    pub(crate) fn lower(
        self,
        parser: &TreeSitterParser,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        ChatParser::parse_main_tier(parser, self.input, self.origin, errors)
    }
}
