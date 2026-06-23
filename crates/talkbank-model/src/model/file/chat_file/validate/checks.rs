//! File-level header, media, and cross-header consistency checks.
//!
//! The `@Options` mode probes (`file_uses_ca_mode` / `file_uses_bullets_mode`)
//! and the media-linkage / media-filename / cross-header validators called by
//! `build_validation_context`. Extracted verbatim from `validate.rs`; each is
//! re-exported `pub(super)` so the orchestrator continues to call them by name.

use std::collections::HashSet;

use crate::Header;
use crate::validation::ValidationState;

use super::ChatFile;

/// Return whether any `@Options` header enables CA mode.
///
/// CA mode relaxes some structural constraints and is propagated into the
/// shared validation context for downstream checks.
pub(super) fn file_uses_ca_mode(headers: &[&Header]) -> bool {
    headers.iter().any(|header| match header {
        Header::Options { options } => options
            .iter()
            .any(crate::model::ChatOptionFlag::enables_ca_mode),
        _ => false,
    })
}

/// E544: @Media declares linkage but transcript has no timing evidence.
///
/// Fires when an @Media header is present, its `status` field is `None`
/// (i.e., not one of `unlinked` / `missing` / `notrans`), AND the file
/// carries no timing evidence. Timing evidence is the union of:
/// - main-tier bullets (already collected by the caller and passed as
///   `main_bullets`, avoids a second walk)
/// - any positional `%wor` timing sidecar on any utterance
///
/// The caller passes the already-collected main-tier bullets to avoid a
/// duplicate walk; all other timing surfaces are discovered here.
///
/// Spec: `spec/errors/E544_media_linkage_without_timing.md`.
pub(super) fn check_media_linkage_has_timing<S: ValidationState>(
    headers: &[(&Header, crate::Span)],
    file: &ChatFile<S>,
    main_bullets: &[&crate::model::Bullet],
    errors: &impl crate::ErrorSink,
) {
    use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

    // Find the first @Media header with no status. Multiple @Media headers
    // would individually need checking, but in practice a file has at most
    // one, and if any is unqualified, the check fires at that header's span.
    let unqualified_media = headers.iter().find_map(|(header, span)| match header {
        Header::Media(m) if m.status.is_none() => Some((m, *span)),
        _ => None,
    });
    let Some((_media, span)) = unqualified_media else {
        // No @Media, or @Media has a status, check does not apply.
        return;
    };

    if !main_bullets.is_empty() {
        // Main-tier bullets satisfy the timing requirement.
        return;
    }

    // Check for any positional %wor timing sidecar as a broader timing
    // surface. Forced-alignment output typically has %wor bullets even when
    // the main tier does not.
    let has_wor_timing = file.utterances().any(|utt| {
        utt.alignments
            .as_ref()
            .and_then(|a| a.wor_timings.as_ref())
            .is_some_and(|w| w.is_positional())
    });
    if has_wor_timing {
        return;
    }

    errors.report(ParseError::new(
        ErrorCode::MediaLinkageWithoutTiming,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new("", 0..0, "media_linkage"),
        "@Media header declares linkage but transcript has no timing evidence (no main-tier bullets, no %wor timing); add `, unlinked` / `, missing` / `, notrans` status, or add timing bullets",
    ));
}

/// E552: the `@Media` header declares `unlinked`, yet the transcript has timing
/// bullets, so the media is in fact linked and the `unlinked` qualifier must be
/// removed. This is the inverse of [`check_media_linkage_has_timing`] (E544):
/// there, declared linkage lacks timing; here, declared `unlinked` has timing.
///
/// The caller passes the already-collected main-tier bullets to avoid a
/// duplicate walk; any positional `%wor` timing sidecar is the other timing
/// surface checked here. Corresponds to CLAN CHECK error 124 ("remove
/// \"unlinked\" from @Media header").
pub(super) fn check_media_unlinked_has_no_timing<S: ValidationState>(
    headers: &[(&Header, crate::Span)],
    file: &ChatFile<S>,
    main_bullets: &[&crate::model::Bullet],
    errors: &impl crate::ErrorSink,
) {
    use crate::model::MediaStatus;
    use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

    // Find the first @Media header whose status is `unlinked`. A file has at
    // most one @Media in practice; if any is `unlinked`, the check fires at
    // that header's span.
    let unlinked_span = headers.iter().find_map(|(header, span)| match header {
        Header::Media(m) if matches!(m.status, Some(MediaStatus::Unlinked)) => Some(*span),
        _ => None,
    });
    let Some(span) = unlinked_span else {
        // No @Media, or its status is not `unlinked`: check does not apply.
        return;
    };

    // Any timing surface (main-tier bullets, or a positional %wor sidecar)
    // contradicts the `unlinked` declaration.
    let has_wor_timing = file.utterances().any(|utt| {
        utt.alignments
            .as_ref()
            .and_then(|a| a.wor_timings.as_ref())
            .is_some_and(|w| w.is_positional())
    });
    if main_bullets.is_empty() && !has_wor_timing {
        // `unlinked` with no timing is the correct, expected state.
        return;
    }

    errors.report(ParseError::new(
        ErrorCode::MediaUnlinkedWithTiming,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new("", 0..0, "media_linkage"),
        "@Media header declares `unlinked`, but the transcript has timing bullets; remove `unlinked` from the @Media header (the media is in fact linked)",
    ));
}

/// E531: validate `@Media` filename against the caller-provided file basename.
pub(super) fn check_media_filename_match(
    headers: &[(&Header, crate::Span)],
    file_name: &str,
    errors: &impl crate::ErrorSink,
) {
    use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

    // Find @Media header
    for (header, span) in headers {
        if let Header::Media(media_header) = header {
            // CLAN exempts remote URL media references from the filename-match
            // rule (verified against real CLAN: `@Media: "https://..."` yields no
            // CHECK 157), so a URL points at remote media and a local-basename
            // match is meaningless. The "is this a URL" decision lives on the
            // MediaFilename newtype, not inline here.
            if media_header.filename.is_remote_url() {
                break;
            }

            let media_filename = media_header.filename.as_str();

            // Compare media filename with provided filename (case-insensitive)
            if !media_filename.eq_ignore_ascii_case(file_name) {
                let media_type_str = media_header.media_type.as_str();

                let mut err = ParseError::new(
                    ErrorCode::MediaFilenameMismatch,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new(media_filename, 0..media_filename.len(), "media_filename"),
                    format!(
                        "Media filename '{}' does not match file name '{}' (case-insensitive comparison)",
                        media_filename, file_name
                    ),
                )
                .with_suggestion(format!(
                    "Update @Media header to: @Media:\t{}, {}",
                    file_name, media_type_str
                ));
                err.location.span = *span;
                errors.report(err);
            }

            // Only check the first @Media header
            break;
        }
    }
}

/// Cross-header consistency checks:
/// - CHECK 122: @ID language not defined on @Languages
/// - CHECK 142: Role on @ID differs from @Participants
pub(super) fn check_cross_header_consistency<S: ValidationState>(
    file: &ChatFile<S>,
    headers: &[(&Header, crate::Span)],
    errors: &impl crate::ErrorSink,
) {
    use crate::model::LanguageCode;
    use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

    // Collect declared languages from @Languages header
    let declared_languages: HashSet<&LanguageCode> = file.languages.0.iter().collect();

    for (header, span) in headers {
        if let Header::ID(id_header) = header {
            // CHECK 122: @ID language not in @Languages
            for id_lang in &id_header.language.0 {
                if !declared_languages.is_empty() && !declared_languages.contains(id_lang) {
                    let lang_str = id_lang.as_str();
                    let mut err = ParseError::new(
                        ErrorCode::InvalidLanguageCode,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new(lang_str, 0..lang_str.len(), "id_language"),
                        format!(
                            "Language '{}' on @ID tier is not defined on @Languages header",
                            lang_str
                        ),
                    )
                    .with_suggestion(format!(
                        "Add '{}' to @Languages header or fix the @ID language field",
                        lang_str
                    ));
                    err.location.span = *span;
                    errors.report(err);
                }
            }

            // CHECK 142: Role on @ID differs from @Participants
            let id_speaker = &id_header.speaker;
            let id_role = id_header.role.as_str();
            if !id_speaker.is_empty()
                && !id_role.is_empty()
                && let Some(participant) = file
                    .participants
                    .get(&crate::model::SpeakerCode::from(id_speaker.as_str()))
            {
                let participant_role = participant.role.as_str();
                if !participant_role.is_empty() && participant_role != id_role {
                    let mut err = ParseError::new(
                        ErrorCode::InvalidParticipantRole,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new(id_role, 0..id_role.len(), "id_role"),
                        format!(
                            "Speaker '{}' has role '{}' on @ID but '{}' on @Participants",
                            id_speaker, id_role, participant_role
                        ),
                    )
                    .with_suggestion("Ensure @ID role matches @Participants role for each speaker");
                    err.location.span = *span;
                    errors.report(err);
                }
            }
        }
    }
}
