//! Lowers generated bullet-text choices in source order, retaining recovery.
use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BulletNode, ChoiceSlot, ContinuationNode, InlinePicNode, KindSlot, NoChild,
    Positioned, SourceBound, SourceBoundKind, SourceField, SourceSlotView, SpaceNode,
    TextSegmentNode, TextWithBulletsAndPicsChild0Choice,
    TextWithBulletsAndPicsChild0ChoiceSourceView, TextWithBulletsAndPicsChild1Choice,
    TextWithBulletsAndPicsChild1ChoiceSourceView, TextWithBulletsChild0Choice,
    TextWithBulletsChild0ChoiceSourceView, TextWithBulletsChild1Choice,
    TextWithBulletsChild1ChoiceSourceView,
};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::{read_source_field, report_source_binding_error};
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{BulletContent, BulletContentSegment};
use tree_sitter::Node;

use super::{BulletTextNode, inline_bullet::parse_inline_bullet, inline_pic::parse_inline_pic};

/// Parse only one of the grammar's two bullet-text carriers.
///
/// The generated choices own text/bullet/picture dispatch and each nested
/// group's trailing spaces. Recovery slots and displaced sinks remain visible;
/// a typed carrier does not certify that its children are free of recovery.
pub fn parse_bullet_content(
    typed: BulletTextNode<'_, '_>,
    errors: &impl ErrorSink,
) -> BulletContent {
    let mut sink = SegmentSink {
        errors,
        segments: SmallVec::new(),
    };
    match typed {
        BulletTextNode::Text(node) => {
            let children = node.extract();
            sink.choice(children.field_child_0().slot(), push_text_first);
            for item in children.field_child_1().slot().iter() {
                sink.choice(item.slot(), push_text_repeat);
            }
            sink.displaced(children.field_unexpected(), "text_with_bullets");
        }
        BulletTextNode::Pictures(node) => {
            let children = node.extract();
            sink.choice(children.field_child_0().slot(), push_pictures_first);
            for item in children.field_child_1().slot().iter() {
                sink.choice(item.slot(), push_pictures_repeat);
            }
            sink.displaced(children.field_unexpected(), "text_with_bullets_and_pics");
        }
    }
    BulletContent::new(sink.segments.into_vec())
}

/// Owns the source-order segment accumulation; no intermediate flattened AST.
struct SegmentSink<'errors, E> {
    errors: &'errors E,
    segments: SmallVec<[BulletContentSegment; 4]>,
}

impl<E: ErrorSink> SegmentSink<'_, E> {
    fn text<'tree>(&mut self, typed: SourceBound<'tree, '_, TextSegmentNode<'tree>>) {
        let text = normalize_free_text_spacing(typed.text());
        if !text.is_empty() {
            self.segments.push(BulletContentSegment::text(text));
        }
    }

    fn bullet<'tree>(&mut self, typed: SourceBound<'tree, '_, BulletNode<'tree>>) {
        if let ParseOutcome::Parsed((start, end)) = parse_inline_bullet(typed, self.errors) {
            self.segments.push(BulletContentSegment::bullet(start, end));
        }
    }

    fn picture<'tree>(&mut self, typed: SourceBound<'tree, '_, InlinePicNode<'tree>>) {
        if let ParseOutcome::Parsed(filename) = parse_inline_pic(typed, self.errors) {
            self.segments.push(BulletContentSegment::picture(filename));
        }
    }

    fn unexpected<'tree>(&self, node: SourceField<'_, 'tree, '_, Node<'tree>>) {
        self.errors.report(unexpected_node_error(
            node.raw_node(),
            node.source(),
            "bullet_content",
        ));
    }

    fn displaced<'tree>(&self, nodes: SourceField<'_, 'tree, '_, Vec<Node<'tree>>>, parent: &str) {
        for node in nodes.iter() {
            let raw = node.raw_node();
            surface_displaced(
                std::slice::from_ref(&raw),
                parent,
                node.source(),
                self.errors,
            );
        }
    }

    /// Missing leaves retain their expected kind. Preserve the old lowering of
    /// these placeholders; the whole-tree backstop still diagnoses recovery.
    fn placeholder<'tree>(&mut self, field: SourceField<'_, 'tree, '_, Node<'tree>>) {
        let node = match field.read_raw() {
            Ok(node) => node,
            Err(error) => {
                report_source_binding_error(field.raw_node(), field.source(), error, self.errors);
                return;
            }
        };
        if let Some(text) = node.typed::<TextSegmentNode>() {
            self.text(text);
        } else if let Some(bullet) = node.typed::<BulletNode>() {
            self.bullet(bullet);
        } else if let Some(picture) = node.typed::<InlinePicNode>() {
            self.picture(picture);
        } else if node.typed::<ContinuationNode>().is_some() {
            self.segments.push(BulletContentSegment::continuation());
        } else if node.typed::<SpaceNode>().is_none() {
            self.unexpected(field);
        }
    }

    fn choice<'value, 'tree: 'value, 'source, T: 'value>(
        &mut self,
        slot: SourceField<'value, 'tree, 'source, ChoiceSlot<'tree, T>>,
        push: impl FnOnce(SourceField<'value, 'tree, 'source, T>, &mut Self),
    ) {
        match slot.view() {
            SourceSlotView::Present(choice) => push(choice, self),
            SourceSlotView::Missing(node) => self.placeholder(node),
            SourceSlotView::Error(node) | SourceSlotView::Unexpected(node) => self.unexpected(node),
            SourceSlotView::Absent(NoChild) => {}
        }
    }

    fn leaf<'tree, 'source, T: SourceBoundKind<'tree>>(
        &mut self,
        slot: SourceField<'_, 'tree, 'source, KindSlot<'tree, T>>,
        push: impl FnOnce(&mut Self, SourceBound<'tree, 'source, T>),
    ) {
        match slot.view() {
            SourceSlotView::Present(node) | SourceSlotView::Missing(node) => {
                if let Some(node) = read_source_field(node, self.errors) {
                    push(self, node);
                }
            }
            SourceSlotView::Error(node) => self.unexpected(node),
            SourceSlotView::Absent(NoChild) => {}
        }
    }

    fn spaces<'tree>(
        &self,
        spaces: SourceField<
            '_,
            'tree,
            '_,
            Vec<Positioned<'tree, KindSlot<'tree, SpaceNode<'tree>>>>,
        >,
        unexpected: SourceField<'_, 'tree, '_, Vec<Node<'tree>>>,
    ) {
        for space in spaces.iter() {
            match space.slot().view() {
                SourceSlotView::Present(_)
                | SourceSlotView::Missing(_)
                | SourceSlotView::Absent(NoChild) => {}
                SourceSlotView::Error(node) => self.unexpected(node),
            }
        }
        self.displaced(unexpected, "bullet_content");
    }
}

// The generator gives first/repeat positions distinct carrier types. Keep
// their exhaustive alternatives identical without erasing either into raw nodes.
macro_rules! text_choices {
    ($function:ident, $choice:ident, $view:ident $(, $picture:ident)?) => {
        fn $function<'tree, E: ErrorSink>(choice: SourceField<'_, 'tree, '_, $choice<'tree>>, sink: &mut SegmentSink<'_, E>) {
            match choice.view() {
                $view::TextSegment(text) => {
                    if let Some(text) = read_source_field(text, sink.errors) {
                        sink.text(text);
                    }
                }
                $view::Bullet(group) => {
                    sink.leaf(group.field_child_0().slot(), SegmentSink::bullet);
                    sink.spaces(group.field_child_1().slot(), group.field_unexpected());
                }
                $view::Continuation(_) => sink.segments.push(BulletContentSegment::continuation()),
                $(
                    $view::$picture(group) => {
                        sink.leaf(group.field_child_0().slot(), SegmentSink::picture);
                        sink.spaces(group.field_child_1().slot(), group.field_unexpected());
                    }
                )?
            }
        }
    };
}

text_choices!(
    push_text_first,
    TextWithBulletsChild0Choice,
    TextWithBulletsChild0ChoiceSourceView
);
text_choices!(
    push_text_repeat,
    TextWithBulletsChild1Choice,
    TextWithBulletsChild1ChoiceSourceView
);
text_choices!(
    push_pictures_first,
    TextWithBulletsAndPicsChild0Choice,
    TextWithBulletsAndPicsChild0ChoiceSourceView,
    InlinePic
);
text_choices!(
    push_pictures_repeat,
    TextWithBulletsAndPicsChild1Choice,
    TextWithBulletsAndPicsChild1ChoiceSourceView,
    InlinePic
);

/// Collapse runs of ASCII spaces to a single space and trim leading/trailing
/// spaces from one free-text run.
///
/// In a free-text dependent tier, spaces are word delimiters, not content
/// (the corpus shows deliberate multi-spaces do not occur: multi-space runs
/// are 0.1% and are typos or structured-coding artifacts). Canonicalizing to
/// single spaces here makes the model match the main tier, which stores no
/// spaces and re-inserts single delimiters on serialization. Only spaces are
/// collapsed; any tab or other character inside the run is preserved.
fn normalize_free_text_spacing(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch == ' ' {
            // Defer emitting a space until a non-space follows, so runs
            // collapse and a trailing run is dropped.
            prev_space = true;
            continue;
        }
        if prev_space && !out.is_empty() {
            out.push(' ');
        }
        prev_space = false;
        out.push(ch);
    }
    out
}
