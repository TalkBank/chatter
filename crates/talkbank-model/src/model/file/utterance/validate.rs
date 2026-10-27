//! Utterance-scoped validation orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//!
//! This module acts as the canonical entrypoint for main-tier + dependent-tier
//! validation for a single utterance so callers (for example parsers or aligners)
//! can retain a single `ValidationContext`.

use super::Utterance;
use crate::ErrorSink;
use crate::model::ValidationTagged;
use crate::model::dependent_tier::DependentTier;
use crate::validation::Validate;
use crate::validation::ValidationContext;

impl Utterance {
    /// Report the `%gra` structural rules for every `%gra` tier of THIS
    /// utterance.
    ///
    /// The utterance owns the tier and its alignment evidence. Analyze the
    /// unchanged tier once, then consume those facts into whole-tier admission.
    fn validate_gra_structure(&self, errors: &impl ErrorSink) {
        for entry in &self.dependent_tiers {
            let DependentTier::Gra(tier) = &entry.tier else {
                continue;
            };
            // Always: dropping a relation cannot create a second root or close
            // a cycle, so a violation among the survivors is one the author
            // wrote.
            let structure = crate::model::dependent_tier::GraStructure::analyze(tier);
            structure.report_monotone(errors);
            // Only on a tier established whole and undisputed: dropping breaks
            // sequentiality by itself, and can remove the only root.
            if let Some(whole) = structure.into_whole(&self.alignment_diagnostics) {
                whole.validate_whole_tier(errors);
            }
        }
    }
}

impl Validate for Utterance {
    /// Performs utterance-local validation checks and reports diagnostics.
    ///
    /// This pass handles speaker consistency, main-tier structure, dependent-tier
    /// invariants, and already-computed alignment diagnostics for the utterance.
    /// Rule ordering is deliberate: foundational parse/speaker issues are
    /// reported first so later tier-level diagnostics have clearer context.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        // Membership is declaration evidence, independent of ID enrichment.
        if context
            .shared
            .participant_roster
            .membership(&self.main.speaker)
            == crate::model::SpeakerMembership::Undeclared
        {
            let speaker_str = self.main.speaker.as_str();
            errors.report(
                crate::ParseError::at_location(
                    crate::ErrorCode::SpeakerNotDefined,
                    crate::Severity::Error,
                    self.main.speaker_location,
                    format!(
                        "Speaker '{}' used on main tier but not declared in @Participants",
                        speaker_str
                    ),
                )
                .with_suggestion(format!(
                    "Add '{}' to the @Participants line, or correct the speaker code on this utterance",
                    speaker_str
                )),
            );
        }

        // Main-tier lexical and structural checks. The context learns
        // whether THIS utterance's main tier was parse-recovered, so
        // content-structure claims (E305/E306) do not fire over content the
        // parser never materialized.
        let main_context = context.clone().with_main_parse_taint(
            self.parse_health()
                .is_tier_tainted(crate::model::ParseHealthTier::Main),
        );
        self.main.validate(&main_context, errors);

        // E356, E357: Validate underline markers are balanced
        crate::validation::utterance::check_underline_balance(self, errors);

        // E230: Validate CA delimiter balance
        crate::validation::utterance::check_ca_delimiter_balance(self, errors);
        // E753: a word consisting only of a repetition segment (fully
        // ↫...↫-wrapped, no stem) is invalid; see the module docs.
        crate::validation::utterance::check_repetition_segment_has_stem(self, errors);

        // E258: Validate no consecutive commas in document order
        crate::validation::utterance::check_consecutive_commas(self, errors);
        crate::validation::utterance::check_semicolon_separators(self, errors);

        // E259: Validate commas are not preceded by non-spoken content
        crate::validation::utterance::check_comma_after_non_spoken(self, errors);

        // E749: comma glued to the following word (CLAN CHECK 92).
        crate::validation::utterance::check_comma_glued_to_next(self, errors);

        // E751: pause glued to the preceding word (CLAN CHECK 57).
        crate::validation::utterance::check_pause_glued_to_word(self, errors);
        crate::validation::utterance::check_code_glued_to_following_content(self, errors);
        crate::validation::utterance::check_prefixed_form_glued_to_preceding_word(self, errors);
        crate::validation::utterance::check_separator_glued_to_following_content(self, errors);

        // E401: Validate no duplicate dependent tiers
        crate::validation::utterance::check_no_duplicate_dependent_tiers(self, errors);

        // E761: `%gra` relation heads must be UD universal relations.
        // Independent of the structural and alignment checks below: a label's
        // legality does not depend on tree shape or on `%mor` cardinality.
        crate::validation::utterance::check_gra_relation_vocabulary(self, errors);

        // E604: `%gra` requires `%mor`.
        // Unparsed MOR is present but cannot participate in alignment. Do not
        // cascade a missing-tier diagnosis from its malformed content.
        let has_gra = self.dependent_tiers.iter().any(|entry| {
            matches!(
                &entry.tier,
                crate::model::dependent_tier::DependentTier::Gra(_)
                    | crate::model::dependent_tier::DependentTier::UnparsedGra(_)
            )
        });
        let has_mor = self.dependent_tiers.iter().any(|entry| {
            matches!(
                &entry.tier,
                crate::model::dependent_tier::DependentTier::Mor(_)
                    | crate::model::dependent_tier::DependentTier::UnparsedMor(_)
            )
        });
        let mor_tainted = self
            .parse_health()
            .is_tier_tainted(crate::model::ParseHealthTier::Mor);
        if has_gra && !has_mor && !mor_tainted {
            errors.report(
                crate::ParseError::at_location(
                    crate::ErrorCode::GraWithoutMor,
                    crate::Severity::Error,
                    self.main.location,
                    "%gra tier requires %mor tier to be present",
                )
                .with_suggestion("Add %mor tier before %gra, or remove %gra tier"),
            );
        }

        // E721-E724: the structural rules for `%gra`, split by what a dropped
        // relation can do to them. `WholeGra` carries the reasoning and the
        // cascade this replaced.
        self.validate_gra_structure(errors);

        // Tier-local content and interval invariants, independent of alignment:
        // E711 `%mor` content; E362 for a `%wor` bullet that ends at or before
        // it starts. An absent bullet remains legitimate partial timing.
        for entry in &self.dependent_tiers {
            match &entry.tier {
                DependentTier::Mor(marker) => marker.validate_content(errors),
                DependentTier::Wor(tier) => tier.validate_word_intervals(errors),
                _ => {}
            }
        }

        // E735-E746: Phon `%x` dependent-tier content validation (syllabification
        // codes/reconstruction, phone alignment, per-phone interval timing).
        crate::validation::utterance::check_phon_xtiers(self, errors);

        // Validate tier alignment diagnostics collected during alignment computation.
        if !self.alignment_diagnostics.is_empty() {
            errors.report_all(self.alignment_diagnostics.clone());
        }

        // E756: any dependent tier that declares no content, not just `%x*`.
        //
        // The kind-specific question ("can this tier even be empty?") belongs
        // to `DependentTier::empty_content_location`, whose match is exhaustive, so
        // a new tier variant stops compiling there rather than silently
        // escaping this rule. This loop only asks.
        for entry in &self.dependent_tiers {
            if let Some(recovery) = entry.tier.unparsed_recovery() {
                errors.report(crate::ParseError::at_location(
                    crate::ErrorCode::UnparsableContent,
                    crate::Severity::Error,
                    recovery.raw().location(),
                    format!(
                        "%{} source is unparsed; repair or regenerate this tier before validation",
                        entry.kind()
                    ),
                ));
            }
            if let Some((content, location)) = entry.tier.bullet_content() {
                let context = context
                    .clone()
                    .with_field_location(location.or(context.field_location));
                content.segments.validate(&context, errors);
            }
            if let DependentTier::Sin(tier) = &entry.tier {
                tier.validate(context, errors);
            }
            if let Some(location) = entry.tier.empty_content_location() {
                crate::validation::check_dependent_tier_content(entry.kind(), location, errors);
            }
        }

        // E603: Validate %tim tier content format.
        for entry in &self.dependent_tiers {
            if let DependentTier::Tim(tim_tier) = &entry.tier
                && tim_tier.has_validation_issue()
            {
                check_tim_tier_format(tim_tier.as_str(), tim_tier.location(), errors);
            }
        }

        // E605: Reject unsupported (non-standard, non-%x) dependent tiers.
        //
        // chatter's supported dependent-tier set is closed: a tier is valid only
        // if it is one of the supported standard tiers OR a `%x`-prefixed
        // user-defined tier. Anything else is invalid CHAT, so this is an ERROR,
        // not a warning (CLAN CHECK error 17, "Tier is not declared in depfile").
        // The supported set deliberately excludes the legacy `%grt`/`%tra`/`%trn`
        // (and `%umor`) tiers retired when morphology was standardized on a single
        // UD `%mor` tier; CLAN still accepts `%grt`/`%tra`/`%trn`, so chatter is
        // intentionally stricter there (a documented divergence, see the book's
        // "Dependent Tiers" page). `%x*` user-defined tiers never reach here.
        for entry in &self.dependent_tiers {
            if let DependentTier::Unsupported(t) = &entry.tier {
                errors.report(
                    crate::ParseError::at_location(
                        crate::ErrorCode::UnsupportedDependentTier,
                        crate::Severity::Error,
                        t.location,
                        format!(
                            "Unsupported dependent tier '%{}'",
                            t.label
                        ),
                    )
                    .with_suggestion(
                        "Use a standard tier name (e.g., %mor, %gra) or prefix with 'x' for user-defined tiers (e.g., %xfoo)",
                    ),
                );
            }
        }
    }
}

/// E603: Emit warning for non-time-like `%tim` tier content.
fn check_tim_tier_format(content: &str, location: crate::Location, errors: &impl ErrorSink) {
    errors.report(
        crate::ParseError::at_location(
            crate::ErrorCode::InvalidTimTierFormat,
            crate::Severity::Warning,
            location,
            format!("Invalid %tim tier format: '{}'", content.trim()),
        )
        .with_suggestion("Expected time format: HH:MM:SS or HH:MM:SS.mmm"),
    );
}

impl Utterance {
    /// Computes alignments first, then runs standard utterance validation.
    ///
    /// Use this entrypoint when validation results should include alignment
    /// diagnostics generated from current tier content. This mutates alignment
    /// caches before validation, so callers should treat it as a stateful pass.
    pub fn validate_with_alignment(
        &mut self,
        context: &ValidationContext,
        errors: &impl ErrorSink,
    ) {
        self.compute_alignments(context);
        self.validate(context, errors);
    }
}
