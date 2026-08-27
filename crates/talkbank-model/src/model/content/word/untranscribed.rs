//! Untranscribed-material markers (`xxx`, `yyy`, `www`) for word tokens.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Untranscribed_Material>
//! - <https://talkbank.org/0info/manuals/CHAT.html#UntranscribedMaterial_Code>

use crate::model::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Classification for untranscribed-word marker tokens.
///
/// Each variant maps directly to one canonical CHAT marker (`xxx`, `yyy`, `www`).
///
/// # CHAT Format Examples
///
/// ```text
/// xxx               Unintelligible speech
/// yyy               Requires phonetic transcription
/// www               Deliberately untranscribed
/// ```
///
/// # Usage Context
///
/// These markers appear as the word text itself when speech cannot be
/// transcribed using standard orthography:
///
/// ```text
/// *CHI: I want xxx .           Child says something unintelligible
/// *MOT: did you say yyy ?      Requires phonetic analysis
/// *CHI: www and then we left   Deliberately not transcribed
/// ```
///
/// # References
///
/// - [Untranscribed Material](https://talkbank.org/0info/manuals/CHAT.html#Untranscribed_Material)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(rename_all = "lowercase")]
pub enum UntranscribedStatus {
    /// Unintelligible speech (`xxx`).
    Unintelligible,

    /// Requires phonetic transcription (`yyy`).
    Phonetic,

    /// Deliberately untranscribed (`www`).
    Untranscribed,
}

/// The two spellings of one marker: the one CHAT requires, and the shortened
/// one it rejects.
///
/// # Why both live in one value
///
/// They used to be two independent lists, and the lists disagreed. The
/// canonical forms were a `match` over this enum, which the compiler checks for
/// completeness; the rejected short forms were a `match` over STRINGS, which it
/// cannot. So `xx` and `yy` were rejected and `ww` was not, from before anyone
/// now working on this could say when, with no reason recorded because there
/// was none. An external integrator eventually reported one edge of it.
///
/// Returning both from a single arm makes that state unwritable: a marker
/// cannot be given a canonical spelling and left without a shortened one,
/// because there is nowhere to put the omission.
struct Spellings {
    /// What CHAT requires: the marker's letter, three times, lowercase.
    canonical: &'static str,
    /// The letter twice, which CHAT rejects and which E241 offers to repair.
    shortened: &'static str,
}

/// How a token is spelled, relative to the untranscribed-marker vocabulary.
///
/// A total classification, so a caller must say what it does about every case
/// rather than testing the one it happens to care about. The predecessor was
/// `Option<&'static str>`, a suggestion string, which answered "what should
/// this have been" and threw away "and what exactly was wrong with it": the
/// autofix then had to recover the fault by comparing the source back to the
/// literal `"xx"`, and so could repair that one spelling and no other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerSpelling {
    /// Exactly the canonical spelling: `xxx`, `yyy`, `www`.
    Canonical(UntranscribedStatus),
    /// The letter twice where CHAT writes it three times: `xx`, `WW`, `Yy`.
    Shortened(UntranscribedStatus),
    /// The right letters, three of them, but not all lowercase: `XXX`, `Xxx`.
    Miscased(UntranscribedStatus),
    /// Not one of the three markers, however spelled.
    NotAMarker,
}

impl MarkerSpelling {
    /// Classify a token against the whole marker vocabulary.
    ///
    /// Derived by iterating `UntranscribedStatus::ALL`, so the set of
    /// rejected spellings is a consequence of the marker list rather than a
    /// second thing to keep beside it.
    #[must_use]
    pub fn of(text: &str) -> Self {
        // 99% of words are not markers (measured: 610,345 of 64.9M), and the
        // loop below spends up to nine string comparisons discovering it. This
        // rejects them on length and first letter first.
        //
        // It is a fact ABOUT `spellings()` rather than a second copy of it:
        // every canonical form is one ASCII letter written three times and every
        // shortened form is the same letter twice, so 2 or 3 bytes beginning
        // x/y/w in either case is a necessary condition. The loop still owns the
        // answer; this only declines to ask it.
        let bytes = text.as_bytes();
        match bytes.first() {
            Some(first)
                if matches!(bytes.len(), 2 | 3) && matches!(first | 0x20, b'x' | b'y' | b'w') => {}
            _ => return Self::NotAMarker,
        }

        for status in UntranscribedStatus::ALL {
            let Spellings {
                canonical,
                shortened,
            } = status.spellings();
            // Order matters, and only here: the exact spelling is also a
            // case-insensitive match for itself, so `Canonical` has to be
            // decided before `Miscased` or nothing would ever be canonical.
            if text == canonical {
                return Self::Canonical(status);
            }
            if text.eq_ignore_ascii_case(canonical) {
                return Self::Miscased(status);
            }
            if text.eq_ignore_ascii_case(shortened) {
                return Self::Shortened(status);
            }
        }
        Self::NotAMarker
    }

    /// The marker this token was meant to be, when it is spelled WRONGLY.
    ///
    /// `None` for a canonical spelling as well as for a non-marker, because both
    /// mean "nothing to repair" and every caller so far is a repair path: the
    /// validator deciding whether to raise E241, and the autofix deciding what
    /// to splice.
    ///
    /// An exhaustive match rather than an `if let`, so a fifth spelling class
    /// has to decide this at compile time instead of inheriting `None`.
    #[must_use]
    pub fn misspelled(self) -> Option<UntranscribedStatus> {
        match self {
            Self::Shortened(status) | Self::Miscased(status) => Some(status),
            Self::Canonical(_) | Self::NotAMarker => None,
        }
    }
}

impl UntranscribedStatus {
    /// Every variant, so the lookups below iterate the vocabulary rather than
    /// listing it.
    ///
    /// Private: its only consumer is in this module, and a `pub` constant on
    /// the CHAT core advertises a use case nothing exercises. Widen it when a
    /// second caller actually appears.
    const ALL: [Self; 3] = [Self::Unintelligible, Self::Phonetic, Self::Untranscribed];

    /// Both spellings of this marker. The single place either is written down.
    ///
    /// Private, and deliberately: the question a caller actually has is
    /// "what is this token", which is [`MarkerSpelling::of`]. Handing out the
    /// raw strings is what let four separate call sites each write their own
    /// comparison, one of which is still visible in the git history as
    /// `if source.get(span.to_range())? != "xx"`.
    const fn spellings(self) -> Spellings {
        match self {
            Self::Unintelligible => Spellings {
                canonical: "xxx",
                shortened: "xx",
            },
            Self::Phonetic => Spellings {
                canonical: "yyy",
                shortened: "yy",
            },
            Self::Untranscribed => Spellings {
                canonical: "www",
                shortened: "ww",
            },
        }
    }

    /// The canonical CHAT spelling of this marker.
    ///
    /// Everything that needs to know the three tokens, including `WriteChat`
    /// below, asks here.
    #[must_use]
    pub const fn canonical(self) -> &'static str {
        self.spellings().canonical
    }

    /// The marker this text spells, ignoring case.
    ///
    /// Case-insensitive because legacy corpora contain `XXX`, which is illegal
    /// (E241) but unambiguously means untranscribed material. Treating it as an
    /// ordinary word sent it to the morphotagger and produced spurious `%mor`.
    ///
    /// A SHORTENED form is deliberately not recognised here, and that is the
    /// shipped behaviour rather than an oversight: `xx` has been rejected by
    /// E241 for years without ever counting as untranscribed material, so a
    /// file containing it is invalid and gets repaired rather than
    /// reinterpreted. Recognising short forms would change what `%mor` is asked
    /// to tag, which is a separate decision from whether the spelling is legal.
    #[must_use]
    pub fn from_marker_text(text: &str) -> Option<Self> {
        match MarkerSpelling::of(text) {
            MarkerSpelling::Canonical(status) | MarkerSpelling::Miscased(status) => Some(status),
            MarkerSpelling::Shortened(_) | MarkerSpelling::NotAMarker => None,
        }
    }
}

impl WriteChat for UntranscribedStatus {
    /// Writes canonical CHAT marker text (`xxx`, `yyy`, or `www`).
    ///
    /// Serialization is intentionally lossless and normalization-free because
    /// downstream tooling relies on the exact marker token.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.canonical())
    }
}
