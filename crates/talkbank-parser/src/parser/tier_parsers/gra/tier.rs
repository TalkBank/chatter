//! `%gra` tier-level parsing logic.
//!
//! Converts one `%gra` line into a `GraTier` by decoding each whitespace-
//! separated `index|head|relation` triple.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::generated_traversal::{
    AdmittedGraContentsChildren, AsRawNode, GraContentsNode, GraDependentTierChildren,
    GraDependentTierNode, GraRelationNode, KindSlot, RangeAdmittedChildren, Ranged, ReadableSlot,
    SourceBound, SourceChildren, SourceSlice, WhitespacesNode,
};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::{ParsedDependentTier, dependent_tier_separator};
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::model::{GraLowering, GraTier};
use talkbank_model::{ErrorSink, ParseOutcome, TierSeparator};

use super::relation::parse_gra_relation;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};

/// Converts one `%gra` tier node into `GraTier`.
///
/// **Grammar Rule:**
/// ```text
/// gra_dependent_tier: seq(gra_tier_prefix, tier_sep, gra_contents, newline)
/// ```
///
/// Source-bound extraction retains the owner through body, repeats and relation
/// fields. Per-rule admission proves the ordinary composite body present.
/// Nested relation recovery remains explicit; an envelope capability does not
/// certify the fields inside the body or the caller's CHAT validity judgment.
/// Source/reconstruction failures propagate as `CstFailure`, not CHAT invalidity.
pub fn parse_gra_tier<'tree>(
    typed: SourceBound<'tree, '_, GraDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<GraTier>, crate::CstFailure> {
    parse_gra_tier_with_separator(typed, errors).map(ParsedDependentTier::into_tier)
}

/// Root extraction and separator provenance share one immutable source owner.
#[must_use]
struct GraTierReading<'tree, 'source> {
    children: SourceChildren<
        'tree,
        'source,
        GraDependentTierChildren<
            'tree,
            crate::generated_traversal::KindAdmitted,
            crate::generated_traversal::Raw,
            crate::generated_traversal::RequiredPresence,
        >,
    >,
    separator: TierSeparator,
}

/// Only this consumed phase can decode or preserve the selected root's model.
#[must_use]
struct RangedGraTier<'tree, 'source> {
    children: RangeAdmittedChildren<
        'tree,
        'source,
        GraDependentTierChildren<
            'tree,
            crate::generated_traversal::KindAdmitted,
            Ranged<'source>,
            crate::generated_traversal::RequiredPresence,
        >,
    >,
    separator: TierSeparator,
}

impl<'tree, 'source> GraTierReading<'tree, 'source> {
    fn extract(
        typed: SourceBound<'tree, 'source, GraDependentTierNode<'tree>>,
    ) -> Result<Self, crate::CstFailure> {
        let children = crate::parser::typed_cst::extract_envelope(typed)?;
        let separator = dependent_tier_separator(children.children().child_1.slot())?;
        Ok(Self {
            children,
            separator,
        })
    }

    fn admit_ranges(self) -> Result<RangedGraTier<'tree, 'source>, crate::CstFailure> {
        Ok(RangedGraTier {
            children: self.children.admit_ranges()?,
            separator: self.separator,
        })
    }
}

impl RangedGraTier<'_, '_> {
    fn parse(
        self,
        errors: &impl ErrorSink,
    ) -> Result<ParsedDependentTier<ParseOutcome<GraTier>>, crate::CstFailure> {
        Ok(ParsedDependentTier::new(
            parse_gra_root_contents(&self.children, errors)?,
            self.separator,
        ))
    }
}

pub(crate) fn parse_gra_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, GraDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParsedDependentTier<ParseOutcome<GraTier>>, crate::CstFailure> {
    let reading = GraTierReading::extract(typed)?;
    surface_displaced(
        &reading.children.children().unexpected,
        "gra_dependent_tier",
        reading.children.source(),
        errors,
    );
    reading.admit_ranges()?.parse(errors)
}

pub(crate) fn gra_recovery_separator<'tree>(
    typed: SourceBound<'tree, '_, GraDependentTierNode<'tree>>,
) -> Result<TierSeparator, crate::CstFailure> {
    // Admit the envelope and its source ranges without manufacturing a graph.
    Ok(GraTierReading::extract(typed)?.admit_ranges()?.separator)
}

fn parse_gra_root_contents<'tree, 'source>(
    children: &RangeAdmittedChildren<
        'tree,
        'source,
        GraDependentTierChildren<
            'tree,
            crate::generated_traversal::KindAdmitted,
            Ranged<'source>,
            crate::generated_traversal::RequiredPresence,
        >,
    >,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<GraTier>, crate::CstFailure> {
    let parent = children.parent();
    let node = parent.raw_node();
    let span = span_of(node);

    let crate::generated_traversal::SlotView::Present(contents) =
        children.children().child_2.slot.view();
    let lowering = parse_gra_relations(*contents, errors)?;
    if node.has_error() {
        // Root recovery can cover text outside a declared relation. Diagnose the
        // complete carrier rather than admitting its surviving relations.
        errors.report(talkbank_model::ParseError::at_span(
            talkbank_model::ErrorCode::UnparsableContent,
            talkbank_model::Severity::Error,
            span,
            "GRA tier contains recovered syntax",
        ));
        Ok(ParseOutcome::Rejected)
    } else {
        Ok(lowering.finish(span))
    }
}

/// Decode every `gra_relation` under a `gra_contents` node into the relation
/// model, driven by the generated `extract_gra_contents` visitor.
///
/// `gra_contents = seq(gra_relation, repeat(seq(whitespaces, gra_relation)))`,
/// so the visitor exposes the first relation as `child_0` and each subsequent
/// `(whitespaces, gra_relation)` pair as a `GraContentsChild1Children` element
/// in `child_1`. This replaces the old `while gra_contents.child(idx)`
/// positional walk. Unlike the OLD backend (built with `--skip whitespaces`),
/// the NEW backend models the separating `whitespaces` token as its own
/// explicit `child_0` position inside each repeat element (`child_1` holds the
/// `gra_relation` itself); that position is purely structural (no content to
/// decode) and retains the recovery handling in [`push_gra_separator`].
///
/// Returns collection for the owning root to finalize. Rejected fields prevent
/// model construction; root recovery also rejects text outside relation slots.
/// Diagnostic traversal continues after rejection to retain independent findings.
fn parse_gra_relations<'tree>(
    typed: SourceBound<'tree, '_, GraContentsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<GraLowering, crate::CstFailure> {
    let contents = typed.extract_canonical()?.admit_ranges()?;
    lower_gra_relations(&contents, errors)
}

/// The whole contents carrier owns its first relation, inline repeats and extras.
fn lower_gra_relations<'tree, 'source>(
    contents: &RangeAdmittedChildren<
        'tree,
        'source,
        AdmittedGraContentsChildren<'tree, Ranged<'source>>,
    >,
    errors: &impl ErrorSink,
) -> Result<GraLowering, crate::CstFailure> {
    let mut lowering = GraLowering::with_capacity(contents.children().child_1.slot.len() + 1);

    push_gra_relation(&contents.children().child_0.slot, errors, &mut lowering)?;
    for element in &contents.children().child_1.slot {
        match &element.slot {
            ReadableSlot::Present(pair) => {
                push_gra_separator(&pair.child_0.slot, errors);
                push_gra_relation(&pair.child_1.slot, errors, &mut lowering)?;
                surface_source_displaced(&pair.unexpected, "gra_contents", errors);
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            ReadableSlot::Error(raw) => {
                errors.report(unexpected_node_error(
                    raw.raw_node(),
                    raw.source(),
                    "gra_contents",
                ));
            }
            ReadableSlot::Missing(never) | ReadableSlot::Unexpected(never) => match *never {},
            ReadableSlot::Absent(never) => match *never {},
        }
    }

    surface_source_displaced(&contents.children().unexpected, "gra_contents", errors);
    Ok(lowering)
}

/// Decode the separating `whitespaces` token inside one `gra_contents` repeat
/// element.
///
/// A NEW position with no OLD counterpart: the OLD backend was generated with
/// `--skip whitespaces`, so the space between two `index|head|relation`
/// triples was never a modeled child at all. It carries no content, so
/// `Present` is a no-op; the recovery arms reuse the SAME diagnostic mechanism
/// [`push_gra_relation`] already uses for this file's other `gra_contents`
/// positions (`check_not_missing` / `unexpected_node_error`), for consistency.
/// Source ownership does not narrow these recovery states. Keep their handling
/// until the producer's slot types establish a stronger invariant.
fn push_gra_separator<'tree, 'source, A: crate::generated_traversal::Absence>(
    slot: &KindSlot<'tree, SourceBound<'tree, 'source, WhitespacesNode<'tree>>, Ranged<'source>, A>,
    errors: &impl ErrorSink,
) {
    match slot {
        ReadableSlot::Present(_) | ReadableSlot::Absent(_) => {}
        ReadableSlot::Missing(raw) => {
            check_not_missing(raw.raw_node(), raw.source(), errors, "gra_contents");
        }
        ReadableSlot::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                raw.source(),
                "gra_contents",
            ));
        }
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

/// Decode one relation slot, recording its actual result in the lowering owner.
///
/// Compiled admission excludes MISSING composite relations. Present relations
/// are retained only after successful field admission; no default is
/// fabricated for a rejected relation. Error is diagnosed and Absent emits
/// nothing. Lexical fields retain their independent recovery checks.
///
/// Only a Present relation is a declaration. Its parsed/rejected result is
/// recorded atomically; Error/Absent does not invent a declaration or relation.
fn push_gra_relation<'tree, 'source, A: crate::generated_traversal::Absence>(
    slot: &crate::generated_traversal::NonMissingKindSlot<
        'tree,
        SourceBound<'tree, 'source, GraRelationNode<'tree>>,
        Ranged<'source>,
        A,
    >,
    errors: &impl ErrorSink,
    lowering: &mut GraLowering,
) -> Result<(), crate::CstFailure> {
    match slot {
        ReadableSlot::Present(relation_node) => {
            lowering.record(parse_gra_relation(*relation_node, errors)?);
        }
        ReadableSlot::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                raw.source(),
                "gra_contents",
            ));
        }
        ReadableSlot::Missing(missing) => match missing.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(_) => {}
    }
    Ok(())
}

fn surface_source_displaced(
    unexpected: &[SourceSlice<'_, '_>],
    context: &str,
    errors: &impl ErrorSink,
) {
    for node in unexpected {
        surface_displaced(&[node.raw_node()], context, node.source(), errors);
    }
}

#[cfg(test)]
use crate::generated_traversal::NoChild;

#[cfg(test)]
mod range_tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::{SourceBindingError, SourceSlotView};
    use talkbank_model::ErrorCollector;

    /// Generated/native boundary, recovery policy and counted selected work.
    /// These are actual authored CST observations, not universal reachability
    /// claims or fixture models invented to enter a recovery branch.
    #[test]
    fn gra_selected_carriers_retain_source_ranges_and_recovery() {
        let parser = TreeSitterParser::new().expect("grammar");
        let cases = [
            (
                "reference",
                include_str!("../../../../../../corpus/reference/tiers/mor-gra.cha"),
            ),
            (
                "E709",
                include_str!(
                    "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E709_1.cha"
                ),
            ),
            (
                "E708",
                include_str!(
                    "../../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E708_1.cha"
                ),
            ),
        ];
        for (name, source) in cases {
            let parsed = parser
                .parse_source_incremental(source, None)
                .expect("authored native source");
            let foreign = parser
                .parse_source_incremental(source, None)
                .expect("independent owner of equal bytes");
            let mut carriers = [0; 3];
            let mut body_states = [0; 4];
            let mut root_checks = Vec::new();
            let mut contents_checks = Vec::new();
            let mut relation_checks = Vec::new();
            let mut repeated_pairs = 0;
            for node in parsed.root().expect("source root").descendants() {
                let node = node.expect("source-bound descendant");
                if let Some(tier) = node.typed::<GraDependentTierNode>() {
                    carriers[0] += 1;
                    assert!(matches!(
                        foreign.bind(tier.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let reading = GraTierReading::extract(tier).expect("kind-admitted root");
                    let admitted = tier
                        .extract_canonical()
                        .expect("kind-admitted root observation");
                    let before = match admitted.field_child_2().slot().view() {
                        SourceSlotView::Present(_) => 0,
                        SourceSlotView::Missing(_) => 3,
                        SourceSlotView::Error(_) => 1,
                        SourceSlotView::Absent(NoChild) => 2,
                    };
                    let separator = reading.separator;
                    let ranged = reading.admit_ranges().expect("whole selected root");
                    assert_eq!(ranged.children.parent().raw_node(), tier.raw_node());
                    assert_eq!(ranged.children.parent().text(), tier.text());
                    assert_eq!(ranged.children.parent().source(), source);
                    let crate::generated_traversal::SlotView::Present(_) =
                        ranged.children.children().child_2.slot.view();
                    let after = 0;
                    assert_eq!(after, before, "range admission retains {name}'s body state");
                    body_states[after] += 1;
                    let checks = ranged.children.checked_range_count();
                    for _ in 0..100 {
                        assert_eq!(ranged.children.parent().text(), tier.text());
                        assert_eq!(ranged.children.checked_range_count(), checks);
                    }
                    root_checks.push(checks);
                    let errors = ErrorCollector::new();
                    let parsed = ranged
                        .parse(&errors)
                        .expect("native lowering is distinct from CHAT validity");
                    assert_eq!(parsed.separator, separator);
                    match parsed.tier {
                        ParseOutcome::Parsed(gra) => {
                            assert!(!tier.raw_node().has_error());
                            assert_eq!(
                                gra.location,
                                talkbank_model::Location::Located(span_of(tier.raw_node()))
                            );
                        }
                        ParseOutcome::Rejected => assert!(!errors.is_empty()),
                    }
                    if tier.raw_node().has_error() {
                        let recovered = gra_recovery_separator(tier)
                            .expect("document structural-recovery root");
                        assert_eq!(recovered, separator);
                    }
                }
                if let Some(contents) = node.typed::<GraContentsNode>() {
                    carriers[1] += 1;
                    assert!(matches!(
                        foreign.bind(contents.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let ranged = contents
                        .extract_canonical()
                        .expect("kind-admitted contents")
                        .admit_ranges()
                        .expect("contents and inline repeat ranges");
                    assert_eq!(ranged.parent().raw_node(), contents.raw_node());
                    assert_eq!(ranged.parent().text(), contents.text());
                    assert_eq!(ranged.parent().source(), source);
                    contents_checks.push(ranged.checked_range_count());
                    for element in &ranged.children().child_1.slot {
                        match &element.slot {
                            ReadableSlot::Present(pair) => {
                                repeated_pairs += 1;
                                if let ReadableSlot::Present(separator) = &pair.child_0.slot {
                                    assert_eq!(separator.source(), source);
                                    assert!(matches!(
                                        foreign.bind(separator.raw_node()),
                                        Err(SourceBindingError::ForeignTree)
                                    ));
                                }
                                if let ReadableSlot::Present(relation) = &pair.child_1.slot {
                                    assert_eq!(relation.source(), source);
                                    assert!(matches!(
                                        foreign.bind(relation.raw_node()),
                                        Err(SourceBindingError::ForeignTree)
                                    ));
                                }
                            }
                            ReadableSlot::Error(error) => assert_eq!(error.source(), source),
                            ReadableSlot::Missing(never) | ReadableSlot::Unexpected(never) => {
                                match *never {}
                            }
                            ReadableSlot::Absent(never) => match *never {},
                        }
                    }
                }
                if let Some(relation) = node.typed::<GraRelationNode>() {
                    carriers[2] += 1;
                    assert!(matches!(
                        foreign.bind(relation.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let ranged = relation
                        .extract()
                        .admit_ranges()
                        .expect("selected lexical fields");
                    assert_eq!(ranged.parent().raw_node(), relation.raw_node());
                    assert_eq!(ranged.parent().text(), relation.text());
                    assert_eq!(ranged.parent().source(), source);
                    let checks = ranged.checked_range_count();
                    super::super::relation::assert_repeated_field_reads(&ranged);
                    relation_checks.push(checks);
                }
            }
            if name != "E708" {
                assert!(
                    carriers.into_iter().all(|count| count > 0),
                    "{name}: {carriers:?}"
                );
                assert!(repeated_pairs > 0, "authored repeated relations required");
            }
            eprintln!(
                "gra selected {name}: carriers={carriers:?} body[present,error,absent,missing]={body_states:?} pairs={repeated_pairs}; checks root={root_checks:?} contents={contents_checks:?} relation={relation_checks:?}; inline reading={} ranged={}",
                std::mem::size_of::<GraTierReading<'_, '_>>(),
                std::mem::size_of::<RangedGraTier<'_, '_>>(),
            );
        }
    }
}
