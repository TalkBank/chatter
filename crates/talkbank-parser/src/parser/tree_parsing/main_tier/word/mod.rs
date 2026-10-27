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

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, ReadableSlot, SourceBound, StandaloneWordChild0Choice, StandaloneWordChild2Choice,
    StandaloneWordNode, WordBodyChoice, WordBodyNode,
};
use crate::model::Word;
use crate::parser::tree_parsing::parser_helpers::{collect_recovery_nodes, surface_displaced};
use crate::parser::typed_cst::{ExtractCanonical, ReportedCstFailure, report_admission};
use smallvec::SmallVec;
use talkbank_model::content::word::{
    FormType, WordCategory, WordContent, WordContents, WordPhonetic, WordText,
};
use talkbank_model::model::WriteChat;
use talkbank_model::model::{EmptyText, NonEmptyString};

mod language;
mod outcome;
mod pieces;
mod tokens;
use crate::parser::node_span::span_of;
pub(in crate::parser) use outcome::LoweredWord;
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
pub(in crate::parser) fn convert_word_node<'tree>(
    typed: SourceBound<'tree, '_, StandaloneWordNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<LoweredWord, ReportedCstFailure> {
    let source = typed.source();
    let node = typed.node().raw_node();
    if node.is_missing() {
        errors.report(ParseError::new(
            ErrorCode::MalformedWordContent,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, span_of(node), ""),
            format!(
                "Internal error: attempted to convert MISSING tree-sitter node at byte {}",
                node.start_byte()
            ),
        ));
        return Ok(LoweredWord::rejected());
    }

    let raw_text = typed.text();
    let span = span_of(node);

    let extraction = typed.extract_canonical();
    let bound_children = report_admission(extraction, node, source, errors)?;
    // The five root fields all participate in lowering. Admit their selected
    // ranges once, retaining the kind proof, rather than lowering a word with
    // a discarded optional field after its source read failed. Keep only displaced
    // recovery across failure; do not clone the complete selected carrier.
    let displaced = bound_children.children().unexpected.clone();
    let bound_children = match bound_children.admit_ranges() {
        Ok(admitted) => admitted,
        Err(failure) => {
            let reported = ReportedCstFailure::report(node, source, failure, errors);
            surface_displaced(&displaced, "standalone_word", source, errors);
            return Err(reported);
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
    let body_result = match &children.child_1.slot {
        ReadableSlot::Present(body) => build_word_contents(*body, errors, &mut content_items),
        ReadableSlot::Missing(missing) => match missing.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Error(_) | ReadableSlot::Absent(_) => Ok(()),
    };

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

    // A failed body projection is not an empty recovered body. Finish the
    // sibling diagnostics above, then refuse the entire word.
    body_result?;

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
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, span_of(node), ""),
                    "Unparsable content: word node produced empty text after parser recovery",
                )
                .with_suggestion(
                    "Check for unclosed brackets or missing word content near this position",
                ),
            );
            return Ok(LoweredWord::rejected());
        }
    };

    // Recovery may retain a prefix or suffix but no body. Keep that syntax
    // outside Word: copying the whole token into its body would duplicate the
    // prefix/suffix on writing and invent spoken material.
    if content_items.is_empty() {
        // Report this word's actual recovery locally as well as through the
        // whole-tree backstop (which deduplicates it). The owning main tier
        // must learn that source content was lost before model validation.
        let mut recovery = Vec::new();
        collect_recovery_nodes(node, source, &mut recovery);
        errors.report_all(recovery);
        return Ok(LoweredWord::incomplete(span, category));
    }

    let cleaned: String = content_items
        .iter()
        .filter_map(|item| match item {
            WordContent::Text(t) => Some(t.as_ref()),
            WordContent::Shortening(s) => Some(s.as_ref()),
            _ => None,
        })
        .collect();

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
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, span_of(node), ""),
                    format!(
                        "Word '{}' contains no spoken material after markers are stripped",
                        raw_text.as_str()
                    ),
                )
                .with_suggestion(
                    "Stress and lengthening markers must attach to actual spoken material",
                ),
            );
            return Ok(LoweredWord::rejected());
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

    let mut word = Word::new(span, cleaned);
    word.category = category;
    word.form_type = form_type;
    word.lang = lang;
    word.part_of_speech = part_of_speech;
    word = word.with_content(WordContents::new(content_items));

    Ok(LoweredWord::complete(word))
}

/// Fold a `@u` word's lexed content pieces into a single phonetic node.
///
/// Falls back to the original pieces if serialization yields nothing (a
/// content-free word already rejected upstream) so no information is ever
/// dropped.
fn fold_phonetic(content_items: SmallVec<[WordContent; 2]>) -> SmallVec<[WordContent; 2]> {
    let phonetic = talkbank_model::ChatString::render(|sink| {
        for item in &content_items {
            item.write_chat(sink)?;
        }
        Ok(())
    });
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
) -> Result<(), ReportedCstFailure> {
    let source = body.source();
    let children = report_admission(
        body.extract_canonical()
            .and_then(|children| children.admit_ranges().map_err(Into::into)),
        body.raw_node(),
        source,
        errors,
    )?;
    surface_displaced(&children.children().unexpected, "word_body", source, errors);
    let content = match &children.children().content.slot {
        ReadableSlot::Present(content) => content,
        ReadableSlot::Missing(never) => match *never {},
        ReadableSlot::Error(_) | ReadableSlot::Absent(_) => {
            return Ok(());
        }
        ReadableSlot::Unexpected(never) => match *never {},
    };
    let mut result = Ok(());
    match content {
        WordBodyChoice::WordSegment(segment_initial) => {
            surface_displaced(&segment_initial.unexpected, "word_body", source, errors);
            result = result.and(push_slot(&segment_initial.child_0.slot, errors, items));
            for positioned in &segment_initial.child_1.slot {
                result = result.and(push_slot(&positioned.slot, errors, items));
            }
        }
        WordBodyChoice::OverlapPoint(marker_initial) => {
            surface_displaced(&marker_initial.unexpected, "word_body", source, errors);
            result = result.and(push_slot(&marker_initial.child_0.slot, errors, items));
            for positioned in &marker_initial.child_1.slot {
                result = result.and(push_slot(&positioned.slot, errors, items));
            }
            result = result.and(push_slot(&marker_initial.child_2.slot, errors, items));
            for positioned in &marker_initial.child_3.slot {
                result = result.and(push_slot(&positioned.slot, errors, items));
            }
        }
    }
    result
}
