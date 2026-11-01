//! GEM headers retain source ownership through optional group projection.
//!
//! Each of `@Bg`, `@Eg` and `@G` is one [`GemKind`]; the model builds the
//! header from it. Displaced nodes at both group and header boundaries are
//! surfaced independently.

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BgHeaderNode, EgHeaderNode, GHeaderNode, SourceBound, SourceSlotView,
};
use crate::model::{GemKind, Header, PlainKind};
use crate::parser::tree_parsing::parser_helpers::{
    Body, ContentReadError, ContentSlot, HeaderSite, read_body, surface_displaced,
};
use talkbank_model::ParseOutcome;

// These three grammar rules share an optional (separator, optional
// free_text) group. The generated field API, not raw child indexes or text,
// owns that shape. Three outcomes, one per thing the author can have written:
// no separator at all is the legal bare marker (`GemKind::bare`); a written
// separator with no (or only blank) text is the one empty-content state,
// refused by validation (E553); a separator with text is the labelled gem,
// its label joined by the model's one joiner. A recovery node is never any
// of these: a MISSING label is recovery, and an ERROR in the label or group
// position is unparsable text, which the whole-tree backstop reports (E316)
// and which keeps the header as unknown with its text, as re2c does for the
// same line (in particular, never the legal bare marker). So is a recovery
// node displaced beside the label (`@G:\tsto` + NUL + `ry` parses `sto` as
// the label and the rest as an ERROR): the label is not what was written,
// so no labelled gem is built from the part before the ERROR, and an ERROR
// after a bare marker (`@Bg` and a NUL) is not the bare form. Those last two
// judgements are `HeaderSite::complete_body`'s, the one every header
// family's content takes (through `read_body` for the label).
macro_rules! gem_header {
    ($name:ident, $node:ident, $kind:expr, $context:literal) => {
        pub(super) fn $name<'tree>(
            typed: SourceBound<'tree, '_, $node<'tree>>,
            errors: &impl ErrorSink,
        ) -> ParseOutcome<Header> {
            let site = HeaderSite::bound(typed);
            let input = typed.source();
            let children = typed.extract();
            let words = ContentSlot {
                missing: concat!("Missing ", $context, " label"),
                suggested_fix: None,
            };
            let header = match children.field_child_1().slot().optional() {
                // The bare marker only when nothing else was written: an
                // ERROR after the marker (`@Bg` and a NUL) is unparsable
                // text, never the legal bare form.
                None => match site.complete_body(()) {
                    Ok(_) => ParseOutcome::Parsed($kind.bare()),
                    Err(failure) => failure.into_outcome(&site, errors),
                },
                Some(slot) => match slot.view() {
                    SourceSlotView::Present(group) => {
                        let header =
                            match read_body(&site, group.field_child_1().slot(), &words, errors) {
                                Ok(Body::Empty(separator)) => {
                                    ParseOutcome::Parsed(PlainKind::Gem($kind).empty(separator))
                                }
                                Ok(Body::Written { text, .. }) => {
                                    ParseOutcome::Parsed($kind.labelled(text))
                                }
                                Err(failure) => failure.into_outcome(&site, errors),
                            };
                        for displaced in group.field_unexpected().iter() {
                            surface_displaced(&[displaced.raw_node()], $context, input, errors);
                        }
                        header
                    }
                    SourceSlotView::Error(_) => {
                        ContentReadError::Unparsable.into_outcome(&site, errors)
                    }
                },
            };
            for displaced in children.field_unexpected().iter() {
                surface_displaced(&[displaced.raw_node()], $context, input, errors);
            }
            header
        }
    };
}

gem_header!(bg, BgHeaderNode, GemKind::Begin, "bg_header");
gem_header!(eg, EgHeaderNode, GemKind::End, "eg_header");
gem_header!(g, GHeaderNode, GemKind::Lazy, "g_header");
