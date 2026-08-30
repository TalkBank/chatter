//! Health-gated edit admission: the gate in front of the byte-splicing engine.
//!
//! [`admit_edits`] partitions a proposed edit set into what may be spliced
//! and what must be refused, using the health of the utterance and the edit's
//! unforgeable recovery-safety state. An edit whose enclosing utterance parsed
//! clean is admitted. A catalog-owned edit typed as the repair for the syntax
//! recovery at its own site is also admitted, then must pass the caller's
//! post-splice reparse. Other edits in recovered, provenance-unknown, or
//! out-of-utterance regions are refused with a reason.
//!
//! This is the mechanism that lets a fixer repair one healthy utterance in a
//! transcript whose OTHER utterances are broken, rather than refusing the
//! whole file the way whole-file repair does: `IISRP 049-1.cha` had a clean
//! utterance at line 106 and an unrelated parse error at line 502, and the
//! whole file was refused for want of exactly this gate.

use talkbank_model::model::{ChatFile, ParseHealthState};

use super::engine::{EditProvenance, RecoverySafety, SpliceEdit};

/// Why one edit was not applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// The enclosing utterance needed parser recovery.
    TaintedUtterance,
    /// The enclosing utterance carries no parse provenance at all.
    ///
    /// `ParseHealthState::Unknown` is the `#[default]` variant, so it marks
    /// content nobody ever ran through a parser path that recorded whether
    /// recovery happened. That is indistinguishable from "healthy" under a
    /// check that merely excludes `Tainted`, so it gets its own reason
    /// rather than folding into either of the other two.
    UnknownHealth,
    /// The span lies outside every utterance (header region, or stray).
    OutsideAnyUtterance,
}

/// One rejected edit and the reason.
#[derive(Clone, Debug)]
pub struct Skipped {
    /// What produced the rejected edit.
    pub provenance: EditProvenance,
    /// Why it was rejected.
    pub reason: SkipReason,
}

/// The partition of offered edits into applied and skipped.
#[derive(Debug, Default)]
pub struct Admission {
    /// Edits cleared for splicing.
    pub admitted: Vec<SpliceEdit>,
    /// Edits refused, each with a reason to report.
    pub skipped: Vec<Skipped>,
}

/// Admit edits whose enclosing utterance parsed clean, plus catalog-owned
/// edits typed as repairing their own recovery-tainted syntax.
///
/// Health is tested for `Clean` EXPLICITLY rather than for "not Tainted".
/// The two states differ precisely on `Unknown`, the derive default: an
/// utterance nobody attached parser provenance to is not evidence that its
/// text is trustworthy, so a negative test (`!= Tainted`) would silently
/// admit edits into content the parser never vouched for. That is exactly
/// the sentinel-shaped failure this engine exists to rule out, so the match
/// below is exhaustive over all three `ParseHealthState` variants and
/// `Clean` is the only one that admits.
pub fn admit_edits(file: &ChatFile, edits: Vec<SpliceEdit>) -> Admission {
    let mut admission = Admission::default();

    for edit in edits {
        let offset = edit.target().start_offset();
        let Some(utterance) = file.utterance_containing(offset) else {
            admission.skipped.push(Skipped {
                provenance: edit.provenance().clone(),
                reason: SkipReason::OutsideAnyUtterance,
            });
            continue;
        };

        match utterance.parse_health {
            ParseHealthState::Clean => admission.admitted.push(edit),
            ParseHealthState::Unknown => admission.skipped.push(Skipped {
                provenance: edit.provenance().clone(),
                reason: SkipReason::UnknownHealth,
            }),
            ParseHealthState::Tainted(_) => match edit.recovery_safety() {
                RecoverySafety::RequiresClean => admission.skipped.push(Skipped {
                    provenance: edit.provenance().clone(),
                    reason: SkipReason::TaintedUtterance,
                }),
                RecoverySafety::RepairsTaintingSyntax => admission.admitted.push(edit),
            },
        }
    }

    admission
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::ErrorCollector;
    use talkbank_model::Span;
    use talkbank_model::model::{Line, MainTier, Terminator, Utterance};
    use talkbank_parser::TreeSitterParser;

    use super::super::engine::{EditTarget, Replacement, TransformName};

    /// Two-utterance fixture: the first utterance is well-formed, the
    /// second carries a malformed `%mor` tier (missing the tab separator),
    /// which is a construct already proven by
    /// `crates/talkbank-parser/tests/integration/test_parse_health_recovery.rs`
    /// to taint the parsing utterance without affecting any other utterance
    /// in the file. Discovered by running the real parser, not assumed.
    fn parse_two_utterance_fixture() -> ChatFile {
        let parser = TreeSitterParser::new().expect("grammar loads");
        let errors = ErrorCollector::new();
        let input = "@UTF8\n@Begin\n*CHI:\thello .\n*CHI:\thello .\n%mor no_tab_separator\n@End\n";
        let file = parser.parse_chat_file_streaming(input, &errors);
        assert!(
            !errors.to_vec().is_empty(),
            "fixture assumption: the malformed %mor tier must produce a parse diagnostic"
        );
        file
    }

    /// Builds a `SpliceEdit` with a throwaway transform provenance; the
    /// specific transform name is never asserted on, only that a
    /// provenance travels through unchanged.
    fn make_edit(target: EditTarget) -> SpliceEdit {
        SpliceEdit::new(
            target,
            Replacement::new("x"),
            EditProvenance::Transform(TransformName::new("test")),
        )
    }

    /// The whole point of the ruling: a broken region elsewhere in the file
    /// must not block a fix in a region that parsed clean.
    #[test]
    fn an_edit_in_a_clean_utterance_is_admitted_despite_damage_elsewhere() {
        let file = parse_two_utterance_fixture();
        let first = file
            .utterances()
            .next()
            .expect("fixture assumption: a first utterance exists");
        assert_eq!(
            first.parse_health,
            ParseHealthState::Clean,
            "fixture assumption: the first utterance must parse clean despite \
             the second utterance's malformed %mor tier"
        );
        let offset = first.main.span.start;

        let admission = admit_edits(&file, vec![make_edit(EditTarget::InsertAt(offset))]);

        assert_eq!(admission.admitted.len(), 1);
        assert!(admission.skipped.is_empty(), "got {:?}", admission.skipped);
    }

    /// `ParseHealthState::Unknown` is `#[default]`, so it is indistinguishable
    /// from "no provenance was ever attached". Treating it as clean is exactly
    /// the silent-sentinel failure this engine exists to stop.
    #[test]
    fn unknown_health_is_refused_not_treated_as_clean() {
        // Built directly rather than through the parser, so nothing ever
        // attaches parse provenance. `Utterance::new` sets `Clean` as a
        // convenience default for hand-assembled test/transform fixtures
        // (see `builder.rs`), so `Unknown` is set explicitly here to model
        // the true "no provenance was ever attached" case rather than
        // relying on a constructor that happens to say otherwise.
        let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY })
            .with_span(Span::new(0, 20));
        let mut utterance = Utterance::new(main);
        utterance.parse_health = ParseHealthState::Unknown;
        let file = ChatFile::new(vec![Line::Utterance(Box::new(utterance))]);

        let admission = admit_edits(&file, vec![make_edit(EditTarget::InsertAt(5))]);

        assert!(
            admission.admitted.is_empty(),
            "got {:?}",
            admission.admitted
        );
        assert_eq!(admission.skipped.len(), 1);
        assert_eq!(admission.skipped[0].reason, SkipReason::UnknownHealth);
    }

    #[test]
    fn an_edit_in_a_tainted_utterance_is_skipped_with_a_reason() {
        let file = parse_two_utterance_fixture();
        let second = file
            .utterances()
            .nth(1)
            .expect("fixture assumption: a second utterance exists");
        assert!(
            matches!(second.parse_health, ParseHealthState::Tainted(_)),
            "fixture assumption: the malformed %mor tier must taint its own utterance, got {:?}",
            second.parse_health
        );
        let offset = second.main.span.start;

        let admission = admit_edits(&file, vec![make_edit(EditTarget::InsertAt(offset))]);

        assert!(
            admission.admitted.is_empty(),
            "got {:?}",
            admission.admitted
        );
        assert_eq!(admission.skipped.len(), 1);
        assert_eq!(admission.skipped[0].reason, SkipReason::TaintedUtterance);
    }

    #[test]
    fn an_e750_edit_is_admitted_to_repair_the_recovery_that_tainted_its_utterance() {
        let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t< dog> [/] dog .\n@End\n";
        let parser = TreeSitterParser::new().expect("grammar loads");
        let errors = ErrorCollector::new();
        let file = parser.parse_chat_file_streaming(source, &errors);
        let diagnostic = errors
            .into_vec()
            .into_iter()
            .find(|error| error.code.as_str() == "E750")
            .expect("fixture emits E750");
        let fix = crate::splice::catalog_fix(&diagnostic, source).expect("E750 has a fix");
        let edits = match fix.kind {
            crate::splice::FixKind::Deterministic(edits) => edits,
            crate::splice::FixKind::Alternatives(_) => panic!("E750 is deterministic"),
        };

        let admission = admit_edits(&file, edits);

        assert_eq!(admission.admitted.len(), 1);
        assert!(admission.skipped.is_empty(), "got {:?}", admission.skipped);
    }

    #[test]
    fn an_edit_matching_no_utterance_is_skipped_rather_than_applied() {
        let file = parse_two_utterance_fixture();

        // Offset 0 falls inside "@UTF8\n", the header region preceding the
        // first utterance: no utterance's main tier or dependent tier spans
        // it.
        let admission = admit_edits(&file, vec![make_edit(EditTarget::InsertAt(0))]);

        assert!(
            admission.admitted.is_empty(),
            "got {:?}",
            admission.admitted
        );
        assert_eq!(admission.skipped.len(), 1);
        assert_eq!(admission.skipped[0].reason, SkipReason::OutsideAnyUtterance);
    }
}
