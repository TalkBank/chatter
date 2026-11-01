//! A colon-bearing header with nothing after its colon, as one model state,
//! and the one decision that tells it apart from a header with content.
//!
//! A header such as `@Situation:` followed by its tab and then nothing says
//! "this header has content" and supplies none. Chatter does not support that
//! shape for any header: it is refused by validation (E553), always, with no
//! per-header policy. Modeling it as ONE state, decided by each parser before
//! any payload is built, is what lets every payload type below it be nonempty
//! by construction (`SituationDescription`, `ChatDate`, `ChatOptionFlags`,
//! ...) instead of each header carrying `""` as a stand-in for "no content"
//! and each validator catching it, or not, by accident.
//!
//! Three proofs carry that decision through the types:
//!
//! - [`HeaderContent::classify`] is the only way to obtain a [`HeaderText`],
//!   text with at least one non-whitespace character. Every text header
//!   payload is built from one (`from_content`), and every fallible route in
//!   (the payloads' `try_new`, `TryFrom` and JSON) goes through `classify`
//!   too. `@Comment`'s bullet content ([`CommentContent`](super::CommentContent))
//!   is admitted by classifying what it writes, by the parsers and JSON
//!   alike. So blank text can reach no payload.
//! - [`WrittenSeparator`] says the colon and its tab are there to write. The
//!   empty state carries one, and both parsers admit it only from the
//!   separator's own source text, so a recovery placeholder for a missing
//!   tab cannot become an empty header that writes a tab back. (What the
//!   type can and cannot prove about provenance is stated on it.)
//! - [`EmptyHeader`] names the header by its kind ([`PlainKind`] or
//!   [`QualifiedKind`] with its participant), so it shares the label table
//!   with every other header.
//!
//! Three colon-bearing headers are deliberately NOT here: `@Languages`,
//! `@Participants` and `@ID`. They declare the transcript's languages and
//! speakers, and their malformed forms (empty ones included) keep their own
//! declaration-admission states (`Header::UnparsedLanguages`,
//! `Header::UnparsedParticipants`, `Header::UnparsedID`) and codes. Folding
//! them into a generic state would make every later rule that needs the
//! declaration see it as absent, and report a spurious missing declaration
//! on top of the real defect. `@Thumbnail` is not here either: it has no
//! supported model at all, so every `@Thumbnail` line, empty or not, is
//! refused as unsupported (E525).
//!
//! Gem headers have a second, legal form with no colon at all (`@Bg`, `@Eg`,
//! `@G`), which is `Header::BeginGem { label: None }` and its siblings. Only
//! the colon form with nothing after the colon is empty content.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use super::kind::{HeaderKind, HeaderPrefix, PlainKind, QualifiedKind};
use crate::model::{NonEmptyStr, SpeakerCode, WriteChat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Header content proven to carry something: at least one character that is
/// not whitespace.
///
/// Only [`HeaderContent::classify`] makes one, so holding a `HeaderText` is
/// holding the outcome of the one emptiness decision. Every header payload
/// is built from it.
///
/// ```compile_fail
/// let forged = talkbank_model::model::HeaderText(" ");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderText<'a>(NonEmptyStr<'a>);

impl<'a> HeaderText<'a> {
    /// The content exactly as written.
    pub fn as_str(self) -> &'a str {
        self.0.as_str()
    }

    /// The same text as a nonempty proof: nonblank text is never empty.
    pub fn as_non_empty(self) -> NonEmptyStr<'a> {
        self.0
    }
}

/// The `parse_reason` both parsers record on the `Header::Unknown` they keep
/// for a header whose body cannot be read (malformed or unparsable content),
/// so the two backends' recoveries of the same line are equal by
/// construction rather than by a matching spelling in each.
pub const MALFORMED_HEADER_CONTENT: &str = "Malformed or unsupported header content";

/// Why text cannot be header content: it is empty or only whitespace.
///
/// One shared refusal for every header payload's fallible constructor and
/// JSON admission, matching what a parser makes of the same text (the empty
/// state, `Header::EmptyContent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("header content cannot be empty or only whitespace")]
pub struct BlankContent;

/// What a colon-bearing header carries after its separator: the one
/// emptiness decision, shared by both parsers and every payload constructor
/// so none of them can disagree.
///
/// A header carries nothing when no text follows the separator, or when all
/// that follows is whitespace (spaces, tabs, line breaks and continuation
/// tabs). Whitespace directly after the separator's tab is separator
/// provenance in both grammars (E758), and a header whose only content is
/// blank says no more than one with none; deciding on the text rather than on
/// the token or node shape is what keeps the two backends in agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderContent<'a> {
    /// Nothing but whitespace, or nothing at all, after the separator.
    Blank,
    /// The content text exactly as written, proven nonblank.
    Text(HeaderText<'a>),
}

impl<'a> HeaderContent<'a> {
    /// Classifies the text a header carries after its separator. Allocates
    /// nothing.
    pub fn classify(text: &'a str) -> Self {
        match NonEmptyStr::new(text) {
            Ok(text) if !text.as_str().chars().all(char::is_whitespace) => {
                Self::Text(HeaderText(text))
            }
            Ok(_) | Err(_) => Self::Blank,
        }
    }

    /// The compile-time form of [`Self::classify`], for a literal such as a
    /// redaction placeholder: admits the literal when it holds a printable
    /// ASCII character, which is never whitespace, so every literal it admits
    /// `classify` admits too. `const`, so a constant can hold the proof.
    ///
    /// # Errors
    ///
    /// [`BlankContent`] when `text` has no printable ASCII character.
    pub const fn classify_literal(text: &'static str) -> Result<HeaderText<'static>, BlankContent> {
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index].is_ascii_graphic() {
                return match NonEmptyStr::new(text) {
                    Ok(text) => Ok(HeaderText(text)),
                    Err(_) => Err(BlankContent),
                };
            }
            index += 1;
        }
        Err(BlankContent)
    }

    /// The text as payload content, refusing blank text: the admission every
    /// payload's fallible constructor and JSON route share.
    ///
    /// # Errors
    ///
    /// [`BlankContent`] when `text` is empty or only whitespace.
    pub fn admit(text: &'a str) -> Result<HeaderText<'a>, BlankContent> {
        match Self::classify(text) {
            Self::Text(text) => Ok(text),
            Self::Blank => Err(BlankContent),
        }
    }
}

/// A header line's separator, its colon and the tab after it, admitted from
/// separator text.
///
/// The empty state carries one, because its CHAT form writes the separator
/// back; a header whose colon has no tab (tree-sitter recovers it with a
/// MISSING tab, reported as E303) is malformed and is never empty content.
///
/// What this proves, stated exactly: a value exists only where some text
/// passed [`Self::admit`] (a colon, a tab, then only spaces), or where a JSON
/// empty header implied it. It does not prove that the text came from a
/// source file. `admit` is public because the two parsers live in their own
/// crates, so any caller can admit the literal `":\t"`. The guarantee that a
/// missing or recovered tab never becomes empty content rests on the parsers
/// admitting it from the separator node's or token's own source text, which
/// both do, and which their recovery tests pin.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WrittenSeparator(());

impl std::fmt::Debug for WrittenSeparator {
    /// The evidence has no payload; its name is the whole fact.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WrittenSeparator")
    }
}

impl WrittenSeparator {
    /// The separator every colon-bearing header writes: a colon and a tab.
    pub const CANONICAL: &'static str = ":\t";

    /// Admits a separator as written: a colon, a tab, and then only the
    /// spaces that E758 reports as separator provenance. Anything else (a
    /// colon alone, a recovered tab, a second tab) is no written separator.
    pub fn admit(text: &str) -> Option<Self> {
        text.strip_prefix(Self::CANONICAL)
            .filter(|trailing| trailing.bytes().all(|byte| byte == b' '))
            .map(|_| Self(()))
    }

    /// The separator a JSON empty header implies: its form is defined as the
    /// written header (`@Situation:` and a tab), so admitting the JSON is
    /// admitting a written separator.
    fn implied_by_wire() -> Self {
        Self(())
    }
}

/// Which header was written with nothing after its colon.
///
/// Two shapes, by kind: a header with no participant, and a
/// participant-qualified one, which keeps the participant it names because
/// that part of the line was written and must survive roundtrip. A qualified
/// empty header with no participant is not expressible. Each carries the
/// [`WrittenSeparator`] that made it.
///
/// Validation always refuses this state (E553). Its CHAT form is the header
/// prefix and separator alone, `@Situation:` followed by a tab. Its JSON form
/// is `{"header": "<kind>"}`, with `"participant"` for a qualified header.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(from = "EmptyHeaderWire", into = "EmptyHeaderWire")]
pub enum EmptyHeader {
    /// A header with no participant, written empty.
    Plain {
        /// Which header.
        kind: PlainKind,
        /// The separator that was written.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        separator: WrittenSeparator,
    },
    /// A participant-qualified header, written empty.
    Qualified {
        /// Which header.
        kind: QualifiedKind,
        /// The participant the header names.
        participant: SpeakerCode,
        /// The separator that was written.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        separator: WrittenSeparator,
    },
}

impl EmptyHeader {
    /// The kind of the header written empty: the same kind that header has
    /// with content.
    pub fn kind(&self) -> HeaderKind {
        match self {
            Self::Plain { kind, .. } => HeaderKind::Plain(*kind),
            Self::Qualified { kind, .. } => HeaderKind::Qualified(*kind),
        }
    }

    /// What the header writes before its separator, `@Situation` or
    /// `@Birth of CHI`.
    pub fn prefix(&self) -> HeaderPrefix<'_> {
        match self {
            Self::Plain { kind, .. } => HeaderPrefix::from(*kind),
            Self::Qualified {
                kind, participant, ..
            } => HeaderPrefix::qualified(*kind, participant),
        }
    }
}

/// The JSON form of [`EmptyHeader`]: two object shapes told apart by their
/// fields, refusing a participant on a plain header and an unknown field.
#[derive(Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum EmptyHeaderWire {
    Qualified {
        header: QualifiedKind,
        participant: SpeakerCode,
    },
    Plain {
        header: PlainKind,
    },
}

impl From<EmptyHeaderWire> for EmptyHeader {
    fn from(wire: EmptyHeaderWire) -> Self {
        let separator = WrittenSeparator::implied_by_wire();
        match wire {
            EmptyHeaderWire::Qualified {
                header,
                participant,
            } => Self::Qualified {
                kind: header,
                participant,
                separator,
            },
            EmptyHeaderWire::Plain { header } => Self::Plain {
                kind: header,
                separator,
            },
        }
    }
}

impl From<EmptyHeader> for EmptyHeaderWire {
    fn from(empty: EmptyHeader) -> Self {
        match empty {
            EmptyHeader::Qualified {
                kind, participant, ..
            } => Self::Qualified {
                header: kind,
                participant,
            },
            EmptyHeader::Plain { kind, .. } => Self::Plain { header: kind },
        }
    }
}

impl JsonSchema for EmptyHeader {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "EmptyHeader".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let plain = generator.subschema_for::<PlainKind>();
        let qualified = generator.subschema_for::<QualifiedKind>();
        let participant = generator.subschema_for::<SpeakerCode>();
        schemars::json_schema!({
            "description": "Which header was written with nothing after its colon. Validation always refuses this state (E553).",
            "oneOf": [
                {
                    "description": "A header with no participant, written empty.",
                    "type": "object",
                    "properties": { "header": plain },
                    "required": ["header"]
                },
                {
                    "description": "A participant-qualified header, written empty.",
                    "type": "object",
                    "properties": { "header": qualified, "participant": participant },
                    "required": ["header", "participant"]
                }
            ]
        })
    }
}

impl WriteChat for EmptyHeader {
    /// Writes the header exactly as it was given: its prefix and the
    /// separator, and nothing else.
    ///
    /// Both grammars admit exactly one separator spelling, a colon and one
    /// tab (trailing spaces are separator provenance reported by E758, not
    /// content), so this is byte-exact for every input that reaches it.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        self.prefix().write_start(w)
    }
}

#[cfg(test)]
mod tests {
    use super::super::kind::{GemKind, StructuredKind, TextKind};
    use super::*;
    use crate::model::Header;

    fn written() -> WrittenSeparator {
        WrittenSeparator::admit(":\t").expect("canonical separator")
    }

    /// Blank content is blank; any other character is content, kept exactly
    /// as written.
    #[test]
    fn header_content_classifies_blank_text_as_blank() {
        for blank in ["", " ", "   ", "\t", " \t ", "\n\t", "\r\n\t  "] {
            assert_eq!(
                HeaderContent::classify(blank),
                HeaderContent::Blank,
                "{blank:?}"
            );
            assert_eq!(HeaderContent::admit(blank), Err(BlankContent));
        }
        for text in ["x", " at home", "home ", "\n\tsecond"] {
            match HeaderContent::classify(text) {
                HeaderContent::Text(kept) => assert_eq!(kept.as_str(), text),
                HeaderContent::Blank => panic!("{text:?} is content"),
            }
        }
    }

    /// The compile-time admission is a strict subset of the runtime one:
    /// every ASCII string `classify_literal` admits, `classify` calls text,
    /// with the same text. Exhaustive over every string of up to two ASCII
    /// bytes, which covers each byte alone, each byte beside every other,
    /// and blank strings of each length.
    #[test]
    fn classify_literal_admits_only_what_classify_calls_text() {
        // Every candidate laid end to end in one buffer, leaked once so each
        // slice is `'static` like the literals `classify_literal` serves.
        let mut buffer = String::new();
        // The empty string first, then each byte alone and each pair.
        let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
        ranges.push(0..0);
        for first in 0_u8..=127 {
            let start = buffer.len();
            buffer.push(char::from(first));
            ranges.push(start..buffer.len());
            for second in 0_u8..=127 {
                let start = buffer.len();
                buffer.push(char::from(first));
                buffer.push(char::from(second));
                ranges.push(start..buffer.len());
            }
        }
        let buffer: &'static str = Box::leak(buffer.into_boxed_str());
        let mut admitted = 0;
        for range in ranges {
            let text = &buffer[range];
            if let Ok(literal) = HeaderContent::classify_literal(text) {
                admitted += 1;
                assert_eq!(
                    HeaderContent::classify(text),
                    HeaderContent::Text(literal),
                    "{text:?}"
                );
            }
        }
        // Both admitted and refused strings were reached.
        assert!(admitted > 0 && admitted < 1 + 128 + 128 * 128);
    }

    /// Only the colon, the tab and trailing spaces are a written separator.
    #[test]
    fn written_separator_admits_only_colon_tab_and_trailing_spaces() {
        for written in [":\t", ":\t ", ":\t   "] {
            assert!(WrittenSeparator::admit(written).is_some(), "{written:?}");
        }
        for unwritten in ["", ":", ": ", ":\t\t", "\t", ":\tx"] {
            assert!(
                WrittenSeparator::admit(unwritten).is_none(),
                "{unwritten:?}"
            );
        }
    }

    /// The empty form renders as the prefix and separator, and the wire
    /// carries the kind (and participant) under its own field.
    #[test]
    fn empty_header_renders_prefix_and_separator_and_roundtrips_json() {
        for (header, chat) in [
            (
                PlainKind::Text(TextKind::Situation).empty(written()),
                "@Situation:\t",
            ),
            (
                PlainKind::Text(TextKind::ColorWords).empty(written()),
                "@Color words:\t",
            ),
            (PlainKind::Text(TextKind::Pid).empty(written()), "@PID:\t"),
            (PlainKind::Gem(GemKind::Lazy).empty(written()), "@G:\t"),
            (
                PlainKind::Structured(StructuredKind::Options).empty(written()),
                "@Options:\t",
            ),
            (
                QualifiedKind::Birth.empty(SpeakerCode::new("CHI"), written()),
                "@Birth of CHI:\t",
            ),
            (
                QualifiedKind::L1Of.empty(SpeakerCode::new("MOT"), written()),
                "@L1 of MOT:\t",
            ),
        ] {
            assert_eq!(header.to_chat(), chat);
            let wire = serde_json::to_string(&header).expect("serialize");
            let restored: Header = serde_json::from_str(&wire).expect("deserialize");
            assert_eq!(restored, header);
        }
        let wire =
            serde_json::to_string(&QualifiedKind::Birth.empty(SpeakerCode::new("CHI"), written()))
                .expect("serialize");
        assert_eq!(
            wire,
            r#"{"type":"empty_content","header":"birth","participant":"CHI"}"#
        );
        let wire = serde_json::to_string(&PlainKind::Gem(GemKind::Begin).empty(written()))
            .expect("serialize");
        assert_eq!(wire, r#"{"type":"empty_content","header":"begin_gem"}"#);
    }

    /// The wire refuses the shapes the Rust type cannot hold: a qualified
    /// header without its participant, and a participant on a plain header.
    #[test]
    fn empty_header_json_refuses_mismatched_participant() {
        for wire in [
            r#"{"type":"empty_content","header":"birth"}"#,
            r#"{"type":"empty_content","header":"situation","participant":"CHI"}"#,
            r#"{"type":"empty_content","header":"languages"}"#,
        ] {
            assert!(serde_json::from_str::<Header>(wire).is_err(), "{wire}");
        }
    }
}
