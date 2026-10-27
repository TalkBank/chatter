//! ChatFile-level validation entry points and orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

use super::ChatFile;
use super::transcript_name::TranscriptName;
use crate::alignment::unknown_alignment_warning;
use crate::validation::{RuleSelection, Validate};
use crate::{ErrorSink, ParseError};
use crate::{Header, Line};

// The file-level header / media / cross-header consistency checks live in a
// sibling submodule to keep this file browseable; they are re-exported by glob
// so `build_validation_context` calls them by their bare names.
mod checks;
use checks::{
    check_cross_header_consistency, check_media_filename_match, check_media_linkage_has_timing,
    check_media_unlinked_has_no_timing, check_separator_trailing_space, check_timing_has_media,
    check_utterance_language_declared, file_uses_ca_mode, report_missing_media_timing,
};

/// What the check sequence does with a linked-media declaration that has no
/// timing evidence (the E544 condition), and what it hands back for it.
///
/// The phase is a type, so each caller receives exactly what its phase
/// produces: complete validation reports E544 and returns nothing, while
/// timing regeneration reports nothing and returns the declaration as an
/// outstanding obligation. Neither can drop the other's result.
trait MediaTimingPhase {
    /// What the check sequence returns under this phase.
    type Outcome;
    /// Settle the E544 observation: `untimed` is the declaration's span when
    /// linked media has no timing evidence. `errors` is the sequence's own
    /// sink, so a finding cannot go anywhere else.
    fn settle(self, untimed: Option<crate::Location>, errors: &impl ErrorSink) -> Self::Outcome;
}

/// Complete admission: an untimed linked-media declaration is E544.
struct CompleteMediaTiming;

impl MediaTimingPhase for CompleteMediaTiming {
    type Outcome = ();
    fn settle(self, untimed: Option<crate::Location>, errors: &impl ErrorSink) {
        if let Some(location) = untimed {
            report_missing_media_timing(location, errors);
        }
    }
}

/// Timing regeneration: the declaration is an obligation, not an error.
struct RegeneratingMediaTiming;

impl MediaTimingPhase for RegeneratingMediaTiming {
    type Outcome = Option<crate::Location>;
    /// Reports nothing: the declaration is handed back, not judged.
    fn settle(
        self,
        untimed: Option<crate::Location>,
        _errors: &impl ErrorSink,
    ) -> Option<crate::Location> {
        untimed
    }
}

/// Build file-level validation context from current declaration headers.
///
/// Header-derived settings (languages/options) are computed once and shared
/// across header, utterance, and cross-utterance validators.
/// This prevents repeated header scanning in downstream validation passes.
fn build_validation_context(
    languages: Result<
        crate::model::DeclaredLanguagesRef<'_>,
        crate::model::LanguageDeclarationError,
    >,
    headers: &[&Header],
    rules: RuleSelection,
) -> crate::validation::ValidationContext {
    let default_language = match languages {
        Ok(codes) => Some(codes.primary().clone()),
        Err(_) => None,
    };
    let declared_languages = languages.map(crate::model::DeclaredLanguagesRef::to_owned);

    let ca_mode = file_uses_ca_mode(headers);
    let enable_quotation_validation = rules.strict_linkers_enabled();

    crate::validation::ValidationContext::from_shared(std::sync::Arc::new(
        crate::validation::SharedValidationData {
            participant_roster: crate::model::ParticipantRoster::from_headers(
                headers.iter().copied(),
            ),
            default_language,
            declared_languages,
            ca_mode,
            enable_quotation_validation,
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
fn run_validation_checks<P: MediaTimingPhase>(
    file: &ChatFile,
    context: &crate::validation::ValidationContext,
    errors: &impl crate::ErrorSink,
    name: TranscriptName<'_>,
    timing: P,
) -> P::Outcome {
    use crate::validation::cross_utterance;

    let headers_with_locations: Vec<(&Header, crate::Location)> =
        file.headers_with_locations().collect();

    // The header half, shared with `validate_headers_only`.
    run_header_checks(file, &headers_with_locations, context, errors, name);

    // Cross-header validation: @ID language vs @Languages, role mismatch.
    check_cross_header_consistency(file, &headers_with_locations, errors);

    // Validate utterances.
    for utt in file.utterances() {
        utt.validate(context, errors);
    }

    // Validate cross-utterance patterns.
    // The file itself: the checks build their own proved sequence from it, so
    // no caller can hand them one assembled some other way.
    cross_utterance::check_cross_utterance_patterns_with_sink(file, context, errors);

    // E362: Validate bullet timestamp monotonicity across utterances. The
    // `bullets` @Options that once switched this off was removed from CHAT;
    // the flag that remembered it was always false and went on 2026-09-08.
    let bullets: Vec<&crate::model::Bullet> = file
        .utterances()
        .filter_map(|utt| {
            utt.main
                .content
                .bullet
                .as_ref()
                .and_then(crate::model::BulletState::as_parsed)
        })
        .collect();
    if !bullets.is_empty() {
        crate::validation::check_bullet_monotonicity(&bullets, errors);
    }

    // The media-consistency family reads ONE union of main-tier timing
    // evidence: utterance-final bullets and bullets INSIDE an utterance
    // (`InternalBullet` items, at any depth). Until 2026-09-08 only the final
    // bullets counted, so `hello \u{15}100_200\u{15} world .` passed with no
    // @Media and became E544 once one was declared; CLAN CHECK 112 fires on
    // it, and a timestamp is a timestamp wherever it sits (maintainer
    // ruling, 2026-09-08). E362's monotonicity check above stays on the
    // final bullets alone.
    let timing_bullets = main_tier_timing_bullets(file);
    // E544: @Media declares linkage but transcript has no timing evidence.
    // Rejected timing is not proof that the source has no timing. Do not
    // cascade a missing-timing diagnostic from failed numeric admission.
    let untimed_media = if file
        .utterances()
        .any(|utt| utt.main.content.has_unparsed_bullets())
    {
        None
    } else {
        check_media_linkage_has_timing(
            &headers_with_locations,
            file,
            timing_bullets.first().copied(),
        )
    };
    let outcome = timing.settle(untimed_media, errors);

    // E552: the inverse, @Media declares `unlinked` but the transcript has
    // timing bullets, so the media is in fact linked (CLAN CHECK 124).
    check_media_unlinked_has_no_timing(&headers_with_locations, file, &timing_bullets, errors);

    // E752: timing evidence with NO @Media header at all (CLAN CHECK 112).
    check_timing_has_media(&headers_with_locations, file, &timing_bullets, errors);

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
    outcome
}

/// The header half of validation: the header set (duplicates, required
/// headers), every header's payload through the per-header dispatcher, and
/// the transcript-name rule. [`ChatFile::validate_headers_only`] is this and
/// nothing else; [`run_validation_checks`] is this, then the cross-header,
/// utterance, cross-utterance, bullet, media and temporal checks. Until
/// 2026-09-08 the two entry points each wrote this sequence out, and a test
/// held them equal; one body makes a header rule reachable from the
/// header-only entry point exactly when it is reachable from full
/// validation.
fn run_header_checks(
    file: &ChatFile,
    headers_with_locations: &[(&Header, crate::Location)],
    context: &crate::validation::ValidationContext,
    errors: &impl crate::ErrorSink,
    name: TranscriptName<'_>,
) {
    use crate::validation::header;

    // Validate header collection (duplicates, required headers). The end of
    // the file is the end of its last located line; a file with no located
    // lines (programmatic or read back from JSON) ends at 0.
    let source_len = file
        .lines
        .iter()
        .rev()
        .find_map(|line| line.location().span())
        .map_or(0, |span| span.end());
    header::structure::check_headers(headers_with_locations, errors, source_len);
    // JSON and programmatic models do not retain parser-side findings. Reuse
    // the same join policy without constructing a second participant cache.
    crate::model::participant::join::build_participants_from_lines(&file.lines)
        .report_diagnostics(errors);

    // Validate individual headers.
    for (header, location) in headers_with_locations {
        header::check_header(header, *location, context, errors);
    }

    // E531: the `@Media` filename must match the transcript's own name. Runs
    // only when the caller says the transcript HAS a name; `Anonymous` is a
    // deliberate answer, not a missing one (see `transcript_name`).
    if let Some(stem) = name.stem() {
        check_media_filename_match(headers_with_locations, stem.as_str(), errors);
    }
}

impl ChatFile {
    /// Run header-only validation and return the derived context.
    ///
    /// Useful for callers that need validated header-derived configuration
    /// before running utterance-level checks. This is the header half of
    /// [`Self::validate`], `run_header_checks`, and nothing else, so a
    /// header rule full validation reports is one this reports. (The LSP
    /// validates through `validate_with_alignment` today; this entry point
    /// is public API with no in-tree caller but its tests.)
    pub fn validate_headers_only(
        &self,
        errors: &impl ErrorSink,
        name: TranscriptName<'_>,
    ) -> crate::validation::ValidationContext {
        let headers_with_locations: Vec<(&Header, crate::Location)> =
            self.headers_with_locations().collect();
        let headers: Vec<&Header> = headers_with_locations.iter().map(|(h, _)| *h).collect();

        let context =
            build_validation_context(self.language_declaration(), &headers, RuleSelection::new());

        run_header_checks(self, &headers_with_locations, &context, errors, name);

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
            let health = utt.parse_health();

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
                    utt.main.location,
                    "%mor tier",
                    Some(mor.location),
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
                    mor.location,
                    "%gra tier",
                    Some(gra.location),
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
                    utt.main.location,
                    "%pho tier",
                    Some(pho.location),
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
                    utt.main.location,
                    "%sin tier",
                    Some(sin.location),
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
    /// ```
    /// use talkbank_model::{ChatFile, ErrorCollector};
    /// use talkbank_model::model::TranscriptName;
    /// # fn inspect(chat_file: &ChatFile) {
    ///
    /// let sink = ErrorCollector::new();
    /// let path = std::path::Path::new("corpus/myfile.cha");
    /// chat_file.validate(&sink, TranscriptName::for_path(path));
    /// let errors = sink.into_vec();
    /// # }
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
        let context =
            build_validation_context(self.language_declaration(), &headers, RuleSelection::new());

        run_validation_checks(self, &context, errors, name, CompleteMediaTiming);

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
    /// ```
    /// use talkbank_model::{ChatFile, ErrorCollector, RuleSelection};
    /// use talkbank_model::model::TranscriptName;
    /// # fn inspect(chat_file: &ChatFile) {
    ///
    /// let errors = ErrorCollector::new();
    /// chat_file.validate_with_rules(
    ///     RuleSelection::new().with_strict_linkers(),
    ///     &errors,
    ///     TranscriptName::Anonymous,
    /// );
    /// # }
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
        let context = build_validation_context(self.language_declaration(), &headers, rules);

        run_validation_checks(self, &context, errors, name, CompleteMediaTiming);

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
        let context =
            build_validation_context(self.language_declaration(), &headers, RuleSelection::new());

        let default_language = context.shared.default_language.as_ref();
        let declared_languages = context
            .shared
            .declared_languages
            .as_ref()
            .map(crate::model::DeclaredLanguages::as_ref)
            .map_err(|error| *error);

        // Compute alignment and language metadata for all utterances.
        for line in &mut self.lines {
            if let Line::Utterance(utterance) = line {
                utterance.compute_alignments(&context);
                utterance.compute_language_metadata_with_declaration(
                    default_language,
                    declared_languages,
                );
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

    /// Validate under a [`ValidationPolicy`](crate::validation::ValidationPolicy):
    /// its rules, at its alignment coverage. The one place a coverage value
    /// becomes a validator call, so every caller holding a policy (the
    /// validation runner, the proof-carrying `validate_with_policy`, `fix`)
    /// runs the same checks for the same value.
    pub fn validate_at(
        &mut self,
        policy: crate::validation::ValidationPolicy,
        errors: &impl crate::ErrorSink,
        name: TranscriptName<'_>,
    ) {
        match policy.alignment() {
            crate::validation::AlignmentValidation::Structure => {
                self.validate_with_rules(policy.rules(), errors, name)
            }
            crate::validation::AlignmentValidation::IncludeTierAlignment => {
                self.validate_with_alignment_and_rules(policy.rules(), errors, name)
            }
        }
    }

    /// The regeneration phase runs the complete default rule/alignment set,
    /// but returns an outstanding timing requirement instead of claiming that
    /// linked media without timing is a complete document.
    pub(crate) fn validate_regenerating_timing(
        &mut self,
        errors: &impl crate::ErrorSink,
        name: TranscriptName<'_>,
    ) -> Option<crate::Location> {
        self.precompute_alignments();
        let headers: Vec<&Header> = self.headers().collect();
        let context =
            build_validation_context(self.language_declaration(), &headers, RuleSelection::new());
        run_validation_checks(self, &context, errors, name, RegeneratingMediaTiming)
    }
}

// Implement Validate trait for ChatFile (all states)
impl Validate for ChatFile {
    /// Delegates trait-based validation to full ChatFile validation pipeline.
    fn validate(&self, _context: &crate::validation::ValidationContext, errors: &impl ErrorSink) {
        // The `Validate` trait has no room for a name, so this path is
        // genuinely anonymous and says so: rules about the transcript's own
        // file name (E531) do not run through the trait. A caller that has a
        // name calls `ChatFile::validate` directly.
        self.validate(errors, TranscriptName::Anonymous);
    }
}

/// Every main-tier bullet in document order: the utterance-final bullet and
/// every bullet inside the utterance, at any depth, which is the timing
/// evidence the media-consistency family (E544, E552, E752) reads.
fn main_tier_timing_bullets(file: &ChatFile) -> Vec<&crate::model::Bullet> {
    let mut bullets = Vec::new();
    visit_main_tier_timing_bullets(file, &mut |bullet| bullets.push(bullet));
    bullets
}

/// Every main-tier bullet of the file in document order.
fn visit_main_tier_timing_bullets<'file>(
    file: &'file ChatFile,
    visit: &mut impl FnMut(&'file crate::model::Bullet),
) {
    for utt in file.utterances() {
        visit_utterance_timing_bullets(utt, visit);
    }
}

/// The file's first main-tier bullet in document order. The search stops at
/// the first utterance carrying one; only that utterance's content is walked
/// to completion, and no bullet collection is allocated.
fn first_main_tier_timing_bullet(file: &ChatFile) -> Option<&crate::model::Bullet> {
    file.utterances().find_map(|utt| {
        let mut first = None;
        visit_utterance_timing_bullets(utt, &mut |bullet| {
            first.get_or_insert(bullet);
        });
        first
    })
}

/// One traversal owner for one utterance's final and recursively nested
/// main-tier bullets: the internal ones in content order, then the final one.
fn visit_utterance_timing_bullets<'file>(
    utt: &'file crate::model::Utterance,
    visit: &mut impl FnMut(&'file crate::model::Bullet),
) {
    use crate::alignment::helpers::{ContentItem, walk_content};
    walk_content(&utt.main.content.content, None, &mut |item| {
        if let ContentItem::InternalBullet(bullet) = item
            && let Some(bullet) = bullet.as_parsed()
        {
            visit(bullet);
        }
    });
    if let Some(bullet) = utt.main.content.bullet.as_ref()
        && let Some(bullet) = bullet.as_parsed()
    {
        visit(bullet);
    }
}

impl ChatFile {
    /// The linked-media declaration E544 would report, if any: an `@Media`
    /// expecting a recording, with no status, in a document with no timing
    /// evidence. Shared by complete validation and by
    /// [`PendingTimingChatFile::discharge`](crate::validation::PendingTimingChatFile::discharge).
    pub(crate) fn untimed_media_declaration(&self) -> Option<crate::Location> {
        let headers_with_locations: Vec<(&Header, crate::Location)> =
            self.headers_with_locations().collect();
        check_media_linkage_has_timing(
            &headers_with_locations,
            self,
            first_main_tier_timing_bullet(self),
        )
    }

    /// Observe timing presence using the same owner as E544. This does not
    /// validate the interval, document, recovery state or media declaration,
    /// and cannot authorize output. The witness borrows this exact structure.
    /// The search stops at the first utterance carrying a main-tier bullet.
    pub fn timing_evidence(&self) -> super::TranscriptTimingEvidence<'_> {
        checks::observe_transcript_timing(self, first_main_tier_timing_bullet(self))
    }
}
