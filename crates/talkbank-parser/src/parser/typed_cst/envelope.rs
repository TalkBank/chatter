//! Per-rule admission for ordinary flat header and dependent-tier envelopes.
//!
//! Eligibility comes from the generated descriptor; admission still checks the
//! compiled language. Each implementation owns its own cache, including refusal.
//! Nested contents and ERROR reconstruction do not acquire this capability.

use super::{CstFailure, canonical_grammar};
use crate::generated_traversal::{
    ExtractRequired, RequiredRule, RequiredRuleType, SourceBound, SourceBoundKind, SourceChildren,
};

pub(in crate::parser) trait EnvelopeRule:
    RequiredRuleType + Sized + 'static
{
    fn admitted() -> Result<&'static RequiredRule<Self>, CstFailure>;
}

macro_rules! envelope_rules {
    ($($rule:ident => $node:ident),+ $(,)?) => {
        $(
            impl EnvelopeRule for crate::generated_traversal::$rule {
                fn admitted() -> Result<&'static RequiredRule<Self>, CstFailure> {
                    // This is a concrete per-rule static, not a static in a
                    // generic function shared by different monomorphizations.
                    static RULE: std::sync::OnceLock<
                        Result<RequiredRule<crate::generated_traversal::$rule>, CstFailure>,
                    > = std::sync::OnceLock::new();
                    RULE.get_or_init(|| canonical_grammar()?.required_rule().map_err(Into::into))
                        .as_ref()
                        .map_err(|fault| *fault)
                }
            }
        )+

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::generated_traversal::NamedKind;

            #[test]
            fn flat_envelope_rules_admit_the_compiled_grammar() {
                let mut refusals = Vec::new();
                $(
                    if let Err(fault) = crate::generated_traversal::$rule::admitted() {
                        refusals.push((stringify!($rule), fault));
                    }
                )+
                assert!(refusals.is_empty(), "compiled envelope refusals: {refusals:?}");
            }

            /// Each requested parent must reject a shortened word, even when
            /// that word uses a real grammar token. Eligibility is not proof.
            #[test]
            #[allow(unsafe_code, reason = "exercise the actual static metadata bridge with owned native language and graph")]
            fn flat_envelope_rules_refuse_truncated_words() {
                let language: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
                let raw = language.into_raw();
                // SAFETY: the statically linked bridge and parser share the
                // compiled header. The language outlives every graph query.
                let graph = unsafe { (tree_sitter_talkbank::REQUIRED_GRAPH_NEW)(raw.cast()) };
                let mut admitted = Vec::new();
                if !graph.is_null() {
                    let children = [c"newline".as_ptr()];
                    $(
                        let parent = std::ffi::CString::new(
                            crate::generated_traversal::$node::KIND.name(),
                        ).expect("generated kind has no NUL");
                        // SAFETY: graph and strings remain alive, the word
                        // length matches the array, and there is no optional slot.
                        let verdict = unsafe {
                            (tree_sitter_talkbank::REQUIRED_RULE)(
                                graph, parent.as_ptr(), children.as_ptr(), children.len(), children.len(),
                            )
                        };
                        if verdict != 0 { admitted.push((stringify!($rule), verdict)); }
                    )+
                    // SAFETY: exactly one matching destruction after all reads.
                    unsafe { (tree_sitter_talkbank::REQUIRED_GRAPH_DROP)(graph) };
                }
                // SAFETY: recover the one owned language passed to into_raw.
                drop(unsafe { tree_sitter::Language::from_raw(raw) });
                assert!(!graph.is_null(), "native graph allocation must succeed");
                assert!(admitted.is_empty(), "truncated envelope words: {admitted:?}");
            }
        }
    };
}

envelope_rules! {
    ActDependentTierChildrenRule => ActDependentTierNode,
    ActivitiesHeaderChildrenRule => ActivitiesHeaderNode,
    AddDependentTierChildrenRule => AddDependentTierNode,
    AltDependentTierChildrenRule => AltDependentTierNode,
    BckHeaderChildrenRule => BckHeaderNode,
    BirthplaceOfHeaderChildrenRule => BirthplaceOfHeaderNode,
    BlankHeaderChildrenRule => BlankHeaderNode,
    CodDependentTierChildrenRule => CodDependentTierNode,
    CohDependentTierChildrenRule => CohDependentTierNode,
    ColorWordsHeaderChildrenRule => ColorWordsHeaderNode,
    ComDependentTierChildrenRule => ComDependentTierNode,
    CommentHeaderChildrenRule => CommentHeaderNode,
    DateHeaderChildrenRule => DateHeaderNode,
    DefDependentTierChildrenRule => DefDependentTierNode,
    EngDependentTierChildrenRule => EngDependentTierNode,
    ErrDependentTierChildrenRule => ErrDependentTierNode,
    ExpDependentTierChildrenRule => ExpDependentTierNode,
    FacDependentTierChildrenRule => FacDependentTierNode,
    FloDependentTierChildrenRule => FloDependentTierNode,
    FontHeaderChildrenRule => FontHeaderNode,
    GlsDependentTierChildrenRule => GlsDependentTierNode,
    GpxDependentTierChildrenRule => GpxDependentTierNode,
    GraDependentTierChildrenRule => GraDependentTierNode,
    IdHeaderChildrenRule => IdHeaderNode,
    IntDependentTierChildrenRule => IntDependentTierNode,
    L1OfHeaderChildrenRule => L1OfHeaderNode,
    LanguagesHeaderChildrenRule => LanguagesHeaderNode,
    LocationHeaderChildrenRule => LocationHeaderNode,
    MediaHeaderChildrenRule => MediaHeaderNode,
    ModDependentTierChildrenRule => ModDependentTierNode,
    ModsylDependentTierChildrenRule => ModsylDependentTierNode,
    MorDependentTierChildrenRule => MorDependentTierNode,
    NewEpisodeHeaderChildrenRule => NewEpisodeHeaderNode,
    NumberHeaderChildrenRule => NumberHeaderNode,
    OptionsHeaderChildrenRule => OptionsHeaderNode,
    OrtDependentTierChildrenRule => OrtDependentTierNode,
    PageHeaderChildrenRule => PageHeaderNode,
    ParDependentTierChildrenRule => ParDependentTierNode,
    ParticipantsHeaderChildrenRule => ParticipantsHeaderNode,
    PhoDependentTierChildrenRule => PhoDependentTierNode,
    PhoalnDependentTierChildrenRule => PhoalnDependentTierNode,
    PhosylDependentTierChildrenRule => PhosylDependentTierNode,
    PidHeaderChildrenRule => PidHeaderNode,
    RecordingQualityHeaderChildrenRule => RecordingQualityHeaderNode,
    RoomLayoutHeaderChildrenRule => RoomLayoutHeaderNode,
    SinDependentTierChildrenRule => SinDependentTierNode,
    SitDependentTierChildrenRule => SitDependentTierNode,
    SituationHeaderChildrenRule => SituationHeaderNode,
    SpaDependentTierChildrenRule => SpaDependentTierNode,
    THeaderChildrenRule => THeaderNode,
    TapeLocationHeaderChildrenRule => TapeLocationHeaderNode,
    ThumbnailHeaderChildrenRule => ThumbnailHeaderNode,
    TimDependentTierChildrenRule => TimDependentTierNode,
    TimeDurationHeaderChildrenRule => TimeDurationHeaderNode,
    TimeStartHeaderChildrenRule => TimeStartHeaderNode,
    TranscriberHeaderChildrenRule => TranscriberHeaderNode,
    TranscriptionHeaderChildrenRule => TranscriptionHeaderNode,
    UnsupportedHeaderChildrenRule => UnsupportedHeaderNode,
    VideosHeaderChildrenRule => VideosHeaderNode,
    WarningHeaderChildrenRule => WarningHeaderNode,
    WindowHeaderChildrenRule => WindowHeaderNode,
    WorDependentTierChildrenRule => WorDependentTierNode,
    XDependentTierChildrenRule => XDependentTierNode,
    XphointDependentTierChildrenRule => XphointDependentTierNode,
}

/// Read an ordinary envelope with its own rule evidence and source owner.
pub(in crate::parser) fn extract_envelope<'tree, 'source, W>(
    node: SourceBound<'tree, 'source, W>,
) -> Result<SourceChildren<'tree, 'source, W::RequiredChildren>, CstFailure>
where
    W: ExtractRequired<'tree> + SourceBoundKind<'tree>,
    W::Rule: EnvelopeRule,
{
    node.extract_required(W::Rule::admitted()?)
        .map_err(Into::into)
}
