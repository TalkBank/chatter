//! Immutable diagnostic evidence paired with checked, display-local coordinates.

use super::chat_formatting::ProjectedDisplayRange;
use super::parse_error::SourceCodeWithOffset;
use super::{DiagnosticLocation, LineColumn, ParseError, SourceIndex, Span};
use miette::Diagnostic;
use std::fmt;
use std::sync::OnceLock;

/// Why source coordinates could not be admitted for presentation.
///
/// This is a context failure, not a new CHAT diagnostic or validation verdict.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiagnosticSourceError {
    /// The primary span is reversed, out of range, or splits a UTF-8 scalar.
    #[error("primary span {span:?} is not a UTF-8 range in source length {source_len}")]
    PrimaryRange {
        /// Rejected absolute primary range, unchanged.
        span: Span,
        /// Supplied source length in bytes.
        source_len: usize,
    },
    /// A secondary span cannot be interpreted in the supplied source.
    #[error("label {index} span {span:?} is not a UTF-8 range in source length {source_len}")]
    LabelRange {
        /// Zero-based index in the original label list.
        index: usize,
        /// Rejected absolute secondary range, unchanged.
        span: Span,
        /// Supplied source length in bytes.
        source_len: usize,
    },
    /// No source bytes are available to show a context window.
    #[error("the source buffer is empty")]
    EmptySource,
    /// The diagnostic concerns a value with no source position, so there is
    /// no window of the source to show.
    #[error("the diagnostic has no source location")]
    Unlocated,
}

/// A checked projection, never an absolute-source `ParseError`.
///
/// Construction is private. Its primary context and secondary labels share one
/// display-local coordinate space; the file location retains absolute bytes.
pub struct DiagnosticPresentation {
    span: Span,
    position: LineColumn,
    pub(super) context: DiagnosticContext,
    pub(super) labels: Vec<DiagnosticLabel>,
    source_cache: OnceLock<SourceCodeWithOffset>,
}

/// A source window in display coordinates, with a mandatory first file line.
///
/// Unlike producer `ErrorContext`, this has no optional line offset or partial
/// snippet state. It cannot be constructed or edited by presentation consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticContext {
    pub(super) source_text: String,
    pub(super) span: DisplaySpan,
    pub(super) first_line: usize,
}

impl DiagnosticContext {
    /// Projected display text, not the original file bytes.
    pub fn source_text(&self) -> &str {
        &self.source_text
    }
    /// Primary highlight in `source_text()`.
    pub fn span(&self) -> DisplaySpan {
        self.span
    }
    /// One-based file line at the start of this window.
    pub fn first_line(&self) -> usize {
        self.first_line
    }
}

/// A range in projected display text, not in the original source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplaySpan(Span);

impl DisplaySpan {
    /// The producer guarantees `start <= end` on UTF-8 boundaries of the
    /// display text, so a projected range is always a valid display span.
    pub(super) fn from_projected(range: ProjectedDisplayRange<'_>) -> Self {
        Self(Span::new(range.start(), range.end()))
    }
    /// Inclusive display byte offset.
    pub fn start(self) -> usize {
        self.0.start
    }
    /// Exclusive display byte offset.
    pub fn end(self) -> usize {
        self.0.end
    }
    /// A range for slicing the associated display text.
    pub fn to_range(self) -> std::ops::Range<usize> {
        self.0.to_range()
    }
}

impl From<DisplaySpan> for miette::SourceSpan {
    fn from(span: DisplaySpan) -> Self {
        span.0.into()
    }
}

/// A secondary label in display coordinates. It cannot be attached to a raw
/// diagnostic as an absolute `ErrorLabel` without an explicit reconstruction.
///
/// ```compile_fail
/// # use talkbank_model::{DiagnosticPresentation, ParseError};
/// # fn attach_display_label(raw: ParseError, display: &DiagnosticPresentation) {
/// raw.with_label(display.labels()[0].clone());
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    pub(super) span: DisplaySpan,
    pub(super) message: String,
}

impl DiagnosticLabel {
    /// Range in the presentation's display window.
    pub fn span(&self) -> DisplaySpan {
        self.span
    }
    /// Original label wording.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl DiagnosticPresentation {
    pub(super) fn new(
        span: Span,
        position: LineColumn,
        context: DiagnosticContext,
        labels: Vec<DiagnosticLabel>,
    ) -> Self {
        Self {
            span,
            position,
            context,
            labels,
            source_cache: OnceLock::new(),
        }
    }

    /// Checked absolute file location, including one-based byte coordinates.
    pub fn location(&self) -> DiagnosticLocation {
        DiagnosticLocation::known_at(self.span, self.position)
    }
    /// Absolute byte span in the file, always known in an admitted presentation.
    pub fn file_span(&self) -> Span {
        self.span
    }
    /// One-based line and byte column, always known in an admitted presentation.
    pub fn position(&self) -> LineColumn {
        self.position
    }

    /// The projected source window and its display-local primary span.
    pub fn context(&self) -> &DiagnosticContext {
        &self.context
    }

    /// Secondary labels in the same display-local window as `context()`.
    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }
}

impl Clone for DiagnosticPresentation {
    fn clone(&self) -> Self {
        Self::new(
            self.span,
            self.position,
            self.context.clone(),
            self.labels.clone(),
        )
    }
}

impl fmt::Debug for DiagnosticPresentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticPresentation")
            .field("location", &self.location())
            .field("context", &self.context)
            .field("labels", &self.labels)
            .finish()
    }
}

impl PartialEq for DiagnosticPresentation {
    fn eq(&self, other: &Self) -> bool {
        self.location() == other.location()
            && self.context == other.context
            && self.labels == other.labels
    }
}

/// Original diagnostic evidence and the outcome of checked source projection.
///
/// There is no conversion from presentation back to `ParseError`, and no mutable
/// accessor. Repeated rendering uses this same value without reinterpreting its
/// display-local labels as absolute file offsets.
///
/// Range admission does **not** prove diagnostic/source origin: callers must
/// retain the producing source snapshot, not reread a potentially edited file.
/// A different same-length source may contain admissible but unrelated ranges.
///
/// ```compile_fail
/// # use talkbank_model::{SourceDiagnostic, SourceIndex, enhance_errors_with_index};
/// # fn project_again(display: SourceDiagnostic, index: &SourceIndex<'_>) {
/// enhance_errors_with_index(&[display], index);
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiagnostic {
    original: ParseError,
    presentation: Result<DiagnosticPresentation, DiagnosticSourceError>,
}

impl SourceDiagnostic {
    /// Admit every absolute span before projecting any of them. Failure keeps
    /// the original diagnostic and exposes no guessed source coordinates.
    pub fn with_index(original: ParseError, index: &SourceIndex<'_>) -> Self {
        let presentation = super::enhance::project(&original, index);
        Self {
            original,
            presentation,
        }
    }

    /// Original code, severity, message, absolute spans, labels and context.
    pub fn original(&self) -> &ParseError {
        &self.original
    }

    /// Checked presentation, or the explicit reason it is unavailable.
    pub fn presentation(&self) -> Result<&DiagnosticPresentation, &DiagnosticSourceError> {
        self.presentation.as_ref()
    }

    /// Checked file location only; never falls back to stale producer coordinates.
    pub fn location(&self) -> Result<DiagnosticLocation, &DiagnosticSourceError> {
        self.presentation().map(|p| p.location())
    }

    /// Display context only, never the original producer's partial snippet.
    pub fn context(&self) -> Result<&DiagnosticContext, &DiagnosticSourceError> {
        self.presentation().map(|p| &p.context)
    }
}

impl fmt::Display for SourceDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.original();
        write!(f, "{}[{}]: {}", raw.severity, raw.code, raw.message)?;
        match (self.presentation(), raw.location) {
            (Ok(p), _) => write!(f, " ({})", p.location()),
            (Err(_), DiagnosticLocation::Unlocated) => write!(f, " ({})", raw.location),
            // Only the bytes: a producer's cached line and column were never
            // checked against this source, so an unadmitted one is not shown.
            (Err(reason), DiagnosticLocation::Known { span, .. }) => write!(
                f,
                " ({}; source context unavailable: {reason})",
                DiagnosticLocation::new(span)
            ),
        }
    }
}

impl std::error::Error for SourceDiagnostic {}

impl Diagnostic for SourceDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Diagnostic::code(self.original())
    }
    fn severity(&self) -> Option<miette::Severity> {
        Diagnostic::severity(self.original())
    }
    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Diagnostic::help(self.original())
    }
    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Diagnostic::url(self.original())
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        let p = match self.presentation() {
            Ok(p) => p,
            Err(_) => return None,
        };
        Some(Box::new(
            std::iter::once(miette::LabeledSpan::new_with_span(
                Some("here".to_owned()),
                p.context.span,
            ))
            .chain(p.labels.iter().map(|label| {
                miette::LabeledSpan::new_with_span(Some(label.message.clone()), label.span)
            })),
        ))
    }
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        let p = match self.presentation() {
            Ok(p) => p,
            Err(_) => return None,
        };
        Some(p.source_cache.get_or_init(|| {
            SourceCodeWithOffset::new("input", p.context.source_text.clone(), p.context.first_line)
        }))
    }
}
