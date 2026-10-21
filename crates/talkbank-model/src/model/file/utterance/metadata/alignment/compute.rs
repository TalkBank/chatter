use super::count_based::build_tier_to_tier_alignment;
use super::diagnostics::skipped_alignment_warning;
use crate::alignment::unknown_alignment_warning;
use crate::model::{AlignmentSet, AlignmentUnits, ParseHealthState, ParseHealthTier};
use crate::validation::ValidationContext;
use crate::{ErrorCode, ParseError, Utterance};

/// One side of a tier alignment relationship (label, span, tier identity).
struct TierSide<'a> {
    label: &'a str,
    location: crate::Location,
    tier: ParseHealthTier,
}

fn alignment_blocked_warning(
    health: ParseHealthState,
    alignment_name: &str,
    left: TierSide<'_>,
    right: TierSide<'_>,
) -> ParseError {
    match health {
        ParseHealthState::Unknown => unknown_alignment_warning(
            alignment_name,
            left.label,
            left.location,
            right.label,
            Some(right.location),
        ),
        _ => skipped_alignment_warning(
            alignment_name,
            left.label,
            health.is_tier_clean(left.tier),
            left.location,
            right.label,
            health.is_tier_clean(right.tier),
            Some(right.location),
        ),
    }
}

/// The blocked warning for an alignment whose partner is a GROUP of tiers
/// (`%xphoaln` against `%mod` and `%pho`): `right_location` is the first partner
/// present, located or not, or `None` when there is none; the partner side
/// gets a label only when it is located.
fn grouped_alignment_blocked_warning(
    health: ParseHealthState,
    alignment_name: &str,
    left: TierSide<'_>,
    right_label: &str,
    right_location: Option<crate::Location>,
    right_clean: bool,
) -> ParseError {
    match health {
        ParseHealthState::Unknown => unknown_alignment_warning(
            alignment_name,
            left.label,
            left.location,
            right_label,
            right_location,
        ),
        _ => skipped_alignment_warning(
            alignment_name,
            left.label,
            health.is_tier_clean(left.tier),
            left.location,
            right_label,
            right_clean,
            right_location,
        ),
    }
}

impl Utterance {
    /// Recompute all derived alignment metadata for this utterance.
    pub fn compute_alignments(&mut self, context: &ValidationContext) {
        self.alignment_diagnostics.clear();

        let units = AlignmentUnits::from_utterance(self, context);
        let mut metadata = AlignmentSet::new(units);
        let health = self.parse_health();

        if let Some(mor) = self.mor_tier() {
            if health.can_align_main_to_mor() {
                metadata.mor = Some(crate::alignment::align_main_to_mor(&self.main, mor));
            } else {
                metadata.mor = Some(crate::alignment::MorAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%mor",
                        TierSide {
                            label: "main tier",
                            location: self.main.location,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%mor tier",
                            location: mor.location,
                            tier: ParseHealthTier::Mor,
                        },
                    ),
                ));
            }
        }

        if let (Some(mor), Some(gra)) = (self.mor_tier(), self.gra_tier()) {
            if health.can_align_mor_to_gra() {
                metadata.gra = Some(crate::alignment::align_mor_to_gra(mor, gra));
            } else {
                metadata.gra = Some(crate::alignment::GraAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "%mor↔%gra",
                        TierSide {
                            label: "%mor tier",
                            location: mor.location,
                            tier: ParseHealthTier::Mor,
                        },
                        TierSide {
                            label: "%gra tier",
                            location: gra.location,
                            tier: ParseHealthTier::Gra,
                        },
                    ),
                ));
            }
        }

        if let Some(tier) = self.pho_tier() {
            let pho_location = tier.location;
            if health.can_align_main_to_pho() {
                metadata.pho = Some(crate::alignment::align_main_to_pho(&self.main, tier));
            } else {
                metadata.pho = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%pho",
                        TierSide {
                            label: "main tier",
                            location: self.main.location,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%pho tier",
                            location: pho_location,
                            tier: ParseHealthTier::Pho,
                        },
                    ),
                ));
            }
        }

        if let Some(wor) = self.wor_tier() {
            // `%wor` is a timing sidecar, not a `TierAlignmentResult`. On
            // parse-taint (`!can_resolve_wor_timing_sidecar()`) we leave the slot as
            // `None`; unlike the structural alignments above there is no
            // error stream to populate, and the sidecar has nothing
            // meaningful to report for tainted input. Callers that need
            // per-tier taint context should consult `ParseHealth` directly.
            if health.can_resolve_wor_timing_sidecar() {
                metadata.wor_timings = Some(crate::alignment::resolve_wor_timing_sidecar(
                    &self.main, wor,
                ));
            }
        }

        if let Some(tier) = self.mod_tier() {
            let mod_location = tier.location;
            if health.can_align_main_to_mod() {
                metadata.mod_ = Some(crate::alignment::align_main_to_pho(&self.main, tier));
            } else {
                metadata.mod_ = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%mod",
                        TierSide {
                            label: "main tier",
                            location: self.main.location,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%mod tier",
                            location: mod_location,
                            tier: ParseHealthTier::Mod,
                        },
                    ),
                ));
            }
        }

        if let Some(tier) = self.sin_tier() {
            let sin_location = tier.location;
            if health.can_align_main_to_sin() {
                metadata.sin = Some(crate::alignment::align_main_to_sin(&self.main, tier));
            } else {
                metadata.sin = Some(crate::alignment::SinAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%sin",
                        TierSide {
                            label: "main tier",
                            location: self.main.location,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%sin tier",
                            location: sin_location,
                            tier: ParseHealthTier::Sin,
                        },
                    ),
                ));
            }
        }

        if let (Some(modsyl), Some(mod_tier)) = (self.modsyl_tier(), self.mod_tier()) {
            let (modsyl_location, mod_location) = (modsyl.location, mod_tier.location);
            if health.can_align_modsyl_to_mod() {
                metadata.modsyl = Some(build_tier_to_tier_alignment(
                    modsyl.word_count(),
                    modsyl_location,
                    "%modsyl",
                    mod_tier.items.len(),
                    mod_location,
                    "%mod",
                    ErrorCode::ModsylModCountMismatch,
                ));
            } else {
                metadata.modsyl = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "%modsyl↔%mod",
                        TierSide {
                            label: "%modsyl tier",
                            location: modsyl_location,
                            tier: ParseHealthTier::Modsyl,
                        },
                        TierSide {
                            label: "%mod tier",
                            location: mod_location,
                            tier: ParseHealthTier::Mod,
                        },
                    ),
                ));
            }
        }

        if let (Some(phosyl), Some(pho_tier)) = (self.phosyl_tier(), self.pho_tier()) {
            let (phosyl_location, pho_location) = (phosyl.location, pho_tier.location);
            if health.can_align_phosyl_to_pho() {
                metadata.phosyl = Some(build_tier_to_tier_alignment(
                    phosyl.word_count(),
                    phosyl_location,
                    "%phosyl",
                    pho_tier.items.len(),
                    pho_location,
                    "%pho",
                    ErrorCode::PhosylPhoCountMismatch,
                ));
            } else {
                metadata.phosyl = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "%phosyl↔%pho",
                        TierSide {
                            label: "%phosyl tier",
                            location: phosyl_location,
                            tier: ParseHealthTier::Phosyl,
                        },
                        TierSide {
                            label: "%pho tier",
                            location: pho_location,
                            tier: ParseHealthTier::Pho,
                        },
                    ),
                ));
            }
        }

        if let Some(phoaln) = self.phoaln_tier() {
            let phoaln_location = phoaln.location;
            let phoaln_wc = phoaln.word_count();
            if health.can_align_phoaln() {
                let mut alignment = crate::alignment::PhoAlignment::new();
                let mod_count = self.mod_tier().map(|t| t.items.len());
                let pho_count = self.pho_tier().map(|t| t.items.len());

                // Each %xphoaln word consumes a word slot on %mod, %pho, or
                // both; a one-sided pause word (spec §2 rule 5) consumes a
                // slot only on the tier bearing the pause, so the raw
                // %xphoaln word count is not the number that must equal
                // %mod's or %pho's count.
                let mut expected_mod = 0;
                let mut expected_pho = 0;
                for binding in self.phoaln_word_bindings() {
                    expected_mod += usize::from(binding.consumes_model());
                    expected_pho += usize::from(binding.consumes_actual());
                    alignment = alignment.with_pair(binding.into_alignment_pair());
                }

                if let Some(mc) = mod_count
                    && expected_mod != mc
                {
                    alignment = alignment.with_error(
                        super::diagnostics::build_phoaln_count_mismatch_error(
                            phoaln_wc,
                            expected_mod,
                            phoaln_location,
                            mc,
                            "%mod",
                            ErrorCode::PhoalnModCountMismatch,
                        ),
                    );
                }
                if let Some(pc) = pho_count
                    && expected_pho != pc
                {
                    alignment = alignment.with_error(
                        super::diagnostics::build_phoaln_count_mismatch_error(
                            phoaln_wc,
                            expected_pho,
                            phoaln_location,
                            pc,
                            "%pho",
                            ErrorCode::PhoalnPhoCountMismatch,
                        ),
                    );
                }

                metadata.phoaln = Some(alignment);
            } else {
                metadata.phoaln = Some(
                    crate::alignment::PhoAlignment::new().with_error(
                        grouped_alignment_blocked_warning(
                            health,
                            "%phoaln↔%mod/%pho",
                            TierSide {
                                label: "%phoaln tier",
                                location: phoaln_location,
                                tier: ParseHealthTier::Phoaln,
                            },
                            "%mod/%pho tiers",
                            // The partner side is `None` only when no partner
                            // tier exists; an unlocated partner is still one.
                            self.mod_tier()
                                .map(|t| t.location)
                                .or_else(|| self.pho_tier().map(|t| t.location)),
                            health.is_tier_clean(ParseHealthTier::Mod)
                                && health.is_tier_clean(ParseHealthTier::Pho),
                        ),
                    ),
                );
            }
        }

        self.alignment_diagnostics = metadata.collect_errors().into_iter().cloned().collect();
        self.alignments = Some(metadata);
    }

    /// Recompute alignments using a default validation context.
    pub fn compute_alignments_default(&mut self) {
        self.compute_alignments(&ValidationContext::default());
    }
}
