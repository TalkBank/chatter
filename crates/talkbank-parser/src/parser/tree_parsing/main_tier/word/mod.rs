//! Word parsing from tree-sitter CST, direct CST traversal.
//!
//! The grammar parses word internals structurally:
//!   standalone_word = [word_prefix] word_body [form_marker] [word_lang_suffix] [pos_tag]
//!   word_body = repeat1(word_segment | shortening | stress_marker | lengthening | '+')
//!
//! This module walks the CST children to build the typed `Word` model
//! without any Chumsky dependency.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, ReadableSlot, SourceBound, SourceSlotView, StandaloneWordChild0Choice,
    StandaloneWordChild2Choice, StandaloneWordNode, WordBodyChoiceSourceView, WordBodyNode,
};
use crate::model::Word;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::content::word::{
    FormType, WordCategory, WordContent, WordContents, WordPhonetic, WordText,
};
use talkbank_model::model::WriteChat;
use talkbank_model::model::{EmptyText, NonEmptyString};

mod language;
mod pieces;
mod tokens;
use pieces::push_slot;

/// Convert a tree-sitter `standalone_word` node into the typed `Word` model.
///
/// Grammar: `seq(optional(choice(word_prefix, zero)), word_body,
/// optional(choice(form_marker, repeated_form_marker)),
/// optional(word_lang_suffix), optional(pos_tag))`. `extract_standalone_word`
/// places the five in typed slots; the body's pieces go through
/// [`build_word_contents`]. Until 2026-09-09 this walked the children by
/// `node.kind()` string, with a silent arm for anything else.
///
/// Content, replacement, %wor and fragment callers retain the producer-bound
/// word and its admitted source text. Missing placeholders still reject;
/// source ownership and a typed kind are not syntax validity. Body and piece
/// projections, prefix/suffix and inline CA conversions retain ownership too.
pub fn convert_word_node<'tree>(
    typed: SourceBound<'tree, '_, StandaloneWordNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Word> {
    let source = typed.source();
    let node = typed.node().raw_node();
    if node.is_missing() {
        errors.report(ParseError::new(
            ErrorCode::MalformedWordContent,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!(
                "Internal error: attempted to convert MISSING tree-sitter node at byte {}",
                node.start_byte()
            ),
        ));
        return ParseOutcome::rejected();
    }

    let raw_text = typed.text();
    let span = talkbank_model::Span::from_usize(node.start_byte(), node.end_byte());

    let extraction = crate::parser::typed_cst::canonical_grammar()
        .and_then(|grammar| typed.extract_admitted(grammar).map_err(Into::into));
    let Ok(bound_children) =
        crate::parser::typed_cst::report_admission(extraction, node, source, errors)
    else {
        return ParseOutcome::Rejected;
    };
    // The five root fields all participate in lowering. Admit their selected
    // ranges once, retaining the kind proof, rather than lowering a word with
    // a discarded optional field after its source read failed. Keep only displaced
    // recovery across failure; do not clone the complete selected carrier.
    let displaced = bound_children.children().unexpected.clone();
    let bound_children = match bound_children.admit_ranges() {
        Ok(admitted) => admitted,
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(node, source, failure, errors);
            surface_displaced(&displaced, "standalone_word", source, errors);
            return ParseOutcome::Rejected;
        }
    };
    drop(displaced);
    let children = bound_children.children();

    // Position 0: a category prefix (`&-`, `&~`, `&+`) or the omission
    // `zero`, which the grammar inlines here rather than through
    // `word_prefix` to resolve a shift-reduce conflict with `nonword`.
    let category = children.child_0.slot.as_ref().and_then(|slot| match slot {
        ReadableSlot::Present(choice) => match choice {
            StandaloneWordChild0Choice::WordPrefix(prefix) => {
                tokens::word_category(*prefix, errors)
            }
            StandaloneWordChild0Choice::Zero(_) => Some(WordCategory::Omission),
        },
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) => None,
    });

    // Position 1: the body, required.
    let mut content_items: SmallVec<[WordContent; 2]> = SmallVec::new();
    match &children.child_1.slot {
        ReadableSlot::Present(body) => build_word_contents(*body, errors, &mut content_items),
        ReadableSlot::Missing(missing) => {
            crate::parser::tree_parsing::parser_helpers::check_not_missing(
                missing.raw_node(),
                missing.source(),
                errors,
                "standalone_word",
            );
        }
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Error(_) | ReadableSlot::Absent(_) => {}
    }

    // Position 2: one `@` suffix run, or the node the grammar builds for a
    // word carrying more than one.
    let form_type = children.child_2.slot.as_ref().and_then(|slot| match slot {
        ReadableSlot::Present(choice) => match choice {
            StandaloneWordChild2Choice::FormMarker(marker) => tokens::form_marker(*marker, errors),
            StandaloneWordChild2Choice::RepeatedFormMarker(marker) => {
                tokens::repeated_form_marker(*marker, errors)
            }
        },
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) => None,
    });

    // Position 3 is an opaque token. Keep its admitted source ownership
    // through spelling admission and atomic candidate conversion.
    let lang = children.child_3.slot.as_ref().and_then(|slot| match slot {
        ReadableSlot::Present(suffix) => {
            language::parse_language_suffix(*suffix, errors).into_option()
        }
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) => None,
    });

    // Position 4: `$tag`.
    let part_of_speech = children.child_4.slot.as_ref().and_then(|slot| match slot {
        ReadableSlot::Present(tag) => tokens::part_of_speech(*tag, errors),
        ReadableSlot::Unexpected(never) | ReadableSlot::Absent(never) => match *never {},
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) => None,
    });

    // Whatever filled no position, after the positions, so that the
    // diagnostics keep the order the old child walk emitted them in.
    for displaced in &children.unexpected {
        surface_displaced(
            std::slice::from_ref(&displaced.raw_node()),
            "standalone_word",
            displaced.source(),
            errors,
        );
    }

    // If no content items were collected, use raw_text as a single Text item
    if content_items.is_empty()
        && let Ok(wt) = WordText::new(raw_text)
    {
        content_items.push(WordContent::Text(wt));
    }

    // Compute cleaned_text from content items (Text + Shortening only)
    let cleaned: String = content_items
        .iter()
        .filter_map(|item| match item {
            WordContent::Text(t) => Some(t.as_ref()),
            WordContent::Shortening(s) => Some(s.as_ref()),
            _ => None,
        })
        .collect();

    // Parser recovery can produce a word node with empty text (from
    // `[: unclosed`, for instance). Source admission proves a readable slice,
    // not that it contains any text. The word's source text is proven
    // non-empty here, where the diagnostic can name the node; the
    // constructor takes the proof.
    let raw_text = match NonEmptyString::new(raw_text) {
        Ok(raw_text) => raw_text,
        Err(EmptyText) => {
            errors.report(
                ParseError::new(
                    ErrorCode::UnparsableContent,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                    "Unparsable content: word node produced empty text after parser recovery",
                )
                .with_suggestion(
                    "Check for unclosed brackets or missing word content near this position",
                ),
            );
            return ParseOutcome::rejected();
        }
    };

    // A word whose cleaned text is empty (only a stress marker `ˈ`, or only a
    // lengthening marker `:`) has no spoken material: E245 (stress marker not
    // before spoken material), and the word is rejected rather than built
    // around an empty text. The proof is built here for the same reason as
    // the source text's; the constructor cannot be handed an empty one.
    let cleaned = match WordText::new(&cleaned) {
        Ok(cleaned) => cleaned,
        Err(EmptyText) => {
            errors.report(
                ParseError::new(
                    ErrorCode::StressNotBeforeSpokenMaterial,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                    format!(
                        "Word '{}' contains no spoken material after markers are stripped",
                        raw_text.as_str()
                    ),
                )
                .with_suggestion(
                    "Stress and lengthening markers must attach to actual spoken material",
                ),
            );
            return ParseOutcome::rejected();
        }
    };

    // A `@u` word's content is a PHONETIC transcription (UNIBET/IPA), not
    // orthography: fold the lexed pieces back into one opaque phonetic
    // node so orthographic word rules structurally cannot apply to it
    // (option B of the 2026-07-13 UNIBET design; scope is @u ONLY per the
    // 2026-07-14 adjudication). The fold serializes each piece's CHAT
    // surface form, so round-tripping is lossless.
    let content_items = if matches!(form_type, Some(FormType::U)) {
        fold_phonetic(content_items)
    } else {
        content_items
    };

    let mut word = Word::new(cleaned);
    word.span = span;
    word.category = category;
    word.form_type = form_type;
    word.lang = lang;
    word.part_of_speech = part_of_speech;
    word = word.with_content(WordContents::new(content_items));

    ParseOutcome::parsed(word)
}

/// Fold a `@u` word's lexed content pieces into a single phonetic node.
///
/// Falls back to the original pieces if serialization yields nothing (a
/// content-free word already rejected upstream) so no information is ever
/// dropped.
fn fold_phonetic(content_items: SmallVec<[WordContent; 2]>) -> SmallVec<[WordContent; 2]> {
    let mut phonetic = String::new();
    for item in &content_items {
        if item.write_chat(&mut phonetic).is_err() {
            // Writing into a String cannot fail; treat a failure as "do
            // not fold" rather than losing content.
            return content_items;
        }
    }
    match WordPhonetic::new(&phonetic) {
        Ok(form) => smallvec::smallvec![WordContent::Phonetic(form)],
        Err(_) => content_items,
    }
}

/// Build `WordContent` items from a typed `word_body` node.
///
/// Grammar: `choice(seq(choice(word_segment, shortening, stress_marker),
/// repeat(...)), seq(<marker-initial> ...))`. Each position's choice is
/// lowered to a [`Piece`] and converted once by [`push_piece`]; the two
/// unexpected sinks are surfaced, where the old walk's `_ =>` arm skipped
/// whatever it did not name.
fn build_word_contents<'tree>(
    body: SourceBound<'tree, '_, WordBodyNode<'tree>>,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) {
    let source = body.source();
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::canonical_grammar()
            .and_then(|grammar| body.extract_admitted(grammar).map_err(Into::into)),
        body.raw_node(),
        source,
        errors,
    ) else {
        return;
    };
    surface_displaced(&children.children().unexpected, "word_body", source, errors);
    let content = match children.field_content().slot().view() {
        SourceSlotView::Present(content) => content,
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => return,
        SourceSlotView::Unexpected(never) => match never {},
    };
    match content.view() {
        WordBodyChoiceSourceView::WordSegment(segment_initial) => {
            for bad in segment_initial.field_unexpected().iter() {
                surface_displaced(&[bad.raw_node()], "word_body", source, errors);
            }
            push_slot(segment_initial.field_child_0().slot(), errors, items);
            for positioned in segment_initial.field_child_1().slot().iter() {
                push_slot(positioned.slot(), errors, items);
            }
        }
        WordBodyChoiceSourceView::OverlapPoint(marker_initial) => {
            for bad in marker_initial.field_unexpected().iter() {
                surface_displaced(&[bad.raw_node()], "word_body", source, errors);
            }
            push_slot(marker_initial.field_child_0().slot(), errors, items);
            for positioned in marker_initial.field_child_1().slot().iter() {
                push_slot(positioned.slot(), errors, items);
            }
            push_slot(marker_initial.field_child_2().slot(), errors, items);
            for positioned in marker_initial.field_child_3().slot().iter() {
                push_slot(positioned.slot(), errors, items);
            }
        }
    }
}
