//! Backend-independent whole-file parse products.
//!
//! A built model and its diagnostics remain together, even after recovery.
//! Neither product variant certifies validity. Both parser backends use this
//! type through the shared `ChatParser` contract.

use super::{ChatParser, ParseOutcome};
use crate::errors::{
    DiagnosticLocation, ErrorCode, ErrorCollector, ErrorContext, ParseError, Severity,
};
use crate::model::ChatFile;
use crate::{ErrorSink, NullErrorSink, TeeErrorSink};

/// The product of parsing a whole CHAT document.
///
/// There is deliberately no variant meaning "a model was built but you do
/// not get it": every case that built a [`ChatFile`] hands it back, along
/// with whatever diagnostics fired while building it. A caller that wants
/// the old fail-on-any-diagnostic behaviour makes that a visible, local
/// decision by inspecting `diagnostics` (or [`ParseProduct::has_error_diagnostics`])
/// itself, rather than the type silently making that call on the caller's
/// behalf.
///
/// Production callers must handle both variants. The fixture-unwrapping
/// extension is available only from the separate parser-test support crate:
///
/// ```compile_fail
/// use talkbank_model::{ChatFile, ParseProduct};
/// fn discard_failure(product: ParseProduct) -> ChatFile {
///     product.expect_built()
/// }
/// ```
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
        /// Diagnostics explaining why no model could be built.
        /// [`ParseProduct::parse`] emits an explicit
        /// [`crate::errors::ErrorCode::ParseFailed`] if a backend rejects
        /// without a diagnostic. Direct enum construction does not enforce
        /// a nonempty collection; rejection remains explicit in the variant.
        diagnostics: Vec<ParseError>,
    },
}

impl ParseProduct {
    /// Parse a whole document at origin zero using either backend.
    /// Retain the actual model and every emitted diagnostic. A rejection
    /// without diagnostics is made explicit as ParseFailed, never a model.
    pub fn parse(parser: &impl ChatParser, input: &str) -> Self {
        Self::parse_streaming(parser, input, &NullErrorSink)
    }

    /// Parse while forwarding each diagnostic to the caller and retaining it
    /// in the returned product. The sink cannot suppress the retained evidence.
    /// Diagnostics are retained in memory even when the caller discards them.
    pub fn parse_streaming(parser: &impl ChatParser, input: &str, errors: &impl ErrorSink) -> Self {
        let collected = ErrorCollector::new();
        let recording = TeeErrorSink::new(errors, &collected);
        let outcome = parser.parse_chat_file(input, 0, &recording);

        let diagnostics = collected.into_vec();
        match outcome {
            ParseOutcome::Parsed(chat_file) => ParseProduct::Built {
                file: chat_file,
                diagnostics,
            },
            ParseOutcome::Rejected => {
                let diagnostics = if diagnostics.is_empty() {
                    let diagnostic = ParseError::new(
                        ErrorCode::ParseFailed,
                        Severity::Error,
                        DiagnosticLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, crate::Span::new(0, input.len()), input),
                        "Parser returned no result and emitted no diagnostics",
                    );
                    errors.report(diagnostic.clone());
                    vec![diagnostic]
                } else {
                    diagnostics
                };
                ParseProduct::Unbuildable { diagnostics }
            }
        }
    }

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
}
