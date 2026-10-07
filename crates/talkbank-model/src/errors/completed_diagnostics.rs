//! Admission of completed diagnostic batches, before presentation or caching.

use super::{DiagnosticKind, ParseError, kind_of};

/// Diagnostics from an attempt with no reported internal failure.
///
/// Not proof of CHAT validity: invalidity findings may remain. There is no
/// mutable access, deserialization, or unchecked constructor for this evidence.
#[derive(Debug)]
pub struct CompletedDiagnostics(Vec<ParseError>);

/// A failed tool attempt, retaining all diagnostics, including input findings.
/// Neither successful validation nor a file-invalid verdict can be inferred.
#[derive(Debug, Clone)]
pub struct InternalFailure(Vec<ParseError>);

impl CompletedDiagnostics {
    /// Classify the complete, unsuppressed batch by intrinsic kind.
    /// Severity downgrades cannot conceal a producer failure.
    pub fn admit(diagnostics: Vec<ParseError>) -> Result<Self, InternalFailure> {
        if diagnostics
            .iter()
            .any(|d| kind_of(d.code) == DiagnosticKind::InternalFailure)
        {
            Err(InternalFailure(diagnostics))
        } else {
            Ok(Self(diagnostics))
        }
    }

    /// Inspect all findings before applying presentation policy.
    pub fn diagnostics(&self) -> &[ParseError] {
        &self.0
    }

    /// Discard completion evidence and recover the original diagnostics.
    pub fn into_diagnostics(self) -> Vec<ParseError> {
        self.0
    }
}

impl InternalFailure {
    /// A fault the tool detected in itself, such as two of its own values
    /// disagreeing, recorded as one internal-error diagnostic at `span`.
    /// [`ErrorCode::InternalError`](super::ErrorCode::InternalError) is an
    /// internal-failure kind, so this is exactly what admission would decide
    /// for that diagnostic.
    pub fn tool_fault(message: impl Into<String>, span: crate::Span) -> Self {
        Self(vec![ParseError::internal(message, span)])
    }

    /// All findings from the failed attempt, without filtering or relabeling.
    pub fn diagnostics(&self) -> &[ParseError] {
        &self.0
    }

    /// Recover findings for presentation, without changing the failed verdict.
    pub fn into_diagnostics(self) -> Vec<ParseError> {
        self.0
    }
}

impl std::fmt::Display for InternalFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "internal tool failure; CHAT validity was not determined")?;
        for diagnostic in &self.0 {
            write!(f, "\n  {} {}", diagnostic.code.as_str(), diagnostic.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for InternalFailure {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, Severity, Span};

    /// Boundary policy: ignore presentation severity, retain mixed evidence,
    /// and distinguish completed invalidity from tool failure.
    #[test]
    fn tool_failure_is_not_validity_even_when_downgraded() {
        let invalid = ParseError::at_span(
            ErrorCode::SyntaxError,
            Severity::Error,
            Span::new(0, 1),
            "invalid syntax",
        );
        assert!(CompletedDiagnostics::admit(vec![invalid.clone()]).is_ok());
        assert!(CompletedDiagnostics::admit(vec![]).is_ok());
        let mut internal = ParseError::internal("producer fault", Span::new(0, 1));
        assert_eq!(internal.code, ErrorCode::InternalError);
        assert_eq!(internal.severity, Severity::Error);
        internal.severity = Severity::Warning;
        let failure = CompletedDiagnostics::admit(vec![invalid, internal]).unwrap_err();
        assert_eq!(failure.diagnostics().len(), 2);
        assert_eq!(failure.diagnostics()[1].code, ErrorCode::InternalError);
        assert!(failure.to_string().contains("validity was not determined"));
    }

    /// The direct constructor agrees with admission: a tool fault re-admitted
    /// is refused as an internal failure, never completed diagnostics.
    #[test]
    fn a_tool_fault_is_what_admission_would_refuse() {
        let fault = InternalFailure::tool_fault("plan and document disagree", Span::new(3, 9));
        let [diagnostic] = fault.diagnostics() else {
            panic!("one diagnostic")
        };
        assert_eq!(diagnostic.code, ErrorCode::InternalError);
        assert_eq!(diagnostic.location.span, Span::new(3, 9));
        assert!(CompletedDiagnostics::admit(fault.into_diagnostics()).is_err());
    }
}
