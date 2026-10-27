//! Linker decoding through generated first/repeated groups.
//!
//! CHAT reference: <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>

use crate::CstFailure;
use crate::error::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, Span,
};
use crate::generated_traversal::{
    AsRawNode, KindSlot, LinkerChoice, LinkerChoiceBoundView, LinkersChild0Child0Choice,
    LinkersChild1Child0Choice, LinkersNode, NoChild, Ranged, ReadableSlot, SlotView, SourceBound,
    SourceSlice, WhitespacesNode,
};
use crate::model::{Linker, LinkerKind};
use crate::parser::node_span::span_of;

/// The model kind and source span come from the same generated alternative.
trait LinkerToken {
    fn to_located_linker(&self) -> LocatedLinker;
}

/// A parser token keeps its mandatory span until source ordering is complete.
/// The broader model's unlocated state cannot enter the ordering buffer.
struct LocatedLinker {
    span: Span,
    kind: LinkerKind,
}

macro_rules! located_linker {
    ($choice:ident, $value:expr) => {
        match $value {
            $choice::LinkerLazyOverlap(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::LazyOverlapPrecedes,
            },
            $choice::LinkerQuickUptake(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::OtherCompletion,
            },
            $choice::LinkerQuickUptakeOverlap(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::QuickUptakeOverlap,
            },
            $choice::LinkerQuotationFollows(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::QuotationFollows,
            },
            $choice::LinkerSelfCompletion(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::SelfCompletion,
            },
            $choice::CaTechnicalBreakLinker(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::TcuContinuation,
            },
            $choice::CaNoBreakLinker(node) => LocatedLinker {
                span: span_of(node.raw_node()),
                kind: LinkerKind::NoBreakTcuContinuation,
            },
        }
    };
}

impl<'tree, 'source> LinkerToken for LinkerChoiceBoundView<'tree, 'source> {
    fn to_located_linker(&self) -> LocatedLinker {
        located_linker!(LinkerChoiceBoundView, self)
    }
}

impl<'tree, 'source> LinkerToken for LinkersChild0Child0Choice<'tree, Ranged<'source>> {
    fn to_located_linker(&self) -> LocatedLinker {
        located_linker!(LinkersChild0Child0Choice, self)
    }
}

impl<'tree, 'source> LinkerToken for LinkersChild1Child0Choice<'tree, Ranged<'source>> {
    fn to_located_linker(&self) -> LocatedLinker {
        located_linker!(LinkersChild1Child0Choice, self)
    }
}

/// Source order is independent of when a generated sink exposes a token.
/// Only typed linker tokens enter; their model kind and span stay paired.
struct SourceOrderedLinkers(Vec<LocatedLinker>);

impl SourceOrderedLinkers {
    fn push(&mut self, token: &impl LinkerToken) {
        let linker = token.to_located_linker();
        // Normal traversal appends in constant time. Only displaced recovery
        // can require insertion among already-read tokens.
        if self
            .0
            .last()
            .is_none_or(|last| last.span.start() <= linker.span.start())
        {
            self.0.push(linker);
        } else {
            let position = self
                .0
                .partition_point(|prior| prior.span.start() <= linker.span.start());
            self.0.insert(position, linker);
        }
    }

    fn finish(self) -> Vec<Linker> {
        self.0
            .into_iter()
            .map(|linker| Linker::new(linker.span, linker.kind))
            .collect()
    }
}

/// The current grammar exposes concrete linker kinds, not a legacy linker
/// wrapper. Required/repeated groups retain their recovery states and sinks.
pub(super) fn parse_linkers<'tree>(
    typed: SourceBound<'tree, '_, LinkersNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<Linker>, CstFailure> {
    let admitted = typed.extract().admit_ranges()?;
    let children = admitted.children();
    let mut linkers = SourceOrderedLinkers(Vec::new());

    // First and repeated groups have distinct generated carrier types but the
    // same field shape. Expansion keeps both enum matches compiler-exhaustive.
    macro_rules! group {
        ($slot:expr) => {
            match $slot.view() {
                SlotView::Present(group) => {
                    push_linker(&group.child_0.slot, errors, &mut linkers);
                    match group.child_1.slot.view() {
                        SlotView::Present(_) | SlotView::Missing(_) | SlotView::Absent(NoChild) => {
                        }
                        SlotView::Error(node) => report_unexpected(node, errors),
                    }
                    for node in &group.unexpected {
                        push_displaced(*node, errors, &mut linkers);
                    }
                }
                SlotView::Error(node) => report_unexpected(node, errors),
                SlotView::Absent(_) => {}
            }
        };
    }
    group!(&children.child_0.slot);
    for element in &children.child_1.slot {
        group!(&element.slot);
    }
    for node in &children.unexpected {
        push_displaced(*node, errors, &mut linkers);
    }
    Ok(linkers.finish())
}

fn push_linker<'tree, 'source, T: LinkerToken>(
    slot: &KindSlot<'tree, T, Ranged<'source>>,
    errors: &impl ErrorSink,
    linkers: &mut SourceOrderedLinkers,
) {
    match slot {
        // Preserve lexical placeholders and their actual zero-width spans;
        // the whole-tree backstop owns the missing-token diagnostic.
        ReadableSlot::Present(token) | ReadableSlot::Missing(token) => linkers.push(token),
        ReadableSlot::Error(node) => push_displaced(*node, errors, linkers),
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(NoChild) => {}
    }
}

/// Classify only this generated recovery boundary, retaining its source owner.
fn push_displaced(
    node: SourceSlice<'_, '_>,
    errors: &impl ErrorSink,
    linkers: &mut SourceOrderedLinkers,
) {
    if let Some(token) = node.typed::<LinkerChoice>() {
        linkers.push(&token.view());
    } else if node.typed::<WhitespacesNode>().is_none() {
        report_unexpected(node, errors);
    }
}

fn report_unexpected(node: SourceSlice<'_, '_>, errors: &impl ErrorSink) {
    let source = node.source();
    let node = node.raw_node();
    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        DiagnosticLocation::new(span_of(node)),
        ErrorContext::new(source, span_of(node), ""),
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
        let mut pending = vec![parsed.root().expect("source-bound root")];
        let mut linkers = SourceOrderedLinkers(Vec::new());
        while let Some(node) = pending.pop() {
            let mut cursor = node.raw_node().walk();
            // A stack visits the last source child first, intentionally.
            pending.extend(
                node.children(&mut cursor)
                    .map(|child| child.expect("bound child")),
            );
            if let Some(token) = node.typed::<LinkerChoice>() {
                linkers.push(&token.view());
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
                source.get(
                    linker.location.span().expect("located").start()
                        ..linker.location.span().expect("located").end()
                ),
                Some(rendered.as_str())
            );
        }
    }
}
