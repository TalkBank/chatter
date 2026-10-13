//! A byte-verified candidate becomes an application result only after policy checks.

use std::collections::HashMap;
use talkbank_model::{ErrorCode, ErrorCollector, ParseError};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::splice::{
    ByteVerifiedSplice, EditProvenance, FixDiagnosis, VerifiedRepairs,
};

/// The only text-bearing success this command may offer to its write branch.
#[derive(Debug)]
pub(super) struct PostCheckedFix<'source> {
    verified: VerifiedRepairs<'source>,
}

impl PostCheckedFix<'_> {
    pub(super) fn text(&self) -> &str {
        self.verified.bytes().text()
    }
    pub(super) fn edit_count(&self) -> usize {
        self.verified.bytes().plan().sites().len()
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum FixVerificationError {
    #[error("{code} still fires at its fix site after re-parsing the result")]
    TargetedCodeStillFires { code: ErrorCode },
    #[error(
        "re-parsing the result found {code} {after} time(s), up from {before} time(s) in the original file"
    )]
    CodeCountIncreased {
        code: ErrorCode,
        before: usize,
        after: usize,
    },
}

/// Only the diagnosis-associated result can become an application receipt.
pub(super) fn verify_fix_result<'source>(
    parser: &TreeSitterParser,
    verified: VerifiedRepairs<'source>,
) -> Result<PostCheckedFix<'source>, FixVerificationError> {
    check_fix_result(parser, verified.bytes(), verified.diagnosis())?;
    Ok(PostCheckedFix { verified })
}

/// Policy/byte boundary checks. This function can issue no application result;
/// adversarial tests supply raw byte-verified candidates and an actual diagnosis.
pub(super) fn check_fix_result(
    parser: &TreeSitterParser,
    verified: &ByteVerifiedSplice<'_>,
    diagnosis: &FixDiagnosis<'_>,
) -> Result<(), FixVerificationError> {
    let codes_before = code_counts(diagnosis.diagnostics());
    let sink = ErrorCollector::new();
    let mut reparsed = parser.parse_chat_file_streaming(verified.text(), &sink);
    reparsed.validate_at(diagnosis.policy(), &sink, diagnosis.name());
    let diagnostics_after = sink.into_vec();
    let codes_after = code_counts(&diagnostics_after);
    for (code, after_count) in &codes_after {
        // Absence in a full diagnostic multiset means zero occurrences.
        let before_count = codes_before.get(code).copied().unwrap_or(0);
        if *after_count > before_count {
            return Err(FixVerificationError::CodeCountIncreased {
                code: *code,
                before: before_count,
                after: *after_count,
            });
        }
    }
    for (code, fix_site) in
        verified
            .plan()
            .sites()
            .iter()
            .filter_map(|edit| match edit.provenance() {
                EditProvenance::Diagnostic(code) => Some((*code, edit.spliced())),
                EditProvenance::Transform(_) => None,
            })
    {
        // Content repair cannot rename the stored transcript. Keep the existing
        // W109 exception only when its media token is already normalized.
        if code == ErrorCode::MediaFilenameNonCanonicalUnicode
            && reparsed
                .headers()
                .find_map(|header| match header {
                    talkbank_model::model::Header::Media(media) => Some(media),
                    _ => None,
                })
                .is_some_and(|media| !media.filename.needs_unicode_normalization())
        {
            continue;
        }
        if diagnostics_after
            .iter()
            .any(|d| d.code == code && d.location.span.overlaps(fix_site))
        {
            return Err(FixVerificationError::TargetedCodeStillFires { code });
        }
    }
    Ok(())
}

/// Count the complete multiset, not only selected or distinct codes.
pub(super) fn code_counts(diagnostics: &[ParseError]) -> HashMap<ErrorCode, usize> {
    let mut counts = HashMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.code).or_insert(0) += 1;
    }
    counts
}
