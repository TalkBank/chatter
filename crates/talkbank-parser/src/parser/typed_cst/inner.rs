//! Cached compiled presence admission for ordinary inner nodes and MOR prefixes.
//! Nested payload validity, lexical Missing and optional recovery stay separate.

use super::{CstFailure, canonical_grammar};
use crate::generated_traversal::{
    ExtractRequired, RequiredRule, RequiredRuleType, SourceBound, SourceBoundKind, SourceChildren,
};

pub(in crate::parser) trait InnerRule:
    RequiredRuleType + Sized + 'static
{
    fn admitted() -> Result<&'static RequiredRule<Self>, CstFailure>;
}

macro_rules! inner_rules {
    ($($rule:ident => $node:ident),+ $(,)?) => {
        $(
            impl InnerRule for crate::generated_traversal::$rule {
                fn admitted() -> Result<&'static RequiredRule<Self>, CstFailure> {
                    static RULE: std::sync::OnceLock<
                        Result<RequiredRule<crate::generated_traversal::$rule>, CstFailure>,
                    > = std::sync::OnceLock::new();
                    RULE.get_or_init(|| canonical_grammar()?.required_rule().map_err(Into::into))
                        .as_ref().map_err(|fault| *fault)
                }
            }
        )+
        #[cfg(test)]
        #[allow(clippy::expect_used)]
        mod tests {
            use super::*;
            use crate::generated_traversal::NamedKind;

            #[test]
            fn inner_rules_compiled_admission() {
                let mut refused = Vec::new();
                $(if let Err(fault) = crate::generated_traversal::$rule::admitted() {
                    refused.push((stringify!($rule), fault));
                })+
                assert!(refused.is_empty(), "compiled inner-node refusals: {refused:?}");
            }

            #[test]
            #[allow(unsafe_code, reason = "negative controls at the statically linked native proof boundary")]
            fn inner_rules_refuse_empty_and_wrong_words() {
                let language: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
                let raw = language.into_raw();
                // SAFETY: the linked bridge shares the parser header; its
                // language and every argument remain alive throughout queries.
                let graph = unsafe { (tree_sitter_talkbank::REQUIRED_GRAPH_NEW)(raw.cast()) };
                let mut accepted = Vec::new();
                if !graph.is_null() {
                    let wrong = [c"newline".as_ptr()];
                    $(
                        let parent = std::ffi::CString::new(
                            crate::generated_traversal::$node::KIND.name(),
                        ).expect("generated kind has no NUL");
                        for count in [0, 1] {
                            // SAFETY: count is within the live array; optional
                            // position equal to count means no optional child.
                            let verdict = unsafe { (tree_sitter_talkbank::REQUIRED_RULE)(
                                graph, parent.as_ptr(), wrong.as_ptr(), count, count,
                            ) };
                            if verdict != 0 { accepted.push((stringify!($rule), count, verdict)); }
                            // SAFETY: the prefix export borrows the same live arguments.
                            let verdict = unsafe { (tree_sitter_talkbank::REQUIRED_PREFIX)(
                                graph, parent.as_ptr(), wrong.as_ptr(), count,
                            ) };
                            if verdict != 0 { accepted.push((stringify!($rule), count, verdict)); }
                        }
                    )+
                    // SAFETY: the graph is destroyed exactly once after reads.
                    unsafe { (tree_sitter_talkbank::REQUIRED_GRAPH_DROP)(graph) };
                }
                // SAFETY: restore the single language ownership transferred above.
                drop(unsafe { tree_sitter::Language::from_raw(raw) });
                assert!(!graph.is_null(), "native graph allocation must succeed");
                assert!(accepted.is_empty(), "invalid inner-node words accepted: {accepted:?}");
            }
        }
    };
}

/// Extract through the exact cached rule and preserve the producing source.
pub(in crate::parser) fn extract_inner<'tree, 'source, W>(
    node: SourceBound<'tree, 'source, W>,
) -> Result<SourceChildren<'tree, 'source, W::RequiredChildren>, CstFailure>
where
    W: ExtractRequired<'tree> + SourceBoundKind<'tree>,
    W::Rule: InnerRule,
{
    node.extract_required(W::Rule::admitted()?)
        .map_err(Into::into)
}

// Only separators expose a geometry-only route. Do not give text-bearing
// inner nodes a generic raw extraction route around source ownership.
macro_rules! separator_geometry {
    ($($name:ident => $node:ident),+ $(,)?) => {$(
        pub(in crate::parser) fn $name<'tree>(
            node: crate::generated_traversal::$node<'tree>,
        ) -> Result<
            <crate::generated_traversal::$node<'tree> as ExtractRequired<'tree>>::RequiredChildren,
            CstFailure,
        > {
            type Rule = <crate::generated_traversal::$node<'static> as ExtractRequired<'static>>::Rule;
            node.extract_required(Rule::admitted()?).map_err(Into::into)
        }
    )+};
}

separator_geometry! {
    extract_header_separator => HeaderSepNode,
    extract_tier_separator => TierSepNode,
}

inner_rules! {
    MorWordChildrenRule => MorWordNode,
    MorContentChildrenRule => MorContentNode,
    MorFeatureChildrenRule => MorFeatureNode,
    MorPostCliticChildrenRule => MorPostCliticNode,
    GraRelationChildrenRule => GraRelationNode,
    EventChildrenRule => EventNode,
    OtherSpokenEventChildrenRule => OtherSpokenEventNode,
    GroupWithAnnotationsChildrenRule => GroupWithAnnotationsNode,
    MainPhoGroupChildrenRule => MainPhoGroupNode,
    MainSinGroupChildrenRule => MainSinGroupNode,
    NonwordWithOptionalAnnotationsChildrenRule => NonwordWithOptionalAnnotationsNode,
    QuotationChildrenRule => QuotationNode,
    QuotationWithOptionalAnnotationsChildrenRule => QuotationWithOptionalAnnotationsNode,
    LongFeatureBeginChildrenRule => LongFeatureBeginNode,
    LongFeatureEndChildrenRule => LongFeatureEndNode,
    NonvocalBeginChildrenRule => NonvocalBeginNode,
    NonvocalEndChildrenRule => NonvocalEndNode,
    NonvocalSimpleChildrenRule => NonvocalSimpleNode,
    HeaderSepChildrenRule => HeaderSepNode,
    TierSepChildrenRule => TierSepNode,
}
