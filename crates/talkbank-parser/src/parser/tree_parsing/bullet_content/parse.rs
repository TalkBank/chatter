//! Lowers generated bullet-text choices in source order, retaining recovery.
use crate::error::ErrorSink;
use crate::generated_traversal::{Absence, Never, NodeSlot};
use crate::generated_traversal::{
    AsRawNode, BulletNode, InlinePicNode, Positioned, SourceBound, SourceBoundKind, SourceField,
    SourceRecovery, SourceSlotView, SpaceNode, TextSegmentNode, TextWithBulletsAndPicsChild0Choice,
    TextWithBulletsAndPicsChild0ChoiceSourceView, TextWithBulletsAndPicsChild0MissingSourceView,
    TextWithBulletsAndPicsChild1Choice, TextWithBulletsAndPicsChild1ChoiceSourceView,
    TextWithBulletsAndPicsChild1MissingSourceView, TextWithBulletsChild0Choice,
    TextWithBulletsChild0ChoiceSourceView, TextWithBulletsChild0MissingSourceView,
    TextWithBulletsChild1Choice, TextWithBulletsChild1ChoiceSourceView,
    TextWithBulletsChild1MissingSourceView,
};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::AnyKindSlot;
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
) -> Result<BulletContent, crate::CstFailure> {
    let mut sink = SegmentSink {
        errors,
        segments: SmallVec::new(),
    };
    match typed {
        BulletTextNode::Text(node) => {
            let children = node.extract();
            sink.choice(children.field_child_0().slot(), push_text_first)?;
            for item in children.field_child_1().slot().iter() {
                sink.choice(item.slot(), push_text_repeat)?;
            }
            sink.displaced(children.field_unexpected(), "text_with_bullets");
        }
        BulletTextNode::Pictures(node) => {
            let children = node.extract();
            sink.choice(children.field_child_0().slot(), push_pictures_first)?;
            for item in children.field_child_1().slot().iter() {
                sink.choice(item.slot(), push_pictures_repeat)?;
            }
            sink.displaced(children.field_unexpected(), "text_with_bullets_and_pics");
        }
    }
    Ok(BulletContent::new(sink.segments.into_vec()))
}

/// A bullet-text position's MISSING placeholder, by kind. tree-sitter inserts
/// a MISSING node only for a terminal, and a choice owns a MISSING head only
/// of a single-node alternative's kind; for all four bullet-text positions the
/// generator found exactly these two. Until 2026-10-20 the placeholder was a
/// raw node re-read as text, bullet, picture, continuation or space, with an
/// "unexpected node" fallback; a MISSING picture or space is now matched at
/// its own alternative's position instead, and a bullet is a rule tree-sitter
/// never inserts MISSING.
enum Placeholder<'value, 'tree, 'source> {
    /// A MISSING text segment: its text is empty.
    TextSegment(SourceField<'value, 'tree, 'source, TextSegmentNode<'tree>>),
    /// A MISSING continuation.
    Continuation,
}

/// A bullet-text position's generated missing view, read as a [`Placeholder`].
/// One exhaustive conversion per position, so a kind added to a position's
/// heads by a grammar change fails to compile here.
trait IntoPlaceholder<'value, 'tree, 'source> {
    fn into_placeholder(self) -> Placeholder<'value, 'tree, 'source>;
}

impl<'value, 'tree, 'source> IntoPlaceholder<'value, 'tree, 'source>
    for TextWithBulletsChild0MissingSourceView<'value, 'tree, 'source>
{
    fn into_placeholder(self) -> Placeholder<'value, 'tree, 'source> {
        match self {
            Self::TextSegment(text) => Placeholder::TextSegment(text),
            Self::Continuation(_) => Placeholder::Continuation,
        }
    }
}

impl<'value, 'tree, 'source> IntoPlaceholder<'value, 'tree, 'source>
    for TextWithBulletsChild1MissingSourceView<'value, 'tree, 'source>
{
    fn into_placeholder(self) -> Placeholder<'value, 'tree, 'source> {
        match self {
            Self::TextSegment(text) => Placeholder::TextSegment(text),
            Self::Continuation(_) => Placeholder::Continuation,
        }
    }
}

impl<'value, 'tree, 'source> IntoPlaceholder<'value, 'tree, 'source>
    for TextWithBulletsAndPicsChild0MissingSourceView<'value, 'tree, 'source>
{
    fn into_placeholder(self) -> Placeholder<'value, 'tree, 'source> {
        match self {
            Self::TextSegment(text) => Placeholder::TextSegment(text),
            Self::Continuation(_) => Placeholder::Continuation,
        }
    }
}

impl<'value, 'tree, 'source> IntoPlaceholder<'value, 'tree, 'source>
    for TextWithBulletsAndPicsChild1MissingSourceView<'value, 'tree, 'source>
{
    fn into_placeholder(self) -> Placeholder<'value, 'tree, 'source> {
        match self {
            Self::TextSegment(text) => Placeholder::TextSegment(text),
            Self::Continuation(_) => Placeholder::Continuation,
        }
    }
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

    fn bullet<'tree>(
        &mut self,
        typed: SourceBound<'tree, '_, BulletNode<'tree>>,
    ) -> Result<(), crate::CstFailure> {
        if let ParseOutcome::Parsed((start, end)) = parse_inline_bullet(typed, self.errors)? {
            self.segments.push(BulletContentSegment::bullet(start, end));
        }
        Ok(())
    }

    fn picture<'tree>(&mut self, typed: SourceBound<'tree, '_, InlinePicNode<'tree>>) {
        if let ParseOutcome::Parsed(filename) = parse_inline_pic(typed, self.errors) {
            self.segments.push(filename.into_segment());
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

    /// A MISSING placeholder keeps its kind: an empty text segment (which
    /// pushes nothing) or a continuation. The whole-tree backstop diagnoses
    /// the recovery node. The continuation still becomes a model segment, as
    /// it always did; `@Comment:\t` with an empty body reaches it. That is a
    /// model value built for a recovery node, recorded with the coverage
    /// drive to be decided from a spec example, not changed here.
    fn placeholder(
        &mut self,
        placeholder: Placeholder<'_, '_, '_>,
    ) -> Result<(), crate::CstFailure> {
        match placeholder {
            Placeholder::TextSegment(text) => self.text(text.read()?),
            Placeholder::Continuation => self.segments.push(BulletContentSegment::continuation()),
        }
        Ok(())
    }

    fn choice<'value, 'tree: 'value, 'source, T: 'value, M, A: Absence>(
        &mut self,
        slot: SourceField<'value, 'tree, 'source, NodeSlot<T, M, Node<'tree>, Never, A>>,
        push: impl FnOnce(
            SourceField<'value, 'tree, 'source, T>,
            &mut Self,
        ) -> Result<(), crate::CstFailure>,
    ) -> Result<(), crate::CstFailure>
    where
        M: SourceRecovery<'value, 'tree, 'source>,
        M::View: IntoPlaceholder<'value, 'tree, 'source>,
    {
        match slot.view() {
            SourceSlotView::Present(choice) => push(choice, self)?,
            SourceSlotView::Missing(view) => self.placeholder(view.into_placeholder())?,
            SourceSlotView::Error(node) => self.unexpected(node),
            SourceSlotView::Unexpected(never) => match never {},
            SourceSlotView::Absent(_) => {}
        }
        Ok(())
    }

    /// Push a leaf slot's node, reading a MISSING placeholder of the leaf's
    /// kind as that kind (its text is empty). Generic over the `Missing`
    /// payload, bounded by exactly that reading: a placeholder whose source
    /// view is a field of `T`, which a plain kind slot and a narrowed one read
    /// under the broad kind proof both provide.
    fn leaf<'value, 'tree: 'value, 'source, T: SourceBoundKind<'tree>, M, Recovery>(
        &mut self,
        slot: SourceField<'value, 'tree, 'source, AnyKindSlot<T, M, Recovery>>,
        push: impl FnOnce(&mut Self, SourceBound<'tree, 'source, T>) -> Result<(), crate::CstFailure>,
    ) -> Result<(), crate::CstFailure>
    where
        M: SourceRecovery<'value, 'tree, 'source, View = SourceField<'value, 'tree, 'source, T>>,
        Recovery: SourceRecovery<
                'value,
                'tree,
                'source,
                View = SourceField<'value, 'tree, 'source, tree_sitter::Node<'tree>>,
            >,
    {
        match slot.view() {
            SourceSlotView::Present(node) | SourceSlotView::Missing(node) => {
                push(self, node.read()?)?;
            }
            SourceSlotView::Error(node) => self.unexpected(node),
            SourceSlotView::Absent(_) => {}
        }
        Ok(())
    }

    fn spaces<'tree>(
        &self,
        spaces: SourceField<
            '_,
            'tree,
            '_,
            Vec<
                Positioned<
                    'tree,
                    crate::generated_traversal::SelectedKindSlot<'tree, SpaceNode<'tree>>,
                >,
            >,
        >,
        unexpected: SourceField<'_, 'tree, '_, Vec<Node<'tree>>>,
    ) {
        for space in spaces.iter() {
            match space.slot().view() {
                SourceSlotView::Present(_) | SourceSlotView::Missing(_) => {}
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
        fn $function<'tree, E: ErrorSink>(choice: SourceField<'_, 'tree, '_, $choice<'tree>>, sink: &mut SegmentSink<'_, E>) -> Result<(), crate::CstFailure> {
            match choice.view() {
                $view::TextSegment(text) => {
                    sink.text(text.read()?);
                }
                $view::Bullet(group) => {
                    sink.leaf(group.field_child_0().slot(), SegmentSink::bullet)?;
                    sink.spaces(group.field_child_1().slot(), group.field_unexpected());
                }
                $view::Continuation(_) => sink.segments.push(BulletContentSegment::continuation()),
                $(
                    $view::$picture(group) => {
                        sink.leaf(group.field_child_0().slot(), |sink, picture| {
                            sink.picture(picture);
                            Ok(())
                        })?;
                        sink.spaces(group.field_child_1().slot(), group.field_unexpected());
                    }
                )?
            }
            Ok(())
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
