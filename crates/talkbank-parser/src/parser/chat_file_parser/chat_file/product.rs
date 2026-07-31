//! The product of a strict whole-file CHAT parse.
//!
//! [`ParseProduct`] is the return type of [`TreeSitterParser::parse_chat_file`]
//! (`crate::parser::TreeSitterParser`). It replaced a `ParseResult<ChatFile>`
//! (`Result<ChatFile, ParseErrors>`) that discarded a successfully built
//! [`ChatFile`] whenever any error-severity diagnostic fired anywhere in the
//! document, even when the diagnostic's region had nothing to do with the
//! content the caller actually needed. `chatter debug fix-s` on a real IISRP
//! transcript had its target utterance parsed and healthy, then threw the
//! whole file away over an unrelated error hundreds of lines later.
//!
//! There is deliberately no `Option<ChatFile>` field anywhere in this type:
//! a nullable field would reintroduce exactly the "built but you do not get
//! it" ambiguity this type exists to remove. The enum forces every caller to
//! match, and every arm that built a model hands it back.

use crate::error::{ParseError, Severity};
use crate::model::ChatFile;

/// The product of parsing a whole CHAT document in strict mode.
///
/// There is deliberately no variant meaning "a model was built but you do
/// not get it": every case that built a [`ChatFile`] hands it back, along
/// with whatever diagnostics fired while building it. A caller that wants
/// the old fail-on-any-diagnostic behaviour makes that a visible, local
/// decision by inspecting `diagnostics` (or [`ParseProduct::has_error_diagnostics`])
/// itself, rather than the type silently making that call on the caller's
/// behalf.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseProduct {
    /// A model was built.
    Built {
        /// The parsed document.
        file: ChatFile,
        /// Diagnostics collected while building `file`, in emission order.
        /// May be empty (a clean parse) or non-empty: a document that
        /// needed recovery is invalid, and the caller decides what to do
        /// about that, but parsing does not lose the model in the process.
        diagnostics: Vec<ParseError>,
    },
    /// No model could be built at all.
    Unbuildable {
        /// Diagnostics explaining why no model could be built. Never
        /// empty: when the underlying parser rejects with no diagnostic of
        /// its own, a synthetic [`crate::error::ErrorCode::ParseFailed`]
        /// diagnostic is substituted so this invariant holds
        /// unconditionally.
        diagnostics: Vec<ParseError>,
    },
}

impl ParseProduct {
    /// `true` for [`ParseProduct::Built`], regardless of whether its
    /// diagnostics are empty.
    #[inline]
    pub fn is_built(&self) -> bool {
        matches!(self, Self::Built { .. })
    }

    /// `true` for [`ParseProduct::Unbuildable`].
    #[inline]
    pub fn is_unbuildable(&self) -> bool {
        matches!(self, Self::Unbuildable { .. })
    }

    /// The diagnostics collected during the parse, regardless of variant.
    #[inline]
    pub fn diagnostics(&self) -> &[ParseError] {
        match self {
            Self::Built { diagnostics, .. } | Self::Unbuildable { diagnostics } => diagnostics,
        }
    }

    /// `true` if any collected diagnostic has [`Severity::Error`].
    ///
    /// A [`ParseProduct::Built`] can still have error-severity diagnostics;
    /// this exists for callers that want to ask "was this clean" without
    /// losing the model in order to ask the question.
    #[inline]
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics()
            .iter()
            .any(|d| matches!(d.severity, Severity::Error))
    }

    /// Unwrap the built [`ChatFile`], panicking with the diagnostics on
    /// [`ParseProduct::Unbuildable`].
    ///
    /// **Test-only.** This is the single sanctioned panic path for
    /// `ParseProduct`; production code must match both variants instead of
    /// calling this. Deliberately not `#[cfg(test)]`-gated: its callers are
    /// test code in *other* crates, where a `#[cfg(test)]` written here
    /// would not apply.
    // Sanctioned panic: this method exists only to be called from test
    // code that already knows its fixture parses cleanly, mirroring the
    // crate-wide `unwrap()`-on-a-known-good-fixture idiom. See the
    // rustdoc above.
    #[allow(clippy::panic)]
    #[track_caller]
    pub fn expect_built(self) -> ChatFile {
        match self {
            Self::Built { file, .. } => file,
            Self::Unbuildable { diagnostics } => {
                panic!("ParseProduct::expect_built called on Unbuildable: {diagnostics:?}")
            }
        }
    }
}
