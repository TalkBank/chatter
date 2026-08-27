//! Appliers for simple text-like dependent tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # One applier per tier, taking that tier's OWN typed node
//!
//! This module used to expose a single `apply_raw_tier(utterance, tier_kind:
//! &str, tier_node: Node, ..) -> bool` whose body was a fifteen-arm
//! `match tier_kind` that re-derived, from the string, the typed wrapper its
//! caller had ALREADY held and thrown away: [`super::parse`] matches the
//! generated `UtteranceChild1Choice`, so at every one of those call sites the
//! concrete `XDependentTierNode` was in hand, was reduced to
//! `(kind constant, raw node)`, and was rebuilt here by an unchecked
//! `XDependentTierNode(tier_node)` that no longer compiles.
//!
//! Nothing tied the arm key to the wrapper it constructed or to the
//! `extract_*` it called: `ORT_DEPENDENT_TIER => extract_eng_dependent_tier(
//! EngDependentTierNode(tier_node))` type-checked and would have populated an
//! `%ort` tier by reading the node as `%eng`. Taking the typed node makes that
//! unwritable, because `extract_ort_dependent_tier` accepts an
//! `OrtDependentTierNode` and nothing else.
//!
//! What that removed: the `tier_kind` parameter and its fifteen call-site
//! constants, the fifteen-arm string match, its `_ => return false` fallthrough
//! (unreachable once the caller is exhaustive over the typed choice), the
//! `bool` return nobody read, and ten verbatim copies of an identical
//! fourteen-line body. The three genuinely different shapes stay visibly
//! different: the uniform ten, the two syllable tiers, and the two tiers whose
//! content parser can fail.

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AltDependentTierNode, AsRawNode, CohDependentTierNode, DefDependentTierNode,
    EngDependentTierNode, ErrDependentTierNode, FacDependentTierNode, FloDependentTierNode,
    GlsDependentTierNode, ModsylDependentTierNode, OrtDependentTierNode, ParDependentTierNode,
    PhoalnDependentTierNode, PhosylDependentTierNode, TimDependentTierNode,
    XphointDependentTierNode, extract_alt_dependent_tier, extract_coh_dependent_tier,
    extract_def_dependent_tier, extract_eng_dependent_tier, extract_err_dependent_tier,
    extract_fac_dependent_tier, extract_flo_dependent_tier, extract_gls_dependent_tier,
    extract_modsyl_dependent_tier, extract_ort_dependent_tier, extract_par_dependent_tier,
    extract_phoaln_dependent_tier, extract_phosyl_dependent_tier, extract_tim_dependent_tier,
    extract_xphoint_dependent_tier,
};
use crate::model::Utterance;
use crate::model::dependent_tier::{DependentTier, DependentTierEntry};
use crate::parser::node_span::span_of;
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{
    PhoalnTier, SylTier, SylTierType, XphointTier, parse_phoaln_content, parse_syl_content,
    parse_xphoint_content,
};

use super::helpers::{read_optional_tier_body_raw_text, read_optional_tier_body_text};

/// A tier is pushed even when its body is ABSENT. That is the E756 widening: an
/// empty tier line is a real (if invalid) construct, so the parser records what
/// the file contains and `DependentTier::declares_nothing` lets the validator
/// judge it. Dropping the tier is what used to lose the line on roundtrip.
///
/// Generates the ten tiers whose whole lowering is "read the optional body as
/// text, wrap it in a `TextTier`, push it under this tier's variant". Each entry
/// is the (typed node type, `extract_*`, `DependentTier` variant) triple, and
/// the three are checked against each other by the compiler: an `extract_*` that
/// does not accept that node type, or a variant that does not accept a
/// `TextTier`, is a build error rather than a tier populated from the wrong line.
macro_rules! plain_text_tier_appliers {
    ($(
        $(#[$meta:meta])*
        $name:ident : $node:ident via $extract:ident => $variant:ident;
    )*) => {
        $(
            $(#[$meta])*
            /// Read this tier's optional text body and attach it to `utterance`.
            pub(super) fn $name(
                utterance: &mut Utterance,
                node: $node<'_>,
                input: &str,
                errors: &impl ErrorSink,
            ) {
                let raw = node.raw_node();
                let span = span_of(raw);
                let children = $extract(node);
                let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
                if let ParseOutcome::Parsed(tier) = read_optional_tier_body_text(
                    raw,
                    children.child_2.slot(),
                    &children.unexpected,
                    input,
                    errors,
                ) {
                    utterance
                        .dependent_tiers
                        .push(DependentTierEntry::with_separator(
                            DependentTier::$variant(tier.with_span(span)),
                            separator,
                        ));
                }
            }
        )*
    };
}

plain_text_tier_appliers! {
    apply_ort: OrtDependentTierNode via extract_ort_dependent_tier => Ort;
    apply_eng: EngDependentTierNode via extract_eng_dependent_tier => Eng;
    apply_gls: GlsDependentTierNode via extract_gls_dependent_tier => Gls;
    apply_alt: AltDependentTierNode via extract_alt_dependent_tier => Alt;
    apply_coh: CohDependentTierNode via extract_coh_dependent_tier => Coh;
    apply_def: DefDependentTierNode via extract_def_dependent_tier => Def;
    apply_err: ErrDependentTierNode via extract_err_dependent_tier => Err;
    apply_fac: FacDependentTierNode via extract_fac_dependent_tier => Fac;
    apply_flo: FloDependentTierNode via extract_flo_dependent_tier => Flo;
    apply_par: ParDependentTierNode via extract_par_dependent_tier => Par;
}

/// Generates `%modsyl` and `%phosyl`, which differ only in the [`SylTierType`]
/// they stamp and the `DependentTier` variant they push. They read the body as
/// RAW text rather than as a `TextTier` because they lower it themselves.
macro_rules! syl_tier_appliers {
    ($(
        $name:ident : $node:ident via $extract:ident => $variant:ident as $syl_type:ident;
    )*) => {
        $(
            /// Read this tier's optional body, parse it into syllable words, and
            /// attach it to `utterance`. An absent body yields no words, which is
            /// what `SylTier::is_empty` reports and E756 judges; the tier stays in
            /// the model either way.
            pub(super) fn $name(
                utterance: &mut Utterance,
                node: $node<'_>,
                input: &str,
                errors: &impl ErrorSink,
            ) {
                let raw = node.raw_node();
                let span = span_of(raw);
                let children = $extract(node);
                let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
                if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                    raw,
                    children.child_2.slot(),
                    &children.unexpected,
                    input,
                    errors,
                ) {
                    let words = match &content {
                        Some(text) => parse_syl_content(text.as_str()),
                        None => Vec::new(),
                    };
                    utterance
                        .dependent_tiers
                        .push(DependentTierEntry::with_separator(
                            DependentTier::$variant(
                                SylTier::new(SylTierType::$syl_type, words).with_span(span),
                            ),
                            separator,
                        ));
                }
            }
        )*
    };
}

syl_tier_appliers! {
    apply_modsyl: ModsylDependentTierNode via extract_modsyl_dependent_tier => Modsyl as Modsyl;
    apply_phosyl: PhosylDependentTierNode via extract_phosyl_dependent_tier => Phosyl as Phosyl;
}

/// Generates `%phoaln` and `%xphoint`, the two whose content parser can FAIL.
///
/// An absent body is the empty tier, which the tier type's `is_empty` reports
/// and E756 judges. Only a body that IS there and does not parse is a
/// malformed-content error, so the absent case never reaches the content parser.
macro_rules! fallible_content_tier_appliers {
    ($(
        $name:ident : $node:ident via $extract:ident => $variant:ident
            using $parse:ident into $tier:ident labelled $label:literal;
    )*) => {
        $(
            /// Read this tier's optional body, run its content parser, and attach
            /// the result to `utterance`; report a malformed-content error if the
            /// parser refuses a body that was present.
            pub(super) fn $name(
                utterance: &mut Utterance,
                node: $node<'_>,
                input: &str,
                errors: &impl ErrorSink,
            ) {
                let raw = node.raw_node();
                let span = span_of(raw);
                let children = $extract(node);
                let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
                if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
                    raw,
                    children.child_2.slot(),
                    &children.unexpected,
                    input,
                    errors,
                ) {
                    let parsed = match &content {
                        Some(text) => $parse(text.as_str()),
                        None => Ok(Vec::new()),
                    };
                    match parsed {
                        Ok(items) => {
                            utterance
                                .dependent_tiers
                                .push(DependentTierEntry::with_separator(
                                    DependentTier::$variant($tier::new(items).with_span(span)),
                                    separator,
                                ));
                        }
                        Err(e) => {
                            errors.report(ParseError::new(
                                ErrorCode::InvalidDependentTier,
                                Severity::Error,
                                SourceLocation::from_offsets(raw.start_byte(), raw.end_byte()),
                                ErrorContext::new(
                                    input,
                                    raw.start_byte()..raw.end_byte(),
                                    $label,
                                ),
                                format!("malformed {} content: {}", $label, e),
                            ));
                        }
                    }
                }
            }
        )*
    };
}

fallible_content_tier_appliers! {
    apply_phoaln: PhoalnDependentTierNode via extract_phoaln_dependent_tier => Phoaln
        using parse_phoaln_content into PhoalnTier labelled "%phoaln";
    apply_xphoint: XphointDependentTierNode via extract_xphoint_dependent_tier => Xphoint
        using parse_xphoint_content into XphointTier labelled "%xphoint";
}

/// Read `%tim`'s optional body and attach it to `utterance`.
///
/// The only free-text tier with no sibling: it classifies its text into time
/// segments, so it is written out rather than generated. An absent body is
/// `TimTier::Empty`, not a fabricated time and not a dropped tier: E756 judges
/// it and the line roundtrips.
pub(super) fn apply_tim(
    utterance: &mut Utterance,
    node: TimDependentTierNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) {
    let raw = node.raw_node();
    let span = span_of(raw);
    let children = extract_tim_dependent_tier(node);
    let separator = super::helpers::dependent_tier_separator(children.child_1.slot());
    if let ParseOutcome::Parsed(content) = read_optional_tier_body_raw_text(
        raw,
        children.child_2.slot(),
        &children.unexpected,
        input,
        errors,
    ) {
        let tier = match content {
            Some(text) => crate::model::dependent_tier::TimTier::from_text(text),
            None => crate::model::dependent_tier::TimTier::empty(),
        };
        utterance
            .dependent_tiers
            .push(DependentTierEntry::with_separator(
                DependentTier::Tim(tier.with_span(span)),
                separator,
            ));
    }
}
