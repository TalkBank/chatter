//! The separator between a CHAT line's label and its content.

use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// The separator between a CHAT line's label and its content: the colon, the
/// one required tab, and any (illegal) trailing spaces. Every source line
/// (`*SPK:`, `%tier:`, `@header:`) has one.
///
/// On a well-formed line `trailing_space` is `None`. A non-CA file with
/// trailing spaces is E758, and serialization canonicalizes the separator to
/// a single tab: the spaces are never re-emitted. Provenance only: the span
/// is `#[serde(skip)]` / `#[schemars(skip)]` / `#[semantic_eq(skip)]`, so the
/// JSON wire form and schema are unchanged and two separators compare
/// semantically equal regardless of position; `SpanShift` shifts the span.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
pub struct TierSeparator {
    /// Source span of illegal trailing spaces after the tab, if any. `.start`
    /// is the first space byte; `None` for a clean single-tab separator.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub trailing_space: Option<Span>,
}

impl TierSeparator {
    /// A clean separator (single tab, no trailing spaces).
    pub const CLEAN: Self = Self {
        trailing_space: None,
    };

    /// A separator carrying illegal trailing spaces at `span`.
    pub fn with_trailing_space(span: Span) -> Self {
        Self {
            trailing_space: Some(span),
        }
    }

    /// The illegal trailing-space span, if the separator has one.
    pub fn trailing_space(&self) -> Option<Span> {
        self.trailing_space
    }
}
