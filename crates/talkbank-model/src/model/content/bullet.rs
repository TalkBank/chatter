//! Media timing bullets (`\u{0015}start_end\u{0015}`).
//!
//! Bullets anchor utterance content to media offsets.
//!
//! # CHAT Format References
//!
//! - [Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)

use super::WriteChat;
use crate::model::MediaTiming;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Provenance of a media timing bullet.
///
/// Bullets are set in two distinct ways, by the UTR pre-pass (provisional)
/// and by human annotation or FA post-processing (authoritative). The source
/// field is not serialized and has no effect on CHAT output; it is used
/// by the forced-alignment pipeline to decide how to update the utterance
/// bullet after FA word timings are injected.
///
/// ## Rules encoded by each variant
///
/// - `Utr`, `update_utterance_bullet` **overwrites** this bullet with the
///   FA word span. The UTR window was a provisional grouping hint; once FA
///   has produced word timings the hint is discarded.
///
/// - `Authoritative`, `update_utterance_bullet` **unions** this bullet with
///   the FA word span (never shrinks). Hand-linked annotations may cover
///   leading fillers, trailing gestures, or other non-alignable content whose
///   timing would be lost if the bullet were blindly overwritten with the
///   word span.
///
/// The default is `Authoritative`, which matches the behavior of all bullets
/// that arrive from a parsed CHAT file or were constructed programmatically
/// after FA (e.g., by `Bullet::new`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BulletSource {
    /// Provisional hint from the UTR (Utterance Timing Recovery) pre-pass.
    ///
    /// FA word timings overwrite this bullet.
    Utr,
    /// Hand-linked annotation, previous FA run, or FA-derived timing.
    ///
    /// FA word timings union with (never shrink) this bullet.
    #[default]
    Authoritative,
}

/// Media timing bullet attached to an utterance or header-level comment payload.
///
/// Start/end offsets are stored in milliseconds. The `source` field records
/// whether the bullet is a provisional UTR hint or an authoritative annotation;
/// it is not serialized and does not appear in CHAT output.
///
/// # CHAT Format
///
/// Bullets appear at the end of utterances using the `\u{0015}` (NEGATIVE ACKNOWLEDGE)
/// character as a delimiter:
///
/// ```text
/// \u{0015}start_end\u{0015}
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: I want cookie . \u{0015}0_1073\u{0015}
/// *MOT: here you go . \u{0015}1073_2456\u{0015}
/// ```
///
/// # References
///
/// - [Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)
/// - [Media Linking](https://talkbank.org/0info/manuals/CHAT.html#MediaLinking)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Bullet {
    /// Start/end media offsets in milliseconds.
    #[serde(flatten)]
    pub timing: MediaTiming,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,

    /// Provenance of this bullet (not serialized, not part of CHAT output).
    ///
    /// Controls whether `update_utterance_bullet` overwrites or unions when
    /// FA word timings are available. See [`BulletSource`] for invariants.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub source: BulletSource,
}

impl Bullet {
    /// Build an authoritative bullet with dummy span metadata.
    ///
    /// Use this constructor for FA-derived timing, hand-linked annotations,
    /// and any bullet that should not be overwritten by a subsequent UTR or FA pass.
    pub fn new(location: impl Into<crate::Location>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            timing: MediaTiming::new(start_ms, end_ms),
            location: location.into(),
            source: BulletSource::Authoritative,
        }
    }

    /// Build a provisional UTR hint bullet.
    ///
    /// UTR sets this before FA runs to mark the audio window for grouping.
    /// `update_utterance_bullet` will overwrite it once FA produces word timings.
    pub fn utr_hint(location: impl Into<crate::Location>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            timing: MediaTiming::new(start_ms, end_ms),
            location: location.into(),
            source: BulletSource::Utr,
        }
    }
}

impl WriteChat for Bullet {
    /// Serializes canonical CHAT bullet syntax.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_char('\u{0015}')?;
        {
            crate::model::WriteChat::write_chat(&self.timing.start_ms, w)?;
            w.write_str("_")?;
            crate::model::WriteChat::write_chat(&self.timing.end_ms, w)
        }?;
        w.write_char('\u{0015}')
    }
}

impl std::fmt::Display for Bullet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

/// Observed media-bullet source that could not be admitted as timing.
///
/// The source includes its delimiters. This is always invalid recovery data,
/// including after JSON deserialization, and provides no numeric accessors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct UnparsedBullet {
    source: smol_str::SmolStr,
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub(crate) location: crate::Location,
}

impl UnparsedBullet {
    /// Preserve observed source without claiming it is usable timing.
    pub fn new(location: impl Into<crate::Location>, source: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            source: source.into(),
            location: location.into(),
        }
    }

    /// Exact rejected spelling, including delimiters.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Location of the observed source, when available.
    pub fn location(&self) -> crate::Location {
        self.location
    }
}

impl WriteChat for UnparsedBullet {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.source())
    }
}

/// An occupied media-bullet position: numeric timing or explicit rejected source.
///
/// Absence is represented by the enclosing optional slot, never by this enum.
/// Parsed JSON retains the existing timing shape; rejected JSON uses an explicit
/// `unparsed` payload. Neither reading nor writing recovery admits numeric timing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(untagged)]
pub enum BulletState {
    /// Numerically representable timing; semantic validation remains separate.
    Parsed(Bullet),
    /// Observed source that failed numeric or syntax admission.
    Unparsed {
        /// Exact rejected source, never numeric timing.
        unparsed: UnparsedBullet,
    },
}

impl BulletState {
    /// Project numeric timing only from the parsed state.
    pub fn as_parsed(&self) -> Option<&Bullet> {
        match self {
            Self::Parsed(bullet) => Some(bullet),
            Self::Unparsed { .. } => None,
        }
    }

    /// Mutably project numeric timing only from the parsed state.
    pub fn as_parsed_mut(&mut self) -> Option<&mut Bullet> {
        match self {
            Self::Parsed(bullet) => Some(bullet),
            Self::Unparsed { .. } => None,
        }
    }

    /// Consume the state, returning the rejected source when timing is unavailable.
    pub fn into_parsed(self) -> Result<Bullet, UnparsedBullet> {
        match self {
            Self::Parsed(bullet) => Ok(bullet),
            Self::Unparsed { unparsed } => Err(unparsed),
        }
    }

    /// Source location of either occupied state.
    pub fn location(&self) -> crate::Location {
        match self {
            Self::Parsed(bullet) => bullet.location,
            Self::Unparsed { unparsed } => unparsed.location(),
        }
    }

    /// Report recovery invalidity by presence, independently of source locations.
    pub(crate) fn validate_recovery(&self, errors: &impl crate::ErrorSink) {
        if let Self::Unparsed { unparsed } = self {
            errors.report(crate::ParseError::at_location(
                crate::ErrorCode::InvalidMediaBullet,
                crate::Severity::Error,
                unparsed.location(),
                "Unparsed media bullet cannot be used as timing",
            ));
        }
    }
}

impl From<Bullet> for BulletState {
    fn from(bullet: Bullet) -> Self {
        Self::Parsed(bullet)
    }
}

impl From<UnparsedBullet> for BulletState {
    fn from(unparsed: UnparsedBullet) -> Self {
        Self::Unparsed { unparsed }
    }
}

impl WriteChat for BulletState {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        match self {
            Self::Parsed(bullet) => bullet.write_chat(w),
            Self::Unparsed { unparsed } => unparsed.write_chat(w),
        }
    }
}

impl std::fmt::Display for BulletState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_roundtrip() {
        let bullet = Bullet::new(crate::Location::Unlocated, 0, 1073);
        let output = bullet.to_string();
        assert_eq!(output, "\u{0015}0_1073\u{0015}");
    }

    #[test]
    fn bullet_zero_time_roundtrip() {
        let bullet = Bullet::new(crate::Location::Unlocated, 0, 0);
        let output = bullet.to_string();
        assert_eq!(output, "\u{0015}0_0\u{0015}");
    }
}
