//! [`Location`]: where a model value sits in its source, or that it has no
//! source position.
//!
//! A [`Span`] is pure geometry and always a real location, including `0..0`
//! at the start of a file. Whether a value *has* a location is a separate
//! fact, and this enum records it: a parsed node is `Located`, a node built
//! programmatically or read back from AST JSON is `Unlocated`. There is
//! deliberately no accessor that turns `Unlocated` into a default span; a
//! consumer that needs geometry matches and states its policy for the
//! unlocated arm.

use super::Span;

/// Where a value sits in its source text, or the honest statement that it
/// has no source position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Location {
    /// The value was produced from this byte span of its source.
    Located(Span),
    /// The value has no source position.
    Unlocated,
}

impl Location {
    /// [`Location::Unlocated`], as a named function for
    /// `#[serde(skip, default = "Location::unlocated")]`: located fields leave
    /// the AST wire and come back unlocated, and `Location` has no `Default`
    /// on purpose, so the field attribute names this instead.
    #[must_use]
    pub const fn unlocated() -> Self {
        Self::Unlocated
    }

    /// This location if it is located, otherwise `fallback`.
    ///
    /// For a diagnostic that would rather point at an enclosing node than
    /// say nothing: a marker falls back to its word, a word to its tier.
    #[must_use]
    pub const fn or(self, fallback: Location) -> Location {
        match self {
            Self::Located(_) => self,
            Self::Unlocated => fallback,
        }
    }

    /// The same located value in another coordinate space; unlocated stays
    /// unlocated.
    #[must_use]
    pub fn map(self, project: impl FnOnce(Span) -> Span) -> Location {
        match self {
            Self::Located(span) => Self::Located(project(span)),
            Self::Unlocated => Self::Unlocated,
        }
    }

    /// The zero-width point where a located value starts; unlocated stays so.
    ///
    /// For diagnostics that point at the start of a construct rather than
    /// cover it (a header line, an utterance).
    #[must_use]
    pub fn start_point(self) -> Location {
        self.map(|span| Span::at(span.start()))
    }

    /// The span, if the value is located.
    #[must_use]
    pub const fn span(self) -> Option<Span> {
        match self {
            Self::Located(span) => Some(span),
            Self::Unlocated => None,
        }
    }
}

impl From<Span> for Location {
    /// A span is always a real location, so this is total.
    fn from(span: Span) -> Self {
        Self::Located(span)
    }
}
