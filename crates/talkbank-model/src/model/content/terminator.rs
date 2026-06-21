//! Utterance terminator tokens.
//!
//! Terminators encode sentence-final punctuation and interruption/completion
//! state.
//!
//! # CHAT Format References
//!
//! - [Terminators](https://talkbank.org/0info/manuals/CHAT.html#Terminators)
//! - [CA Intonation](https://talkbank.org/0info/manuals/CHAT.html#CA_Intonation)

use super::WriteChat;
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// End-of-utterance token variant.
///
/// # Standard Terminators
///
/// - `.` - Period (declarative)
/// - `?` - Question mark (interrogative)
/// - `!` - Exclamation (imperative/exclamatory)
///
/// # Interruption Terminators
///
/// - `+...` - Trailing off (incomplete thought)
/// - `+/.` - Interruption by another speaker
/// - `+//.` - Self-interruption
/// - `+/?` - Interrupted question
/// - `+//?` - Self-interrupted question
/// - `+/??` - Broken off question
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want that .         Standard declarative
/// *MOT: what do you want ?    Question
/// *CHI: look at this !        Exclamation
/// *CHI: I was going to +...   Trailing off
/// *MOT: did you +/. yes I did Interrupted by CHI
/// *CHI: um the +//. the dog   Self-interruption
/// ```
///
/// # References
///
/// - [Terminators](https://talkbank.org/0info/manuals/CHAT.html#Terminators)
/// - [Interruption Terminator](https://talkbank.org/0info/manuals/CHAT.html#Interruption_Terminator)
/// - [CA Intonation](https://talkbank.org/0info/manuals/CHAT.html#CA_Intonation)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Terminator {
    /// Period `.` - declarative statement
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Period_Terminator>
    Period {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// Question mark `?` - interrogative
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuestionMark_Terminator>
    Question {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// Exclamation `!` - imperative/exclamatory
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#ExclamationMark_Terminator>
    Exclamation {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +... - trailing off (incomplete utterance)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TrailingOff_Terminator>
    TrailingOff {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +/. - interruption (interrupted by another speaker)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Interruption_Terminator>
    Interruption {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +//. - self-interruption
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Self_Interruption_Terminator>
    SelfInterruption {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +/? - interrupted question
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Interrupted_Question_Terminator>
    InterruptedQuestion {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +!? - broken-off question
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#BrokenQuestion_Terminator>
    BrokenQuestion {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +"/. - SUTNL - quoted utterance, next line (quote-slash-period)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
    #[serde(rename = "quoted_new_line")]
    QuotedNewLine {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +". - SUTQP - quoted utterance with period (quote-period, no slash)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#QuotedPeriod_Terminator>
    #[serde(rename = "quoted_period_simple")]
    QuotedPeriodSimple {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +//? - self-interrupted question
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SelfInterruptedQuestion_Terminator>
    #[serde(rename = "self_interrupted_question")]
    SelfInterruptedQuestion {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +..? - trailing off question
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#TrailingOffQuestion_Terminator>
    #[serde(rename = "trailing_off_question")]
    TrailingOffQuestion {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
    /// +. - break for coding
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#BreakForCoding>
    #[serde(rename = "break_for_coding")]
    BreakForCoding {
        #[serde(skip)]
        #[schemars(skip)]
        #[semantic_eq(skip)]
        /// Source span for error reporting.
        span: Span,
    },
}

// NOTE: CA-prosody arrows (⇗ ↗ → ↘ ⇘) and CA TCU markers (≋ +≋ ≈ +≈) are
// modeled as ``Separator`` variants, not as ``Terminator`` variants. The
// grammar dispatches them to ``non_colon_separator``; the parser builds
// ``Separator::Level``, ``Separator::RisingToMid``, etc. CHECK accepts
// them anywhere on the main tier; they are NOT utterance-final tokens.
// The previous ``Terminator::Ca*`` variants were the bug behind BUG-009
// (the morphotag pipeline propagated ⇗↗→↘⇘ into ``%mor`` because they
// were typed as terminators); they were retired once the parser was
// fixed to dispatch them as separators.

impl Terminator {
    /// Parse the canonical CHAT terminator string into its typed variant.
    ///
    /// Accepts exactly the strings that [`WriteChat::write_chat`] emits for
    /// each variant (see the CHAT manual
    /// <https://talkbank.org/0info/manuals/CHAT.html#Terminators> for the
    /// supported inventory. Returns `None` for any other input, including
    /// content punctuation like `,` `;` `:`, CA separators/linkers, and any
    /// non-terminator text.
    /// Spans are set to [`Span::DUMMY`] since the caller has no source
    /// location for a terminator recovered from a free string.
    ///
    /// This is the round-trip partner of [`WriteChat::write_chat`]. Useful
    /// for classifying untyped tokens (e.g., UD `PUNCT` words returned by
    /// a morphotag pipeline) without stringly-typed pattern matching at the
    /// call site.
    pub fn try_from_chat_str(s: &str) -> Option<Self> {
        let span = Span::DUMMY;
        let t = match s {
            "." => Self::Period { span },
            "?" => Self::Question { span },
            "!" => Self::Exclamation { span },
            "+..." => Self::TrailingOff { span },
            "+/." => Self::Interruption { span },
            "+//." => Self::SelfInterruption { span },
            "+/?" => Self::InterruptedQuestion { span },
            "+!?" => Self::BrokenQuestion { span },
            "+\"/." => Self::QuotedNewLine { span },
            "+\"." => Self::QuotedPeriodSimple { span },
            "+//?" => Self::SelfInterruptedQuestion { span },
            "+..?" => Self::TrailingOffQuestion { span },
            "+." => Self::BreakForCoding { span },
            _ => return None,
        };
        Some(t)
    }

    /// Whether the given string is a recognized CHAT utterance terminator.
    ///
    /// Thin helper over [`Terminator::try_from_chat_str`] for the common
    /// callsite pattern "does this string terminate an utterance?".
    /// Returns `false` for content punctuation (`,`, `;`, `:`, etc.).
    pub fn is_chat_terminator(s: &str) -> bool {
        Self::try_from_chat_str(s).is_some()
    }

    /// Returns source span metadata associated with this terminator.
    pub fn span(&self) -> Span {
        match self {
            Terminator::Period { span }
            | Terminator::Question { span }
            | Terminator::Exclamation { span }
            | Terminator::TrailingOff { span }
            | Terminator::Interruption { span }
            | Terminator::SelfInterruption { span }
            | Terminator::InterruptedQuestion { span }
            | Terminator::BrokenQuestion { span }
            | Terminator::QuotedNewLine { span }
            | Terminator::QuotedPeriodSimple { span }
            | Terminator::SelfInterruptedQuestion { span }
            | Terminator::TrailingOffQuestion { span }
            | Terminator::BreakForCoding { span } => *span,
        }
    }

    /// Replace the stored source span while preserving the terminator kind.
    pub fn with_span(self, span: Span) -> Self {
        match self {
            Terminator::Period { .. } => Terminator::Period { span },
            Terminator::Question { .. } => Terminator::Question { span },
            Terminator::Exclamation { .. } => Terminator::Exclamation { span },
            Terminator::TrailingOff { .. } => Terminator::TrailingOff { span },
            Terminator::Interruption { .. } => Terminator::Interruption { span },
            Terminator::SelfInterruption { .. } => Terminator::SelfInterruption { span },
            Terminator::InterruptedQuestion { .. } => Terminator::InterruptedQuestion { span },
            Terminator::BrokenQuestion { .. } => Terminator::BrokenQuestion { span },
            Terminator::QuotedNewLine { .. } => Terminator::QuotedNewLine { span },
            Terminator::QuotedPeriodSimple { .. } => Terminator::QuotedPeriodSimple { span },
            Terminator::SelfInterruptedQuestion { .. } => {
                Terminator::SelfInterruptedQuestion { span }
            }
            Terminator::TrailingOffQuestion { .. } => Terminator::TrailingOffQuestion { span },
            Terminator::BreakForCoding { .. } => Terminator::BreakForCoding { span },
        }
    }
}

impl WriteChat for Terminator {
    /// Serializes the canonical CHAT token for this terminator.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            Terminator::Period { .. } => w.write_char('.'),
            Terminator::Question { .. } => w.write_char('?'),
            Terminator::Exclamation { .. } => w.write_char('!'),
            Terminator::TrailingOff { .. } => w.write_str("+..."),
            Terminator::Interruption { .. } => w.write_str("+/."),
            Terminator::SelfInterruption { .. } => w.write_str("+//."),
            Terminator::InterruptedQuestion { .. } => w.write_str("+/?"),
            Terminator::BrokenQuestion { .. } => w.write_str("+!?"),
            Terminator::QuotedNewLine { .. } => w.write_str("+\"/."),
            Terminator::QuotedPeriodSimple { .. } => w.write_str("+\"."),
            Terminator::SelfInterruptedQuestion { .. } => w.write_str("+//?"),
            Terminator::TrailingOffQuestion { .. } => w.write_str("+..?"),
            Terminator::BreakForCoding { .. } => w.write_str("+."),
        }
    }
}

impl std::fmt::Display for Terminator {
    /// Formats the exact CHAT token for the current terminator variant.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip every supported variant through `Display` + `try_from_chat_str`.
    ///
    /// Legacy CA-only variants are intentionally excluded here because the
    /// parser/classifier no longer treats their surface forms as terminators.
    #[test]
    fn supported_variants_round_trip_display_to_try_from_chat_str() {
        let span = Span::DUMMY;
        let all = [
            Terminator::Period { span },
            Terminator::Question { span },
            Terminator::Exclamation { span },
            Terminator::TrailingOff { span },
            Terminator::Interruption { span },
            Terminator::SelfInterruption { span },
            Terminator::InterruptedQuestion { span },
            Terminator::BrokenQuestion { span },
            Terminator::QuotedNewLine { span },
            Terminator::QuotedPeriodSimple { span },
            Terminator::SelfInterruptedQuestion { span },
            Terminator::TrailingOffQuestion { span },
            Terminator::BreakForCoding { span },
        ];
        for t in all {
            let emitted = t.to_string();
            let parsed = Terminator::try_from_chat_str(&emitted)
                .unwrap_or_else(|| panic!("{emitted:?} did not parse back to a Terminator"));
            // The parsed variant uses DUMMY span; compare only the kind via
            // re-emission, which is what callers actually key off of.
            assert_eq!(
                parsed.to_string(),
                emitted,
                "round trip mismatch on {emitted:?}"
            );
        }
    }

    /// Content punctuation (comma, semicolon, colon) must NOT parse as a
    /// terminator. Regression guard: without this discrimination, every
    /// CHAT comma would be silently treated as a terminator by callers
    /// that classify UD `PUNCT` tokens.
    #[test]
    fn content_punct_is_not_a_chat_terminator() {
        for s in [
            ",", ";", ":", "-", "\"", "'", "(", ")", "[", "]", "„", "‡", "&", "%",
        ] {
            assert!(
                Terminator::try_from_chat_str(s).is_none(),
                "content punct {s:?} must not parse as a terminator"
            );
            assert!(
                !Terminator::is_chat_terminator(s),
                "is_chat_terminator({s:?}) must be false"
            );
        }
    }

    /// Arbitrary text must never parse as a terminator.
    #[test]
    fn words_are_not_terminators() {
        for s in ["hello", "the", "", " ", "...", "ab", "+not_a_term"] {
            assert!(
                !Terminator::is_chat_terminator(s),
                "{s:?} must not be classified as a terminator"
            );
        }
    }

    /// Whitespace does not accidentally match.
    #[test]
    fn trailing_whitespace_not_accepted() {
        // Callers must trim before calling; this ensures we don't accept
        // `". "` (with trailing space) as a terminator silently.
        assert!(Terminator::try_from_chat_str(". ").is_none());
        assert!(Terminator::try_from_chat_str(" .").is_none());
    }

    /// CA arrows, CA TCU separators, and CA TCU linker forms are not CHAT
    /// terminators; they are modeled elsewhere.
    #[test]
    fn ca_symbols_are_not_chat_terminators() {
        for s in ["⇗", "↗", "→", "↘", "⇘", "≈", "≋", "+≈", "+≋"] {
            assert!(
                Terminator::try_from_chat_str(s).is_none(),
                "{s:?} must not parse as a terminator"
            );
            assert!(
                !Terminator::is_chat_terminator(s),
                "{s:?} must not be classified as a terminator"
            );
        }
    }
}
