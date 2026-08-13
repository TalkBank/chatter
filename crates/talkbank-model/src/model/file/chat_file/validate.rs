//! ChatFile-level validation entry points and orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

use std::collections::HashSet;

use super::ChatFile;
use super::transcript_name::TranscriptName;
use crate::validation::{RuleSelection, Validate, ValidationState};
use crate::{ErrorSink, ParseError};
use crate::{Header, Line};

// The file-level header / media / cross-header consistency checks live in a
// sibling submodule to keep this file browseable; they are re-exported by glob
// so `build_validation_context` calls them by their bare names.
mod checks;
use checks::{
    check_cross_header_consistency, check_media_filename_match, check_media_linkage_has_timing,
    check_media_unlinked_has_no_timing, check_separator_trailing_space, check_timing_has_media,
    check_utterance_language_declared, file_uses_ca_mode,
};

fn unknown_alignment_warning(
    alignment_name: &str,
    left_label: &str,
    left_span: crate::Span,
    right_label: &str,
    right_span: crate::Span,
) -> ParseError {
    let location = if !left_span.is_dummy() {
        left_span
    } else if !right_span.is_dummy() {
        right_span
    } else {
        crate::Span::DUMMY
    };

    let mut error = ParseError::new(
        crate::ErrorCode::TierValidationError,
        crate::Severity::Warning,
        crate::SourceLocation::new(location),
        crate::ErrorContext::new("", location.to_range(), ""),
        format!(
            "Tier validation warning: skipped {} alignment because parse provenance is unknown for {} and {}",
            alignment_name, left_label, right_label
        ),
    )
    .with_suggestion(
        "Run parser-backed validation to establish parse provenance before alignment checks",
    );

    if !left_span.is_dummy() {
        error
            .labels
            .push(crate::ErrorLabel::new(left_span, left_label));
    }
    if !right_span.is_dummy() {
        error
            .labels
            .push(crate::ErrorLabel::new(right_span, right_label));
    }

    error
}

/// Build file-level validation context from headers and participant IDs.
///
/// Header-derived settings (languages/options) are computed once and shared
/// across header, utterance, and cross-utterance validators.
/// This prevents repeated header scanning in downstream validation passes.
fn build_validation_context(
    participant_ids: HashSet<crate::model::SpeakerCode>,
    languages: &crate::model::LanguageCodes,
    headers: &[&Header],
    rules: RuleSelection,
) -> crate::validation::ValidationContext {
    let declared_languages = languages.as_slice();
    let default_language = declared_languages.first();

    let ca_mode = file_uses_ca_mode(headers);
    // The `bullets` @Options was removed from CHAT, so no file is ever in
    // bullets mode.
    let bullets_mode = false;
    let enable_quotation_validation = rules.strict_linkers_enabled();

    crate::validation::ValidationContext::from_shared(std::sync::Arc::new(
        crate::validation::SharedValidationData {
            participant_ids,
            default_language: default_language.cloned(),
            declared_languages: declared_languages.to_vec(),
            ca_mode,
            enable_quotation_validation,
            bullets_mode,
        },
    ))
}

/// Shared check sequence run by both [`ChatFile::validate`] and
/// [`ChatFile::validate_with_rules`].
///
/// The two entry points differ only in which [`RuleSelection`] built the
/// `context`; the actual check sequence must stay identical or the two paths
/// silently drift (this is what happened to E758 before the two bodies were
/// unified here: a check present in `validate` was simply absent from the
/// rule-selecting entry point, so `--strict-linkers` runs never reported it).
/// Factoring the sequence into one function makes that class of drift
/// structurally impossible: there is only one place to add a new file-level
/// check.
fn run_validation_checks<S: ValidationState>(
    file: &ChatFile<S>,
    context: &crate::validation::ValidationContext,
    errors: &impl crate::ErrorSink,
    name: TranscriptName<'_>,
) {
    use crate::validation::{cross_utterance, header};

    let headers_with_spans: Vec<(&Header, crate::Span)> = file.headers_with_spans().collect();

    // Validate header collection (duplicates, required headers).
    let source_len = file.lines.last().map(|l| l.span().end as usize);
    header::structure::check_headers(&headers_with_spans, errors, source_len);

    // Validate individual headers.
    for (header, span) in &headers_with_spans {
        header::check_header(header, *span, context, errors);
    }

    // Cross-header validation: @ID language vs @Languages, role mismatch.
    check_cross_header_consistency(file, &headers_with_spans, errors);

    // Validate utterances.
    for utt in file.utterances() {
        utt.validate(context, errors);
    }

    // Validate cross-utterance patterns.
    // The file itself: the checks build their own proved sequence from it, so
    // no caller can hand them one assembled some other way.
    cross_utterance::check_cross_utterance_patterns_with_sink(file, context, errors);

    // E362: Validate bullet timestamp monotonicity across utterances.
    // Skip monotonicity check if bullets mode is enabled.
    let bullets: Vec<&crate::model::Bullet> = file
        .utterances()
        .filter_map(|utt| utt.main.content.bullet.as_ref())
        .collect();
    if !bullets.is_empty() && !context.shared.bullets_mode {
        crate::validation::check_bullet_monotonicity(&bullets, errors);
    }

    // E544: @Media declares linkage but transcript has no timing evidence.
    check_media_linkage_has_timing(&headers_with_spans, file, &bullets, errors);

    // E552: the inverse, @Media declares `unlinked` but the transcript has
    // timing bullets, so the media is in fact linked (CLAN CHECK 124).
    check_media_unlinked_has_no_timing(&headers_with_spans, file, &bullets, errors);

    // E752: timing evidence with NO @Media header at all (CLAN CHECK 112).
    check_timing_has_media(&headers_with_spans, file, &bullets, errors);

    // E755: a [- CODE] utterance language must be declared in @Languages
    // (CLAN CHECK 152); word-level @s:CODE deliberately exempt.
    check_utterance_language_declared(file, errors);

    // E758: leading space between the tab and tier content (CLAN CHECK
    // 123). CA files are exempt.
    if !context.shared.ca_mode {
        check_separator_trailing_space(file, errors);
    }

    // E767: whitespace between the @Media filename and its comma (CLAN
    // CHECK 148). Unconditional, and here rather than in a parser lowering so
    // both front ends report it from one implementation.

    // E701, E704: Validate temporal constraints on media bullets.
    crate::validation::temporal::validate_temporal_constraints(file, errors);

    // E531: the `@Media` filename must match the transcript's own name. Runs
    // only when the caller says the transcript HAS a name; `Anonymous` is a
    // deliberate answer, not a missing one (see `transcript_name`).
    if let Some(stem) = name.stem() {
        check_media_filename_match(&headers_with_spans, stem.as_str(), errors);
    }
}

impl<S: ValidationState> ChatFile<S> {
    /// Run header-only validation and return the derived context.
    ///
    /// Useful for callers that need validated header-derived configuration
    /// before running utterance-level checks.
    pub fn validate_headers_only(
        &self,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> crate::validation::ValidationContext {
        use crate::validation::header;

        let headers_with_spans: Vec<(&Header, crate::Span)> = self.headers_with_spans().collect();
        let headers: Vec<&Header> = headers_with_spans.iter().map(|(h, _)| *h).collect();

        // Extract participant IDs from parsed participant map.
        let participant_ids: HashSet<crate::model::SpeakerCode> =
            self.participants.keys().cloned().collect();

        // Validate header-set invariants (duplicates, required headers).
        let source_len = self.lines.last().map(|l| l.span().end as usize);
        header::structure::check_headers(&headers_with_spans, errors, source_len);

        let context = build_validation_context(
            participant_ids,
            &self.languages,
            &headers,
            RuleSelection::new(),
        );

        // Validate each header payload.
        for (header, span) in &headers_with_spans {
            header::check_header(header, *span, &context, errors);
        }

        // E531: see the sibling call in `run_validation_checks`.
        if let Some(stem) = name.stem() {
            check_media_filename_match(&headers_with_spans, stem.as_str(), errors);
        }

        context
    }

    /// Run tier alignment checks on all utterances, respecting ParseHealth flags.
    ///
    /// Returns any alignment errors found (count mismatches between tiers).
    /// Tainted tiers (from lenient parse error recovery) are skipped to
    /// prevent false positives on pre-existing data quality issues.
    ///
    /// This is a lightweight check intended for use as a pre-serialization gate:
    /// it catches corrupted output (e.g. mismatched %mor/%gra counts) without
    /// running full file-level validation.
    pub fn validate_alignments(&self) -> Vec<ParseError> {
        use crate::alignment::{
            align_main_to_mor, align_main_to_pho, align_main_to_sin, align_mor_to_gra,
        };

        let mut errors = Vec::new();

        for utt in self.utterances() {
            let health = utt.parse_health;

            // Main → %mor alignment
            if health.can_align_main_to_mor()
                && let Some(mor) = utt.mor_tier()
            {
                let alignment = align_main_to_mor(&utt.main, mor);
                errors.extend(alignment.errors);
            } else if health.is_unknown()
                && let Some(mor) = utt.mor_tier()
            {
                errors.push(unknown_alignment_warning(
                    "main↔%mor",
                    "main tier",
                    utt.main.span,
                    "%mor tier",
                    mor.span,
                ));
            }

            // %mor → %gra alignment
            if health.can_align_mor_to_gra()
                && let (Some(mor), Some(gra)) = (utt.mor_tier(), utt.gra_tier())
            {
                let alignment = align_mor_to_gra(mor, gra);
                errors.extend(alignment.errors);
            } else if health.is_unknown()
                && let (Some(mor), Some(gra)) = (utt.mor_tier(), utt.gra_tier())
            {
                errors.push(unknown_alignment_warning(
                    "%mor↔%gra",
                    "%mor tier",
                    mor.span,
                    "%gra tier",
                    gra.span,
                ));
            }

            // Main → %wor alignment is intentionally NOT validated here.
            //
            // `%wor` is a timing sidecar, not a structural alignment, see
            // `WorTimingSidecar`. No validation runs here: drift is a data
            // state, not a diagnostic. Timing-recovery consumers read the
            // sidecar directly via `resolve_wor_timing_sidecar` or
            // `AlignmentSet.wor_timings`.

            // Main → %pho alignment
            if health.can_align_main_to_pho()
                && let Some(pho) = utt.pho_tier()
            {
                let alignment = align_main_to_pho(&utt.main, pho);
                errors.extend(alignment.errors);
            } else if health.is_unknown()
                && let Some(pho) = utt.pho_tier()
            {
                errors.push(unknown_alignment_warning(
                    "main↔%pho",
                    "main tier",
                    utt.main.span,
                    "%pho tier",
                    pho.span,
                ));
            }

            // Main → %sin alignment
            if health.can_align_main_to_sin()
                && let Some(sin) = utt.sin_tier()
            {
                let alignment = align_main_to_sin(&utt.main, sin);
                errors.extend(alignment.errors);
            } else if health.is_unknown()
                && let Some(sin) = utt.sin_tier()
            {
                errors.push(unknown_alignment_warning(
                    "main↔%sin",
                    "main tier",
                    utt.main.span,
                    "%sin tier",
                    sin.span,
                ));
            }
        }

        errors
    }

    /// Validate this CHAT file with streaming error output.
    ///
    /// Errors are reported to the `errors` sink as they're discovered, enabling:
    /// - Early cancellation when user has seen enough errors
    /// - Real-time error display in GUI applications
    /// - Memory-efficient processing of large files
    ///
    /// # Parameters
    ///
    /// * `errors` - Error sink for streaming validation errors
    /// * `name` - What the transcript is called, or `TranscriptName::Anonymous`
    ///   when it has no name. Decides whether E531 (`@Media` filename match) runs.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use talkbank_model::{ChatFile, ErrorCollector, ErrorSink};
    ///
    /// let sink = ErrorCollector::new();
    /// chat_file.validate(&sink, Some("myfile"));
    /// let errors = sink.into_vec();
    /// ```
    #[tracing::instrument(skip(self, errors), fields(lines = self.lines.len()))]
    pub fn validate(&self, errors: &impl crate::ErrorSink, name: TranscriptName<'_>) {
        let header_count = self.header_count();
        let utterance_count = self.utterance_count();
        tracing::debug!(
            "Validating CHAT file ({} headers, {} utterances) with streaming",
            header_count,
            utterance_count
        );

        let headers: Vec<&Header> = self.headers().collect();
        let participant_ids: HashSet<crate::model::SpeakerCode> =
            self.participants.keys().cloned().collect();
        let context = build_validation_context(
            participant_ids,
            &self.languages,
            &headers,
            RuleSelection::new(),
        );

        run_validation_checks(self, &context, errors, name);

        tracing::debug!("Streaming validation complete");
    }

    /// Validate this CHAT file under an explicit [`RuleSelection`].
    ///
    /// Reports the COMPLETE diagnostic set for that rule selection: every
    /// diagnostic the selected rules produced, at the severity the validator
    /// assigned. Nothing is filtered or re-labelled here.
    ///
    /// # Why no suppression happens at this seam
    ///
    /// Deciding what a reader sees is a separate step, applied to these
    /// diagnostics afterwards (`talkbank_transform::PresentationPolicy`). Doing
    /// it here would make the outcome of validation depend on a display
    /// preference, which is how a `--suppress` list ended up in the validation
    /// cache key in v0.6.0 and partitioned the cache per suppression set. What
    /// a caller may cache, or count, is what this function reports.
    ///
    /// # Parameters
    ///
    /// * `rules` - Which validation rules to run
    /// * `errors` - Error sink for streaming validation errors
    /// * `name` - What the transcript is called, or `TranscriptName::Anonymous`
    ///   when it has no name. Decides whether E531 (`@Media` filename match) runs.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use talkbank_model::{ChatFile, ErrorCollector, RuleSelection};
    ///
    /// let errors = ErrorCollector::new();
    /// chat_file.validate_with_rules(
    ///     RuleSelection::new().with_strict_linkers(),
    ///     &errors,
    ///     Some("myfile"),
    /// );
    /// ```
    #[tracing::instrument(skip(self, errors), fields(lines = self.lines.len()))]
    pub fn validate_with_rules(
        &self,
        rules: RuleSelection,
        errors: &impl crate::ErrorSink,
        name: TranscriptName<'_>,
    ) {
        let header_count = self.header_count();
        let utterance_count = self.utterance_count();
        tracing::debug!(
            "Validating CHAT file ({} headers, {} utterances) with an explicit rule selection",
            header_count,
            utterance_count
        );

        let headers: Vec<&Header> = self.headers().collect();
        let participant_ids: HashSet<crate::model::SpeakerCode> =
            self.participants.keys().cloned().collect();
        let context = build_validation_context(participant_ids, &self.languages, &headers, rules);

        run_validation_checks(self, &context, errors, name);

        tracing::debug!("Streaming validation with rule selection complete");
    }

    /// Precompute per-utterance tier alignment and language metadata.
    ///
    /// Shared by [`Self::validate_with_alignment`] and
    /// [`Self::validate_with_alignment_and_rules`]. Alignment computation only
    /// needs the header-derived default/declared languages, which no rule
    /// selection changes, so both callers precompute with a fresh default
    /// [`RuleSelection`] regardless of which one the validation pass uses.
    fn precompute_alignments(&mut self) {
        let utterance_count = self.utterance_count();
        tracing::debug!(
            "Computing tier alignments for {} utterances",
            utterance_count
        );

        // Build shared context once for metadata precomputation.
        let headers: Vec<&Header> = self.headers().collect();
        let participant_ids: HashSet<crate::model::SpeakerCode> =
            self.participants.keys().cloned().collect();
        let context = build_validation_context(
            participant_ids,
            &self.languages,
            &headers,
            RuleSelection::new(),
        );

        let default_language = context.shared.default_language.as_ref();
        let declared_languages = context.shared.declared_languages.as_slice();

        // Compute alignment and language metadata for all utterances.
        for line in &mut self.lines {
            if let Line::Utterance(utterance) = line {
                utterance.compute_alignments(&context);
                utterance.compute_language_metadata(default_language, declared_languages);
            }
        }

        tracing::debug!("Tier alignments computed");
    }

    /// Validate this CHAT file including alignment/language precomputation.
    ///
    /// This first computes per-utterance alignment and language metadata, then
    /// runs the normal streaming validation pipeline.
    ///
    /// # Parameters
    ///
    /// * `errors` - Error sink for streaming validation errors
    /// * `name` - What the transcript is called, or `TranscriptName::Anonymous`
    ///   when it has no name. Decides whether E531 (`@Media` filename match) runs.
    #[tracing::instrument(skip(self, errors), fields(lines = self.lines.len()))]
    pub fn validate_with_alignment(
        &mut self,
        errors: &impl crate::ErrorSink,
        name: TranscriptName<'_>,
    ) {
        self.precompute_alignments();
        tracing::debug!("running streaming validation");
        self.validate(errors, name)
    }

    /// Validate this CHAT file with alignment/language precomputation AND an
    /// explicit [`RuleSelection`].
    ///
    /// This is the combination [`Self::validate_with_alignment`] cannot express
    /// (it always uses the default rule selection) and
    /// [`Self::validate_with_rules`] cannot express (it never precomputes
    /// alignment): both alignment-aware diagnostics AND opt-in rules apply.
    /// Streamed validation runners use this entry point so there is exactly one
    /// validation call per file regardless of which options are active.
    ///
    /// # Parameters
    ///
    /// * `rules` - Which validation rules to run
    /// * `errors` - Error sink for streaming validation errors
    /// * `name` - What the transcript is called, or `TranscriptName::Anonymous`
    ///   when it has no name. Decides whether E531 (`@Media` filename match) runs.
    #[tracing::instrument(skip(self, errors), fields(lines = self.lines.len()))]
    pub fn validate_with_alignment_and_rules(
        &mut self,
        rules: RuleSelection,
        errors: &impl crate::ErrorSink,
        name: TranscriptName<'_>,
    ) {
        self.precompute_alignments();
        tracing::debug!("running streaming validation with an explicit rule selection");
        self.validate_with_rules(rules, errors, name)
    }
}

// Implement Validate trait for ChatFile (all states)
impl<S: ValidationState> Validate for ChatFile<S> {
    /// Delegates trait-based validation to full ChatFile validation pipeline.
    fn validate(&self, _context: &crate::validation::ValidationContext, errors: &impl ErrorSink) {
        // The `Validate` trait has no room for a name, so this path is
        // genuinely anonymous and says so: rules about the transcript's own
        // file name (E531) do not run through the trait. A caller that has a
        // name calls `ChatFile::validate` directly.
        self.validate(errors, TranscriptName::Anonymous);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use crate::model::{
        GraTier, GrammaticalRelation, Header, LanguageCode, MainTier, Mor, MorTier, MorWord,
        PosCategory, Terminator, Utterance, UtteranceContent, Word,
    };

    /// Build a minimal ChatFile wrapping one utterance.
    fn chat_with_utterance(utt: Utterance) -> ChatFile {
        ChatFile::new(vec![
            Line::header(Header::Utf8),
            Line::header(Header::Begin),
            Line::header(Header::Languages {
                codes: vec![LanguageCode::new("eng").expect("test literal is non-empty")].into(),
            }),
            Line::utterance(utt),
            Line::header(Header::End),
        ])
    }

    /// Builds a minimal main tier from word strings.
    fn simple_main_tier(words: &[&str]) -> MainTier {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::new_unchecked(*w, *w))))
            .collect();
        MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY })
    }

    /// Builds a minimal `%mor` tier from `(pos, lemma)` tuples.
    fn simple_mor_tier(items: &[(&str, &str)]) -> MorTier {
        let mors: Vec<Mor> = items
            .iter()
            .map(|(pos, lemma)| Mor::new(MorWord::new(PosCategory::new(*pos), *lemma)))
            .collect();
        MorTier::new_mor(
            mors,
            crate::Terminator::Period {
                span: crate::Span::DUMMY,
            },
        )
    }

    /// Builds a synthetic `%gra` tier with `count` relations.
    fn simple_gra_tier(count: usize) -> GraTier {
        let mut rels = Vec::new();
        for i in 0..count {
            if i == 0 {
                rels.push(GrammaticalRelation::new(1, 0, "ROOT"));
            } else {
                rels.push(GrammaticalRelation::new(i + 1, 1, "MOD"));
            }
        }
        GraTier::new_gra(rels)
    }

    /// Alignment check passes when `%mor`/`%gra` cardinalities are consistent.
    #[test]
    fn validate_alignments_no_errors_for_matching_tiers() {
        let main = simple_main_tier(&["I", "go"]);
        let mor = simple_mor_tier(&[("pro", "I"), ("v", "go")]);
        // 2 words + terminator = 3 mor chunks → need 3 gra relations
        let gra = simple_gra_tier(3);
        let utt = Utterance::new(main).with_mor(mor).with_gra(gra);
        let chat = chat_with_utterance(utt);

        let errors = chat.validate_alignments();
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    /// Alignment check reports mismatch when `%gra` has too few relations.
    #[test]
    fn validate_alignments_catches_mor_gra_mismatch() {
        let main = simple_main_tier(&["I", "go"]);
        let mor = simple_mor_tier(&[("pro", "I"), ("v", "go")]);
        // Intentionally wrong: 2 gra relations for 3 mor chunks (2 words + terminator)
        let gra = simple_gra_tier(2);
        let utt = Utterance::new(main).with_mor(mor).with_gra(gra);
        let chat = chat_with_utterance(utt);

        let errors = chat.validate_alignments();
        assert!(
            !errors.is_empty(),
            "Expected alignment errors for mor/gra mismatch"
        );
    }

    /// Tainted tier domains are skipped during alignment validation.
    #[test]
    fn validate_alignments_skips_tainted_tiers() {
        use crate::model::ParseHealthTier;

        let main = simple_main_tier(&["I", "go"]);
        let mor = simple_mor_tier(&[("pro", "I"), ("v", "go")]);
        // Intentionally wrong: 2 gra relations for 3 mor chunks (2 words + terminator)
        let gra = simple_gra_tier(2);

        let mut utt = Utterance::new(main).with_mor(mor).with_gra(gra);
        // Taint the gra tier, validation should skip mor→gra check
        utt.mark_parse_taint(ParseHealthTier::Gra);
        let chat = chat_with_utterance(utt);

        let errors = chat.validate_alignments();
        // Mor→gra check is skipped because gra is tainted, so no errors from that check.
        // Main→mor is still checked but should pass (2 words, 2 mor items).
        assert!(
            errors.is_empty(),
            "Expected no errors when gra is tainted, got: {:?}",
            errors
        );
    }

    /// Alignment check reports mismatch when main-word and `%mor` counts diverge.
    #[test]
    fn validate_alignments_catches_main_mor_mismatch() {
        // 3 words but only 2 mor items
        let main = simple_main_tier(&["I", "go", "home"]);
        let mor = simple_mor_tier(&[("pro", "I"), ("v", "go")]);
        let utt = Utterance::new(main).with_mor(mor);
        let chat = chat_with_utterance(utt);

        let errors = chat.validate_alignments();
        assert!(
            !errors.is_empty(),
            "Expected alignment errors for main/mor mismatch"
        );
    }

    /// An out-of-bounds `%gra` head should surface as E713 without cascading into
    /// additional root/cycle diagnostics from structural validation.
    #[test]
    fn validate_with_alignment_out_of_bounds_head_does_not_cascade_structure_errors() {
        let main = simple_main_tier(&["I", "go"]);
        let mor = simple_mor_tier(&[("pro", "I"), ("v", "go")]);
        let gra = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 5, "DEP"),
            GrammaticalRelation::new(2, 1, "OBJ"),
            GrammaticalRelation::new(3, 2, "PUNCT"),
        ]);
        let utt = Utterance::new(main).with_mor(mor).with_gra(gra);
        let mut chat = chat_with_utterance(utt);

        let errs = crate::validate_chat_file_with_options(
            &mut chat,
            &crate::ParseValidateOptions::default().with_alignment(),
        )
        .expect_err("out-of-bounds gra head must fail validation");

        assert!(
            errs.iter()
                .any(|e| e.code == crate::ErrorCode::GraInvalidHeadIndex),
            "out-of-bounds head must report E713"
        );
        assert!(
            !errs.iter().any(|e| e.code == crate::ErrorCode::GraNoRoot),
            "E713 must not cascade into E722"
        );
        assert!(
            !errs
                .iter()
                .any(|e| e.code == crate::ErrorCode::GraCircularDependency),
            "E713 must not cascade into E724"
        );
    }
}
