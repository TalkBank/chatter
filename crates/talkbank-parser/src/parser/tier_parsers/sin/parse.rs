//! Parser for `%sin` tier bodies.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{
    AdmittedSinDependentTierChildren, AdmittedSinGroupsChildren, AsRawNode, NoChild,
    NonMissingKindSlot, RangeAdmittedChildren, Ranged, ReadableSlot, SinDependentTierNode,
    SinGroupNode, SinGroupsNode, SourceBound, SourceChildren,
};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::{ParsedDependentTier, dependent_tier_separator};
use talkbank_model::ErrorSink;
use talkbank_model::model::{SinItem, SinTier, TierSeparator};

use super::groups::{extract_sin_group_items, push_sin_separator, surface_source_displaced};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

/// Converts one `%sin` tier node into `SinTier`.
///
/// **Grammar Rule:**
/// ```text
/// sin_dependent_tier: seq('%', 'sin', colon, tab, sin_groups, newline)
/// ```
///
/// Source-bound extraction retains ownership through body, groups and tokens.
/// Compiled-grammar admission excludes missing composite bodies; Error/Absent
/// retain the existing empty-tier policy. Unexpected is uninhabited in the slot.
/// Source/reconstruction failures propagate as `CstFailure` rather than empty
/// content. Source ownership does not prove remaining recovery impossible.
pub fn parse_sin_tier<'tree>(
    typed: SourceBound<'tree, '_, SinDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<SinTier, crate::CstFailure> {
    parse_sin_tier_with_separator(typed, errors).map(ParsedDependentTier::into_tier)
}

/// One selected root extraction supplies both payload and separator provenance.
#[must_use]
struct SinTierReading<'tree, 'source> {
    children: SourceChildren<'tree, 'source, AdmittedSinDependentTierChildren<'tree>>,
    separator: TierSeparator,
}

/// Consuming range admission retains the same root and its metadata.
#[must_use]
struct RangedSinTier<'tree, 'source> {
    children: RangeAdmittedChildren<
        'tree,
        'source,
        AdmittedSinDependentTierChildren<'tree, Ranged<'source>>,
    >,
    separator: TierSeparator,
}

impl<'tree, 'source> SinTierReading<'tree, 'source> {
    fn extract(
        typed: SourceBound<'tree, 'source, SinDependentTierNode<'tree>>,
    ) -> Result<Self, crate::CstFailure> {
        let children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
        let separator = dependent_tier_separator(children.children().child_1.slot());
        Ok(Self {
            children,
            separator,
        })
    }

    fn admit_ranges(self) -> Result<RangedSinTier<'tree, 'source>, crate::CstFailure> {
        Ok(RangedSinTier {
            children: self.children.admit_ranges()?,
            separator: self.separator,
        })
    }
}

impl RangedSinTier<'_, '_> {
    fn parse(
        self,
        errors: &impl ErrorSink,
    ) -> Result<ParsedDependentTier<SinTier>, crate::CstFailure> {
        Ok(ParsedDependentTier::new(
            lower_sin_tier(&self.children, errors)?,
            self.separator,
        ))
    }
}

pub(crate) fn parse_sin_tier_with_separator<'tree>(
    typed: SourceBound<'tree, '_, SinDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParsedDependentTier<SinTier>, crate::CstFailure> {
    let reading = SinTierReading::extract(typed)?;
    surface_displaced(
        &reading.children.children().unexpected,
        "sin_dependent_tier",
        reading.children.source(),
        errors,
    );
    reading.admit_ranges()?.parse(errors)
}

fn lower_sin_tier<'tree, 'source>(
    children: &RangeAdmittedChildren<
        'tree,
        'source,
        AdmittedSinDependentTierChildren<'tree, Ranged<'source>>,
    >,
    errors: &impl ErrorSink,
) -> Result<SinTier, crate::CstFailure> {
    let span = span_of(children.parent().raw_node());
    Ok(match &children.children().child_2.slot {
        ReadableSlot::Present(groups) => {
            let items = parse_sin_groups(*groups, errors)?;
            SinTier::new(items).with_span(span)
        }
        ReadableSlot::Error(_) | ReadableSlot::Absent(NoChild) => {
            SinTier::new(Vec::new()).with_span(span)
        }
        ReadableSlot::Missing(never) => match never.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
    })
}

/// Decode every `sin_group` under a `sin_groups` node into gesture/sign items,
/// driven by the generated `extract_sin_groups` visitor.
///
/// `sin_groups = seq(sin_group, repeat(seq(whitespaces, sin_group)))`, so the
/// visitor exposes the first group as `child_0` and each subsequent
/// `(whitespaces, sin_group)` pair as a `SinGroupsChild1Children` element in
/// `child_1`. This replaces the old `while sin_groups.child(idx)` positional
/// walk. Unlike the OLD backend (built with `--skip whitespaces`), the NEW
/// backend models the separating `whitespaces` token as its own explicit
/// `child_0` position inside each repeat element (`child_1` holds the
/// `sin_group`); that position is purely structural and handled by
/// [`push_sin_separator`].
fn parse_sin_groups<'tree>(
    typed: SourceBound<'tree, '_, SinGroupsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<SinItem>, crate::CstFailure> {
    let groups = typed
        .extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?
        .admit_ranges()?;
    lower_sin_groups(&groups, errors)
}

fn lower_sin_groups<'tree, 'source>(
    groups: &RangeAdmittedChildren<
        'tree,
        'source,
        AdmittedSinGroupsChildren<'tree, Ranged<'source>>,
    >,
    errors: &impl ErrorSink,
) -> Result<Vec<SinItem>, crate::CstFailure> {
    let mut items = Vec::with_capacity(groups.children().child_1.slot.len() + 1);

    push_sin_group(&groups.children().child_0.slot, errors, &mut items)?;
    for element in &groups.children().child_1.slot {
        match &element.slot {
            ReadableSlot::Present(pair) => {
                push_sin_separator(&pair.child_0.slot, errors, "sin_groups");
                push_sin_group(&pair.child_1.slot, errors, &mut items)?;
                surface_source_displaced(&pair.unexpected, "sin_groups", errors);
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            ReadableSlot::Error(raw) => {
                errors.report(unexpected_node_error(
                    raw.raw_node(),
                    raw.source(),
                    "sin_groups",
                ));
            }
            ReadableSlot::Absent(never) => match *never {},
            ReadableSlot::Missing(never) | ReadableSlot::Unexpected(never) => match *never {},
        }
    }

    surface_source_displaced(&groups.children().unexpected, "sin_groups", errors);
    Ok(items)
}

/// Decode one `sin_group` slot, extending `items` with its gesture/sign items.
///
/// Grammar admission rules out missing composite groups, not Error or Absent.
/// Present groups retain flat/grouped decoding; Error is diagnosed and Absent
/// emits nothing. Source failures propagate independently of CHAT recovery.
fn push_sin_group<'tree, 'source>(
    slot: &NonMissingKindSlot<
        'tree,
        SourceBound<'tree, 'source, SinGroupNode<'tree>>,
        Ranged<'source>,
    >,
    errors: &impl ErrorSink,
    items: &mut Vec<SinItem>,
) -> Result<(), crate::CstFailure> {
    match slot {
        ReadableSlot::Present(group_node) => {
            items.extend(extract_sin_group_items(*group_node, errors)?);
        }
        ReadableSlot::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                raw.source(),
                "sin_groups",
            ));
        }
        ReadableSlot::Absent(NoChild) => {}
        ReadableSlot::Missing(never) => match never.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
    }
    Ok(())
}

#[cfg(test)]
mod range_tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::{
        AdmittedSinGroupChoiceSourceView, SinGroupChoice, SinGroupedContentNode,
        SourceBindingError, SourceSlotView,
    };
    use talkbank_model::ErrorCollector;
    use talkbank_model::model::DependentTier;

    /// Actual selected carriers, foreign-owner boundary and counted reads.
    /// Recovery shapes are observations, not universal reachability verdicts.
    #[test]
    fn sin_selected_carriers_retain_source_ranges_and_choices() {
        let parser = TreeSitterParser::new().expect("grammar");
        let cases = [
            (
                "flat",
                include_str!("../../../../../../corpus/reference/tiers/sin.cha"),
            ),
            (
                "grouped",
                include_str!("../../../../../../corpus/reference/annotation/groups-sign.cha"),
            ),
        ];
        let mut witnessed = [false; 4];
        for (name, source) in cases {
            let parsed = parser
                .parse_source_incremental(source, None)
                .expect("authored source");
            let foreign = parser
                .parse_source_incremental(source, None)
                .expect("independent equal bytes");
            let mut carriers = [0; 4];
            let mut body_states = [0; 3];
            let mut choices = [0; 5];
            let mut checks: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::new());
            let mut pairs = [0; 2];
            macro_rules! observe_pairs {
                ($owner:expr, $counter:expr) => {
                    for element in &$owner.children().child_1.slot {
                        match &element.slot {
                            ReadableSlot::Present(pair) => {
                                $counter += 1;
                                if let ReadableSlot::Present(separator) = &pair.child_0.slot {
                                    assert_eq!(separator.source(), source);
                                    assert!(matches!(
                                        foreign.bind(separator.raw_node()),
                                        Err(SourceBindingError::ForeignTree)
                                    ));
                                }
                                if let ReadableSlot::Present(content) = &pair.child_1.slot {
                                    assert_eq!(content.source(), source);
                                    assert!(matches!(
                                        foreign.bind(content.raw_node()),
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
                };
            }
            for node in parsed.root().expect("root").descendants() {
                let node = node.expect("canonical source-bound descendant");
                if let Some(tier) = node.typed::<SinDependentTierNode>() {
                    carriers[0] += 1;
                    assert!(matches!(
                        foreign.bind(tier.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let reading = SinTierReading::extract(tier).expect("kind-admitted root");
                    let before = match reading.children.field_child_2().slot().view() {
                        SourceSlotView::Present(_) => 0,
                        SourceSlotView::Error(_) => 1,
                        SourceSlotView::Absent(NoChild) => 2,
                    };
                    let separator = reading.separator;
                    let ranged = reading.admit_ranges().expect("selected root ranges");
                    let after = match &ranged.children.children().child_2.slot {
                        ReadableSlot::Present(_) => 0,
                        ReadableSlot::Error(_) => 1,
                        ReadableSlot::Absent(NoChild) => 2,
                        ReadableSlot::Missing(never) => match never.0 {},
                        ReadableSlot::Unexpected(never) => match *never {},
                    };
                    assert_eq!(after, before);
                    body_states[after] += 1;
                    assert_eq!(ranged.children.parent().raw_node(), tier.raw_node());
                    assert_eq!(ranged.children.parent().source(), source);
                    let count = ranged.children.checked_range_count();
                    for _ in 0..100 {
                        assert_eq!(ranged.children.parent().text(), tier.text());
                        assert_eq!(ranged.children.checked_range_count(), count);
                    }
                    checks[0].push(count);
                    let errors = ErrorCollector::new();
                    let entry = ranged
                        .parse(&errors)
                        .expect("native lowering")
                        .into_entry(DependentTier::Sin);
                    assert_eq!(entry.span(), span_of(tier.raw_node()));
                    assert_eq!(entry.separator, separator);
                    assert!(errors.is_empty(), "legal authored roots: {name}");
                }
                if let Some(groups) = node.typed::<SinGroupsNode>() {
                    carriers[1] += 1;
                    assert!(matches!(
                        foreign.bind(groups.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let ranged = groups
                        .extract_admitted(
                            crate::parser::typed_cst::canonical_grammar().expect("binding"),
                        )
                        .expect("kind-admitted groups")
                        .admit_ranges()
                        .expect("groups and inline repeats");
                    assert_eq!(ranged.parent().raw_node(), groups.raw_node());
                    assert_eq!(ranged.parent().text(), groups.text());
                    assert_eq!(ranged.parent().source(), source);
                    checks[1].push(ranged.checked_range_count());
                    observe_pairs!(ranged, pairs[0]);
                }
                if let Some(group) = node.typed::<SinGroupNode>() {
                    carriers[2] += 1;
                    assert!(matches!(
                        foreign.bind(group.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let reading = group
                        .extract_admitted(
                            crate::parser::typed_cst::canonical_grammar().expect("binding"),
                        )
                        .expect("kind-admitted group");
                    let before = match reading.field_content().slot().view() {
                        SourceSlotView::Present(choice) => match choice.view() {
                            AdmittedSinGroupChoiceSourceView::SinWord(_) => 0,
                            AdmittedSinGroupChoiceSourceView::SinBeginGroup(_) => 1,
                        },
                        SourceSlotView::Missing(_) => 2,
                        SourceSlotView::Error(_) => 3,
                        SourceSlotView::Absent(NoChild) => 4,
                        SourceSlotView::Unexpected(never) => match never {},
                    };
                    let ranged = reading
                        .admit_ranges()
                        .expect("complete flat/bracketed choice");
                    let after = match &ranged.children().content.slot {
                        ReadableSlot::Present(SinGroupChoice::SinWord(_)) => 0,
                        ReadableSlot::Present(SinGroupChoice::SinBeginGroup(_)) => 1,
                        ReadableSlot::Missing(_) => 2,
                        ReadableSlot::Error(_) => 3,
                        ReadableSlot::Absent(NoChild) => 4,
                        ReadableSlot::Unexpected(never) => match *never {},
                    };
                    assert_eq!(after, before);
                    choices[after] += 1;
                    assert_eq!(ranged.parent().raw_node(), group.raw_node());
                    assert_eq!(ranged.parent().source(), source);
                    let count = ranged.checked_range_count();
                    for _ in 0..100 {
                        assert_eq!(ranged.parent().text(), group.text());
                        if let ReadableSlot::Present(SinGroupChoice::SinWord(word)) =
                            &ranged.children().content.slot
                        {
                            assert_eq!(word.source(), source);
                            assert!(!word.text().is_empty());
                        }
                        assert_eq!(ranged.checked_range_count(), count);
                    }
                    checks[2].push(count);
                }
                if let Some(content) = node.typed::<SinGroupedContentNode>() {
                    carriers[3] += 1;
                    assert!(matches!(
                        foreign.bind(content.raw_node()),
                        Err(SourceBindingError::ForeignTree)
                    ));
                    let ranged = content
                        .extract_admitted(
                            crate::parser::typed_cst::canonical_grammar().expect("binding"),
                        )
                        .expect("kind-admitted interior")
                        .admit_ranges()
                        .expect("interior and inline token repeats");
                    assert_eq!(ranged.parent().raw_node(), content.raw_node());
                    assert_eq!(ranged.parent().text(), content.text());
                    assert_eq!(ranged.parent().source(), source);
                    checks[3].push(ranged.checked_range_count());
                    observe_pairs!(ranged, pairs[1]);
                }
            }
            for (seen, count) in witnessed.iter_mut().zip(carriers) {
                *seen |= count > 0;
            }
            assert!(pairs[0] > 0, "reference needs repeated groups");
            if name == "grouped" {
                assert!(
                    choices[1] > 0 && pairs[1] > 0,
                    "reference needs bracketed content and repeated tokens"
                );
            }
            eprintln!(
                "sin selected {name}: carriers={carriers:?} body[present,error,absent]={body_states:?} choice[word,group,missing,error,absent]={choices:?} pairs[groups,content]={pairs:?} checks={checks:?}; inline reading={} ranged={}",
                std::mem::size_of::<SinTierReading<'_, '_>>(),
                std::mem::size_of::<RangedSinTier<'_, '_>>()
            );
        }
        assert!(
            witnessed.into_iter().all(|seen| seen),
            "all four actual carriers required"
        );
    }
}
