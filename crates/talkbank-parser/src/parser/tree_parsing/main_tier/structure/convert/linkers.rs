//! Linker decoding through generated first/repeated groups.
//!
//! CHAT reference: <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, KindSlot, KindSlotValue, LinkerChoice, LinkersChild0Child0Choice,
    LinkersChild1Child0Choice, LinkersNode, NoChild, SlotView, WhitespacesNode, extract_linkers,
};
use crate::model::{Linker, LinkerKind};
use crate::parser::node_span::span_of;
use tree_sitter::Node;

/// The model kind and source span come from the same generated alternative.
trait LinkerToken<'tree>: AsRawNode<'tree> + FromNodeKind<'tree> + Clone {
    fn to_linker(&self) -> Linker;
}

macro_rules! linker_tokens {
    ($($choice:ident),* $(,)?) => {
        $(
            impl<'tree> LinkerToken<'tree> for $choice<'tree> {
                fn to_linker(&self) -> Linker {
                    let kind = match self {
                        Self::LinkerLazyOverlap(_) => LinkerKind::LazyOverlapPrecedes,
                        Self::LinkerQuickUptake(_) => LinkerKind::OtherCompletion,
                        Self::LinkerQuickUptakeOverlap(_) => LinkerKind::QuickUptakeOverlap,
                        Self::LinkerQuotationFollows(_) => LinkerKind::QuotationFollows,
                        Self::LinkerSelfCompletion(_) => LinkerKind::SelfCompletion,
                        Self::CaTechnicalBreakLinker(_) => LinkerKind::TcuContinuation,
                        Self::CaNoBreakLinker(_) => LinkerKind::NoBreakTcuContinuation,
                    };
                    Linker::new(kind, span_of(self.raw_node()))
                }
            }
        )*
    };
}
linker_tokens!(
    LinkerChoice,
    LinkersChild0Child0Choice,
    LinkersChild1Child0Choice
);

/// Source order is independent of when a generated sink exposes a token.
/// Only typed linker tokens enter; their model kind and span stay paired.
struct SourceOrderedLinkers(Vec<Linker>);

impl SourceOrderedLinkers {
    fn push<'tree>(&mut self, token: &impl LinkerToken<'tree>) {
        let linker = token.to_linker();
        // Normal traversal appends in constant time. Only displaced recovery
        // can require insertion among already-read tokens.
        if self
            .0
            .last()
            .is_none_or(|last| last.span.start <= linker.span.start)
        {
            self.0.push(linker);
        } else {
            let position = self
                .0
                .partition_point(|prior| prior.span.start <= linker.span.start);
            self.0.insert(position, linker);
        }
    }

    fn finish(self) -> Vec<Linker> {
        self.0
    }
}

/// The current grammar exposes concrete linker kinds, not a legacy linker
/// wrapper. Required/repeated groups retain their recovery states and sinks.
pub(super) fn parse_linkers(
    typed: LinkersNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<Linker> {
    let children = extract_linkers(typed);
    let mut linkers = SourceOrderedLinkers(Vec::new());

    // First and repeated groups have distinct generated carrier types but the
    // same field shape. Expansion keeps both enum matches compiler-exhaustive.
    macro_rules! group {
        ($slot:expr) => {
            match $slot.view() {
                SlotView::Present(group) => {
                    push_linker(group.child_0.slot(), source, errors, &mut linkers);
                    match group.child_1.slot().view() {
                        SlotView::Present(_) | SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                        }
                        SlotView::Error(node) => report_unexpected(node, source, errors),
                    }
                    for node in &group.unexpected {
                        push_displaced(*node, source, errors, &mut linkers);
                    }
                }
                SlotView::Error(node) => report_unexpected(node, source, errors),
                SlotView::Absent(NoChild) => {}
            }
        };
    }
    group!(children.child_0.slot());
    for element in children.child_1.slot() {
        group!(element.slot());
    }
    for node in &children.unexpected {
        push_displaced(*node, source, errors, &mut linkers);
    }
    linkers.finish()
}

fn push_linker<'tree, T: LinkerToken<'tree>>(
    slot: &KindSlot<'tree, T>,
    source: &str,
    errors: &impl ErrorSink,
    linkers: &mut SourceOrderedLinkers,
) {
    match slot.known_or_placeholder() {
        // Preserve kind-admitted MISSING tokens and their zero-width spans;
        // the whole-tree backstop owns the missing-token diagnostic.
        KindSlotValue::Present(token) | KindSlotValue::Placeholder(token) => linkers.push(&token),
        KindSlotValue::Error(node) => {
            push_displaced(node, source, errors, linkers);
        }
        KindSlotValue::Absent(NoChild) => {}
    }
}

/// A recoverable concrete linker can be displaced out of a generated group.
/// Keep it as the former flat walk did; classify only this recovery boundary
/// through the generated supertype, never a second hand-written kind list.
fn push_displaced(
    node: Node<'_>,
    source: &str,
    errors: &impl ErrorSink,
    linkers: &mut SourceOrderedLinkers,
) {
    if let Some(token) = LinkerChoice::from_node(node) {
        linkers.push(&token);
    } else if WhitespacesNode::from_node(node).is_none() {
        report_unexpected(node, source, errors);
    }
}

fn report_unexpected(node: Node<'_>, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.byte_range(), ""),
        format!(
            "Expected 'linker' or 'whitespaces' in linkers, found '{}'",
            node.kind()
        ),
    ));
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;

    /// Sink delivery order must not reorder a recovered model. These tokens
    /// come from the committed multiple-linker fixture, not fabricated spans.
    #[test]
    fn parsed_linkers_remain_source_ordered_when_delivered_backwards() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/linkers-multiple.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut linkers = SourceOrderedLinkers(Vec::new());
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            // A stack visits the last source child first, intentionally.
            pending.extend(node.children(&mut cursor));
            if let Some(token) = LinkerChoice::from_node(node) {
                linkers.push(&token);
            }
        }
        let linkers = linkers.finish();
        assert_eq!(
            linkers.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["+<", "+,", "+,", "++", "+<", "+\""]
        );
        for linker in &linkers {
            let rendered = linker.to_string();
            assert_eq!(
                source.get(linker.span.start as usize..linker.span.end as usize),
                Some(rendered.as_str())
            );
        }
    }
}
