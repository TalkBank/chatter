//! Content validation for Phon's four `%x` dependent tiers.
//!
//! These checks implement Greg Hedlund's "Phon `%x` Dependent Tiers, Format &
//! Validation" specification. Word-count cross-checks (E725-E728) live in the
//! alignment pass (`compute_alignments`); this module adds the per-tier content
//! rules:
//!
//! - `%xmodsyl` / `%xphosyl`: every unit is `phone:CODE` with a legal code
//!   (E735/E736), and stripping the codes reproduces the `%mod`/`%pho` word
//!   (E737/E738).
//! - `%xphoaln`: every pair is well-formed (E739), and concatenating the model
//!   and actual sides (skipping `∅`) reproduces the `%mod` and `%pho` words
//!   (E740/E741).
//! - `%xphoint`: each bullet has `start < end` (E742), interval starts are
//!   non-decreasing (E743) and lie within the record's media bullet (E744), and
//!   each group's phones reproduce the `%pho` word (E745) with one group per
//!   `%pho` word (E746).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>

use crate::model::Utterance;
use crate::model::dependent_tier::{PhoItem, PhoTier, SylTier, SylWordKind, classify_syl_word};
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};

/// 1 ms rounding tolerance for the `%xphoint` media-bounds check.
///
/// Sub-millisecond rounding can make adjacent boundaries differ by 1 ms; the
/// spec calls this expected, not an error.
const MEDIA_BOUNDS_TOLERANCE_MS: u64 = 1;

/// Notation the segment-level reconstruction comparison ignores.
///
/// The `%xphoaln` tier aligns **segments**; its legal elements are phones, not
/// suprasegmentals or boundary notation. The source `%mod`/`%pho` word,
/// however, may carry:
///
/// - stress: primary (ˈ, U+02C8) and secondary (ˌ, U+02CC), which the
///   syllabification tiers keep attached to a phone unit (e.g. `ˈa:Np:C`);
/// - syllable-boundary notation: Phon's `^` (U+005E) between syllables
///   (e.g. `ˈbɔ^hɔɪ`) and the IPA syllable break `.` (U+002E)
///   (e.g. `ko.çɔ̃`), both attested at scale in the wild phon corpora.
///
/// The phone-by-phone alignment reconstruction is therefore compared modulo
/// these markers; otherwise a source word like `ˈbɔ^hɔɪ` would never match its
/// marker-free segmental alignment `b↔…,ɔ↔…,h↔…,ɔɪ↔…`. No phone is itself a
/// stress or boundary character, so stripping cannot mask a real segment
/// mismatch.
const SEGMENT_COMPARISON_IGNORED: [char; 4] = ['\u{02C8}', '\u{02CC}', '\u{005E}', '\u{002E}'];

/// Remove stress and syllable-boundary notation for segmental reconstruction
/// comparison.
fn strip_nonsegmental(s: &str) -> String {
    s.chars()
        .filter(|c| !SEGMENT_COMPARISON_IGNORED.contains(c))
        .collect()
}

/// Validate the content of the four Phon `%x` dependent tiers on one utterance.
pub(crate) fn check_phon_xtiers(utterance: &Utterance, errors: &impl ErrorSink) {
    validate_syllabification(utterance, errors);
    validate_phoaln(utterance, errors);
    validate_xphoint(utterance, errors);
}

/// Text of the `i`-th `%mod`/`%pho` word for reconstruction comparison.
///
/// Returns `None` for a group item (`‹...›`) or when the index is out of range;
/// in those cases the per-word reconstruction check is skipped (count mismatches
/// are reported separately by E725-E728).
fn source_word(source: Option<&PhoTier>, i: usize) -> Option<&str> {
    match source?.items.0.get(i)? {
        PhoItem::Word(word) => Some(word.as_str()),
        PhoItem::Group(_) => None,
    }
}

// ---------------------------------------------------------------------------
// %xmodsyl / %xphosyl
// ---------------------------------------------------------------------------

fn validate_syllabification(utterance: &Utterance, errors: &impl ErrorSink) {
    if let Some(modsyl) = utterance.modsyl_tier() {
        let recon_clean = utterance.parse_health.can_align_modsyl_to_mod();
        validate_syl_tier(
            modsyl,
            utterance.mod_tier(),
            recon_clean,
            ErrorCode::ModsylReconstructionMismatch,
            "%mod",
            errors,
        );
    }
    if let Some(phosyl) = utterance.phosyl_tier() {
        let recon_clean = utterance.parse_health.can_align_phosyl_to_pho();
        validate_syl_tier(
            phosyl,
            utterance.pho_tier(),
            recon_clean,
            ErrorCode::PhosylReconstructionMismatch,
            "%pho",
            errors,
        );
    }
}

fn validate_syl_tier(
    syl: &SylTier,
    source: Option<&PhoTier>,
    reconstruction_clean: bool,
    reconstruction_code: ErrorCode,
    source_label: &str,
    errors: &impl ErrorSink,
) {
    let prefix = syl.prefix();
    for (i, word) in syl.words.iter().enumerate() {
        match classify_syl_word(word.as_str()) {
            // A pause filler mirrors a pause at the same word position on the
            // source tier (Phon keeps word-aligned tiers in index lockstep).
            // It has no phone:CODE structure to check; the reconstruction
            // check degenerates to "the source word is the same pause token".
            Ok(SylWordKind::PauseFiller(_)) => {
                if !reconstruction_clean {
                    continue;
                }
                if let Some(expected) = source_word(source, i)
                    && word.as_str() != expected
                {
                    errors.report(
                        ParseError::at_span(
                            reconstruction_code,
                            Severity::Error,
                            syl.span,
                            format!(
                                "{prefix} word {} is the pause filler '{}', but the {source_label} word at that position is '{}'",
                                i + 1,
                                word.as_str(),
                                expected
                            ),
                        )
                        .with_suggestion(format!(
                            "A pause filler on {prefix} must mirror the same pause on {source_label}"
                        )),
                    );
                }
            }
            Ok(SylWordKind::Units(units)) => {
                if !reconstruction_clean {
                    continue;
                }
                let reconstructed: String = units.iter().map(|u| u.phone.as_str()).collect();
                if let Some(expected) = source_word(source, i)
                    && reconstructed != expected
                {
                    errors.report(
                        ParseError::at_span(
                            reconstruction_code,
                            Severity::Error,
                            syl.span,
                            format!(
                                "{prefix} word {} ('{}') reconstructs to '{}', which does not match the {source_label} word '{}'",
                                i + 1,
                                word.as_str(),
                                reconstructed,
                                expected
                            ),
                        )
                        .with_suggestion(format!(
                            "Make the phones in this {prefix} word match the {source_label} word"
                        )),
                    );
                }
            }
            Err(err) if err.is_illegal_code() => {
                errors.report(
                    ParseError::at_span(
                        ErrorCode::SylIllegalConstituentCode,
                        Severity::Error,
                        syl.span,
                        format!("{prefix} word {} ('{}'): {err}", i + 1, word.as_str()),
                    )
                    .with_suggestion("Use one of the legal constituent codes: O N C L R E A D U"),
                );
            }
            Err(err) => {
                errors.report(
                    ParseError::at_span(
                        ErrorCode::SylUnitMalformed,
                        Severity::Error,
                        syl.span,
                        format!("{prefix} word {} ('{}'): {err}", i + 1, word.as_str()),
                    )
                    .with_suggestion("Each unit must be one phone, a ':' , then a single code"),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// %xphoaln
// ---------------------------------------------------------------------------

fn validate_phoaln(utterance: &Utterance, errors: &impl ErrorSink) {
    let Some(phoaln) = utterance.phoaln_tier() else {
        return;
    };
    let reconstruction_clean = utterance.parse_health.can_align_phoaln();
    let mod_tier = utterance.mod_tier();
    let pho_tier = utterance.pho_tier();

    for (i, word) in phoaln.words.iter().enumerate() {
        // E739: a pair with both sides null (∅↔∅) is never legal. (Missing-arrow
        // and empty-side pairs are rejected earlier at parse time.)
        for pair in &word.pairs {
            if pair.source.is_none() && pair.target.is_none() {
                report_phoaln_pair_malformed(phoaln.span, i, errors);
            }
        }

        if !reconstruction_clean {
            continue;
        }

        // E740: model sides (skipping ∅) must reproduce the %mod word.
        let model_side: String = word
            .pairs
            .iter()
            .filter_map(|p| p.source.as_ref().map(|s| s.as_str()))
            .collect();
        check_phoaln_reconstruction(
            &model_side,
            source_word(mod_tier, i),
            phoaln.span,
            i,
            "%mod",
            ErrorCode::PhoalnModReconstructionMismatch,
            errors,
        );

        // E741: actual sides (skipping ∅) must reproduce the %pho word.
        let actual_side: String = word
            .pairs
            .iter()
            .filter_map(|p| p.target.as_ref().map(|t| t.as_str()))
            .collect();
        check_phoaln_reconstruction(
            &actual_side,
            source_word(pho_tier, i),
            phoaln.span,
            i,
            "%pho",
            ErrorCode::PhoalnPhoReconstructionMismatch,
            errors,
        );
    }
}

fn report_phoaln_pair_malformed(span: Span, word_index: usize, errors: &impl ErrorSink) {
    errors.report(
        ParseError::at_span(
            ErrorCode::PhoalnPairMalformed,
            Severity::Error,
            span,
            format!(
                "%xphoaln word {} contains a '∅↔∅' pair, which is never legal",
                word_index + 1
            ),
        )
        .with_suggestion("Every pair needs a non-null phone on at least one side"),
    );
}

fn check_phoaln_reconstruction(
    reconstructed: &str,
    expected: Option<&str>,
    span: Span,
    word_index: usize,
    source_label: &str,
    code: ErrorCode,
    errors: &impl ErrorSink,
) {
    if let Some(expected) = expected
        && strip_nonsegmental(reconstructed) != strip_nonsegmental(expected)
    {
        errors.report(
            ParseError::at_span(
                code,
                Severity::Error,
                span,
                format!(
                    "%xphoaln word {} reconstructs to '{}' on the {source_label} side, which does not match '{}'",
                    word_index + 1,
                    reconstructed,
                    expected
                ),
            )
            .with_suggestion(format!(
                "Align the {source_label}-side phones with the {source_label} word"
            )),
        );
    }
}

// ---------------------------------------------------------------------------
// %xphoint
// ---------------------------------------------------------------------------

fn validate_xphoint(utterance: &Utterance, errors: &impl ErrorSink) {
    let Some(xphoint) = utterance.xphoint_tier() else {
        return;
    };
    let span = xphoint.span;
    let pho_tier = utterance.pho_tier();
    let reconstruction_clean = utterance.parse_health.can_align_xphoint_to_pho();

    // E746: one group per %pho word.
    if reconstruction_clean
        && let Some(pho) = pho_tier
        && xphoint.groups.len() != pho.items.0.len()
    {
        errors.report(
            ParseError::at_span(
                ErrorCode::XphointGroupCountMismatch,
                Severity::Error,
                span,
                format!(
                    "%xphoint has {} group(s) but %pho has {} word(s)",
                    xphoint.groups.len(),
                    pho.items.0.len()
                ),
            )
            .with_suggestion("Emit exactly one ' / '-separated group per %pho word"),
        );
    }

    let mut prev_start: Option<u64> = None;
    let mut first_start: Option<u64> = None;
    let mut last_end: Option<u64> = None;

    for (i, group) in xphoint.groups.iter().enumerate() {
        // E745: group phones reproduce the %pho word.
        if reconstruction_clean {
            let reconstructed: String = group.phones.iter().map(|p| p.phone.as_str()).collect();
            if let Some(expected) = source_word(pho_tier, i)
                && reconstructed != expected
            {
                errors.report(
                    ParseError::at_span(
                        ErrorCode::XphointPhoneReconstructionMismatch,
                        Severity::Error,
                        span,
                        format!(
                            "%xphoint group {} reconstructs to '{}', which does not match the %pho word '{}'",
                            i + 1,
                            reconstructed,
                            expected
                        ),
                    )
                    .with_suggestion("Make the group's phones match the %pho word"),
                );
            }
        }

        for phone in &group.phones {
            let timing = &phone.bullet.timing;
            // E742: start < end.
            if timing.start_ms >= timing.end_ms {
                errors.report(
                    ParseError::at_span(
                        ErrorCode::XphointBulletInvalid,
                        Severity::Error,
                        span,
                        format!(
                            "%xphoint phone '{}' has a bullet with start {} >= end {}",
                            phone.phone.as_str(),
                            timing.start_ms,
                            timing.end_ms
                        ),
                    )
                    .with_suggestion("Each interval's start must be strictly less than its end"),
                );
            }
            // E743: starts are non-decreasing.
            if let Some(prev) = prev_start
                && timing.start_ms < prev
            {
                errors.report(
                    ParseError::at_span(
                        ErrorCode::XphointIntervalNotMonotonic,
                        Severity::Error,
                        span,
                        format!(
                            "%xphoint interval start {} is before the previous start {}",
                            timing.start_ms, prev
                        ),
                    )
                    .with_suggestion("Order intervals so each start is at or after the previous"),
                );
            }
            prev_start = Some(timing.start_ms);
            if first_start.is_none() {
                first_start = Some(timing.start_ms);
            }
            last_end = Some(timing.end_ms);
        }
    }

    // E744: first start / last end fall within the record's media bullet.
    if let (Some(first), Some(last), Some(media)) =
        (first_start, last_end, utterance.media_bullet())
    {
        let media_start = media.timing.start_ms;
        let media_end = media.timing.end_ms;
        let starts_before = first + MEDIA_BOUNDS_TOLERANCE_MS < media_start;
        let ends_after = last > media_end + MEDIA_BOUNDS_TOLERANCE_MS;
        if starts_before || ends_after {
            errors.report(
                ParseError::at_span(
                    ErrorCode::XphointMediaBoundsViolation,
                    Severity::Error,
                    span,
                    format!(
                        "%xphoint interval span {}-{} falls outside the record media bullet {}-{}",
                        first, last, media_start, media_end
                    ),
                )
                .with_suggestion("Keep phone intervals within the *SPK: line's media bullet"),
            );
        }
    }
}
