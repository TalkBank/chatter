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

use super::content::report_tree_shape;
use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, NodeSlot, RecoveryNode, StandaloneWordChild0Choice,
    StandaloneWordChild2Choice, StandaloneWordNode, WordBodyChoice, WordBodyNode,
    extract_standalone_word, extract_word_body,
};
use crate::model::Word;
use crate::parser::tree_parsing::parser_helpers::{
    after_marker, extract_utf8_text, surface_displaced,
};
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::content::word::{
    FormMarkerPayload, FormType, WordCategory, WordContent, WordContents, WordLanguageMarker,
    WordPhonetic, WordText,
};
use talkbank_model::model::WriteChat;
use talkbank_model::model::{EmptyText, LanguageCode, NonEmptyString};
use tree_sitter::Node;

mod pieces;
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
/// Callers hand over the raw node (the content converter, the replacement
/// parser, the `%wor` tier and the fragment API all hold one), so the typing
/// happens here; a node that is not a `standalone_word` is refused with a
/// diagnostic rather than walked.
pub fn convert_word_node(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<Word> {
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
    let Some(typed) = StandaloneWordNode::from_node(node) else {
        report_tree_shape(
            node,
            format!("Expected a standalone_word node, found '{}'", node.kind()),
            source,
            errors,
        );
        return ParseOutcome::rejected();
    };

    let raw_text = extract_utf8_text(node, source, errors, "standalone_word", "");
    let span = talkbank_model::Span::from_usize(node.start_byte(), node.end_byte());

    let children = extract_standalone_word(typed);

    // Position 0: a category prefix (`&-`, `&~`, `&+`) or the omission
    // `zero`, which the grammar inlines here rather than through
    // `word_prefix` to resolve a shift-reduce conflict with `nonword`.
    let category = children
        .child_0
        .slot()
        .as_ref()
        .and_then(|slot| taken(slot, "word prefix", "standalone_word", source, errors))
        .and_then(|choice| match choice {
            StandaloneWordChild0Choice::WordPrefix(prefix) => {
                word_category(prefix.raw_node(), source, errors)
            }
            StandaloneWordChild0Choice::Zero(_) => Some(WordCategory::Omission),
        });

    // Position 1: the body, required.
    let mut content_items: SmallVec<[WordContent; 2]> = SmallVec::new();
    if let Some(body) = taken(
        children.child_1.slot(),
        "word_body",
        "standalone_word",
        source,
        errors,
    ) {
        build_word_contents(*body, source, errors, &mut content_items);
    }

    // Position 2: one `@` suffix run, or the node the grammar builds for a
    // word carrying more than one.
    let form_type = children
        .child_2
        .slot()
        .as_ref()
        .and_then(|slot| taken(slot, "form marker", "standalone_word", source, errors))
        .and_then(|choice| match choice {
            StandaloneWordChild2Choice::FormMarker(marker) => {
                form_marker(marker.raw_node(), source, errors)
            }
            StandaloneWordChild2Choice::RepeatedFormMarker(marker) => {
                repeated_form_marker(marker.raw_node(), source, errors)
            }
        });

    // Position 3: `@s`, `@s:eng`, `@s:eng+zho`, `@s:eng&zho`.
    let lang = children
        .child_3
        .slot()
        .as_ref()
        .and_then(|slot| taken(slot, "language suffix", "standalone_word", source, errors))
        .and_then(|suffix| build_lang_marker(suffix.raw_node(), source, errors));

    // Position 4: `$tag`.
    let part_of_speech = children
        .child_4
        .slot()
        .as_ref()
        .and_then(|slot| {
            taken(
                slot,
                "part-of-speech tag",
                "standalone_word",
                source,
                errors,
            )
        })
        .and_then(|tag| {
            after_marker(tag.raw_node(), "$", "Part-of-speech tag", source, errors)
                .map(smol_str::SmolStr::from)
        });

    // Whatever filled no position, after the positions, so that the
    // diagnostics keep the order the old child walk emitted them in.
    surface_displaced(&children.unexpected, "standalone_word", source, errors);

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
    // `[: unclosed`, for instance), and a node whose text does not decode
    // arrives here as the empty fallback. The word's source text is proven
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

    let mut word = Word::new(raw_text, cleaned);
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
/// An E203 anchored on one `@` suffix token.
///
/// The two arms that report it differ only in their message and suggestion;
/// the code, severity, location and context were spelled out twice, with
/// `child.start_byte()`/`end_byte()` appearing four times between them.
fn at_suffix_error(
    child: Node,
    source: &str,
    message: String,
    suggestion: &'static str,
) -> ParseError {
    ParseError::new(
        ErrorCode::InvalidFormType,
        Severity::Error,
        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
        message,
    )
    .with_suggestion(suggestion)
}

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

/// The node a typed word position holds, or `None` when it holds none.
///
/// A MISSING placeholder and an ERROR node are both the whole-tree pass's
/// to report: it names a stray token inside a word with its classified code
/// (E375 for a bracket glued to the word, E316 otherwise), and a local
/// "lost its shape" report here would outrank that by span overlap and
/// replace the specific code with E330, which is what the first draft of
/// this migration did to `E375.md#2`. A displaced node is the one state the
/// backstop cannot see, so that is reported here. An optional position that
/// is empty holds nothing.
///
/// `what` names the position and `within` the node it belongs to, so the
/// diagnostic reads "Expected shortening content in shortening" rather than
/// naming the word for a position three levels down.
pub(super) fn taken<'a, 'tree, T, M, U, A>(
    slot: &'a NodeSlot<'tree, T, M, U, A>,
    what: &str,
    within: &str,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<&'a T>
where
    U: RecoveryNode<'tree>,
{
    match slot {
        NodeSlot::Present(value) => Some(value),
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Absent(_) => None,
        NodeSlot::Unexpected(bad) => {
            report_tree_shape(
                bad.node(),
                format!("Expected {what} in {within}, found '{}'", bad.node().kind()),
                source,
                errors,
            );
            None
        }
    }
}

/// The category a `word_prefix` token names. The token is one of three
/// spellings by grammar; any other text is a shape fault, reported rather
/// than read as "no category".
fn word_category(node: Node, source: &str, errors: &impl ErrorSink) -> Option<WordCategory> {
    let text = extract_utf8_text(node, source, errors, "word_prefix", "");
    match text {
        "&-" => Some(WordCategory::Filler),
        "&~" => Some(WordCategory::Nonword),
        "&+" => Some(WordCategory::PhonologicalFragment),
        other => {
            report_tree_shape(
                node,
                format!("Unknown word prefix {other:?}"),
                source,
                errors,
            );
            None
        }
    }
}

/// The form type a `form_marker` token declares.
///
/// The grammar captures the whole marker as ONE token, `@` and any `:label`
/// included, and deliberately tokenizes shapes it does not sanction (`@zz`,
/// `@x:foo`) so they can be named here rather than degrading into generic
/// unparsable content. Which codes exist, and which of them take a `:label`,
/// is the registry's business, not this function's: `@z` requires a label and
/// `@x` refuses one, and both facts arrive through `from_payload`. A token
/// without its `@` does not fit its own grammar and declares no form
/// (reported by [`after_marker`]); until 2026-09-09 it was read as a bare
/// payload, so a bare declared code would have parsed as a marker.
fn form_marker(node: Node, source: &str, errors: &impl ErrorSink) -> Option<FormType> {
    // The `@` is stripped HERE rather than by the payload type, because the
    // guarantee that it is present is the GRAMMAR's, and this is the only
    // code that reads a grammar token.
    let payload = after_marker(node, "@", "Form marker", source, errors)?;
    Some(
        match FormType::from_payload(FormMarkerPayload::after_at(payload)) {
            Ok(declared) => declared,
            Err(undeclared) => {
                // CLAN CHECK 147, "undeclared special form marker".
                errors.report(at_suffix_error(
                    node,
                    source,
                    format!("Undeclared form marker '@{}'", undeclared.payload()),
                    undeclared.suggestion(),
                ));
                // Record what was WRITTEN, not a marker that happens to be
                // declared. This used to store `UserDefined(payload)`, which
                // claimed `word@zz` was `@z` with label `zz` and would serialize
                // back as `word@z:zz`. Storing it also keeps the model's own E203
                // check quiet, so the specific diagnostic above is not duplicated
                // by the generic one.
                FormType::Undeclared(undeclared.into_payload())
            }
        },
    )
}

/// A word carrying MORE THAN ONE `@` suffix run. The rule and the ruling
/// behind it live in `spec/errors/E203.md`; two things are decided HERE and
/// belong here.
///
/// THE MESSAGE NAMES THE RUN, not a marker. `@c` in `@c@s:spa` is perfectly
/// real, so `Undeclared form marker '@c@s:spa'` would send a transcriber
/// hunting for a marker that does not exist instead of deleting one of the
/// two that do. The node KIND carries this, which is the structural answer
/// `check_inline_at_markers` says the model could not previously see.
///
/// THE TEXT IS STORED VERBATIM, so the word serializes back byte for byte
/// and `normalize` refuses the file rather than rewriting it. A token with
/// no leading `@` is REFUSED rather than absorbed, unlike [`form_marker`]:
/// the payload is stored and `write_chat` puts the `@` back, so absorbing a
/// sigil-less text would emit an `@` that was never in the source. The
/// grammar cannot produce this node without one, so the branch is
/// unreachable and says so rather than inventing a value.
fn repeated_form_marker(node: Node, source: &str, errors: &impl ErrorSink) -> Option<FormType> {
    let text = extract_utf8_text(node, source, errors, "repeated_form_marker", "");
    let Some(payload) = text.strip_prefix('@') else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                source,
                node.start_byte()..node.end_byte(),
                "repeated_form_marker",
            ),
            format!("repeated form marker token has no leading '@': {text:?}"),
        ));
        return None;
    };
    errors.report(at_suffix_error(
        node,
        source,
        format!("A word may carry only one '@' suffix, found '{text}'"),
        "Keep one marker on the word and remove the rest",
    ));
    Some(FormType::Undeclared(payload.to_owned()))
}

/// Build `WordContent` items from a typed `word_body` node.
///
/// Grammar: `choice(seq(choice(word_segment, shortening, stress_marker),
/// repeat(...)), seq(<marker-initial> ...))`. Each position's choice is
/// lowered to a [`Piece`] and converted once by [`push_piece`]; the two
/// unexpected sinks are surfaced, where the old walk's `_ =>` arm skipped
/// whatever it did not name.
fn build_word_contents(
    body: WordBodyNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) {
    let children = extract_word_body(body);
    surface_displaced(&children.unexpected, "word_body", source, errors);
    let Some(content) = taken(
        children.content.slot(),
        "word body content",
        "word_body",
        source,
        errors,
    ) else {
        return;
    };
    match content {
        WordBodyChoice::WordSegment(segment_initial) => {
            surface_displaced(&segment_initial.unexpected, "word_body", source, errors);
            push_slot(segment_initial.child_0.slot(), source, errors, items);
            for positioned in segment_initial.child_1.slot() {
                push_slot(positioned.slot(), source, errors, items);
            }
        }
        WordBodyChoice::OverlapPoint(marker_initial) => {
            surface_displaced(&marker_initial.unexpected, "word_body", source, errors);
            push_slot(marker_initial.child_0.slot(), source, errors, items);
            for positioned in marker_initial.child_1.slot() {
                push_slot(positioned.slot(), source, errors, items);
            }
            push_slot(marker_initial.child_2.slot(), source, errors, items);
            for positioned in marker_initial.child_3.slot() {
                push_slot(positioned.slot(), source, errors, items);
            }
        }
    }
}

/// Build a `WordLanguageMarker` from a `word_lang_suffix` token text.
///
/// The token is a single `token.immediate` matching `@s(?::[a-z]{2,3}(?:[+&][a-z]{2,3})*)?`.
/// Examples: `@s`, `@s:eng`, `@s:eng+zho+fra`, `@s:eng&zho&fra`. A token
/// without its `@s` does not fit its own grammar and is no marker (reported
/// by [`after_marker`]); until 2026-09-09 it was read as the bare `@s`
/// shortcut.
fn build_lang_marker(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<WordLanguageMarker> {
    let after_at_s = after_marker(node, "@s", "Language suffix", source, errors)?;

    // No colon: the bare `@s` shortcut.
    let Some(codes_str) = after_at_s.strip_prefix(':') else {
        return Some(WordLanguageMarker::Shortcut);
    };

    // Check for & separator (ambiguous) vs + separator (multiple). Every
    // split segment is guaranteed non-empty by the `word_lang_suffix` token
    // regex (`[a-z]{2,3}` per `+`/`&`-separated segment, see the doc
    // comment above); `LanguageCode::new` rejects an empty segment fallibly,
    // and the (token-regex-impossible) `Err` arms below stay panic-free and
    // non-silent per the no-panic / no-silent-drop policy: list forms skip
    // the offending segment via `filter_map`, the single-code form reports a
    // diagnostic and recovers with the documented `LanguageCode::empty()`
    // ("und") parser-recovery sentinel.
    if codes_str.contains('&') {
        let codes: Vec<LanguageCode> = codes_str
            .split('&')
            .filter_map(|c| LanguageCode::new(c).ok())
            .collect();
        Some(WordLanguageMarker::Ambiguous(codes))
    } else if codes_str.contains('+') {
        let codes: Vec<LanguageCode> = codes_str
            .split('+')
            .filter_map(|c| LanguageCode::new(c).ok())
            .collect();
        Some(WordLanguageMarker::Multiple(codes))
    } else {
        // Single code
        Some(match LanguageCode::new(codes_str) {
            Ok(code) => WordLanguageMarker::Explicit(code),
            Err(error) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(
                        source,
                        node.start_byte()..node.end_byte(),
                        "word_lang_suffix",
                    ),
                    format!("Invalid @s language code: {error}"),
                ));
                WordLanguageMarker::Explicit(LanguageCode::empty())
            }
        })
    }
}
