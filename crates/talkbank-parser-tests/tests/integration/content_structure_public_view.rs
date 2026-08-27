//! `ContentStructure` seen from OUTSIDE `talkbank-model`, through a real parse.
//!
//! Two things are pinned here, and neither can be checked from inside the
//! defining crate.
//!
//! **That the classification is reachable at all.** It was `pub(crate)` until
//! v0.11.0, so every downstream walker re-derived the container set by hand,
//! and two of them broke within a day of v0.10.0 adding `AnnotatedRetrace`,
//! both compiling clean. A test inside `talkbank-model` sees the type either
//! way and would have stayed green throughout; only a consumer notices. Same
//! reasoning as `closed_newtype_consumer_view`.
//!
//! **That the annotation axis is answerable for every spelling.** It used not
//! to be: an annotated group kept its annotations, an annotated retrace had
//! them dropped by the classifier, and an annotated event became a
//! payload-free `Other`. A downstream crate that wanted "does this item carry
//! annotations" therefore hand-rolled a 22-arm match, which is the duplication
//! this type exists to prevent.
//!
//! POLICY, not an invariant a signature can carry: WHICH items count as
//! carrying annotations is a modelling decision, so it is pinned rather than
//! left to a reader.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{ContentStructure, GroupRef, LeafContent, RetraceRef, WordRef};
use talkbank_parser::TreeSitterParser;

use talkbank_parser_tests::test_error::TestError;

/// Classify every content item of one main tier, from outside the model crate.
fn classify(fragment: &str) -> Result<Vec<(&'static str, usize)>, TestError> {
    let parser = TreeSitterParser::new()?;
    let errors = ErrorCollector::new();
    let main = parser
        .parse_main_tier_fragment(fragment, 0, &errors)
        .into_option()
        .ok_or_else(|| TestError::Failure(format!("fragment did not parse: {fragment}")))?;
    Ok(main
        .content
        .content
        .iter()
        .map(|item| {
            let structure = item.structure();
            let label = match structure {
                ContentStructure::Word(WordRef::Bare(_)) => "word",
                ContentStructure::Word(WordRef::Annotated(_)) => "annotated-word",
                ContentStructure::Word(WordRef::Replaced(_)) => "replaced-word",
                ContentStructure::Retrace(RetraceRef::Bare(_)) => "retrace",
                ContentStructure::Retrace(RetraceRef::Annotated(_)) => "annotated-retrace",
                ContentStructure::Group(GroupRef::Angle(group)) => {
                    if group.annotations.is_empty() {
                        "group"
                    } else {
                        "annotated-group"
                    }
                }
                // The two spellings stay DISTINGUISHABLE, as a FIELD rather
                // than a variant. That is the point of the struct shape: a
                // caller that wants the difference asks for it, and a caller
                // asking only "is this a quotation" cannot write half the
                // answer, because there is no second arm to omit.
                ContentStructure::Group(GroupRef::Quotation(quotation)) => {
                    if quotation.annotations.is_empty() {
                        "quotation"
                    } else {
                        "annotated-quotation"
                    }
                }
                ContentStructure::Group(GroupRef::Pho(_)) => "pho",
                ContentStructure::Group(GroupRef::Sin(_)) => "sin",
                ContentStructure::Leaf(leaf) => match leaf.content {
                    LeafContent::Spoken => "spoken-leaf",
                    LeafContent::Notation => "notation-leaf",
                },
            };
            (label, structure.scoped_annotations().len())
        })
        .collect())
}

/// Every annotated spelling reports its own annotations, none reports another's.
#[test]
fn the_annotation_axis_is_answerable_for_every_spelling() -> Result<(), TestError> {
    // An annotated WORD.
    assert_eq!(
        classify("*CHI:\tdog [* p:w] .")?,
        vec![("annotated-word", 1)]
    );
    // An annotated RETRACE. This is the one the classifier used to drop: it
    // reached the node through `&annotated.inner` and the annotations went
    // nowhere, so the count here was 0 and no caller could recover it.
    assert_eq!(
        classify("*CHI:\tdog [/] [* p:w] dog .")?,
        vec![("annotated-retrace", 1), ("word", 0),]
    );
    // An annotated EVENT. It is SPOKEN and it carries annotations; the
    // predecessor made those one axis and reported only the second.
    assert_eq!(
        classify("*CHI:\t&=laughs [* p:w] .")?,
        vec![("spoken-leaf", 1)]
    );
    // A REPLACED word. `dog [: cat] [* p:w]` carries scoped annotations of its
    // own, which two rendering paths and the alignment units already read; the
    // first cut of this accessor returned an empty slice for it, so the fix for
    // Shape C shipped with Shape C in it, and no test covered the spelling.
    assert_eq!(
        classify("*CHI:\tdog [: cat] [* p:w] .")?,
        vec![("replaced-word", 1)]
    );
    // An annotated GROUP, which always kept them; pinned so the four stay
    // consistent rather than only the one that happened to work.
    assert_eq!(
        classify("*CHI:\t<the dog> [* p:w] .")?,
        vec![("annotated-group", 1)]
    );
    Ok(())
}

/// Unannotated items report no annotations, rather than reporting nothing.
#[test]
fn an_unannotated_item_reports_an_empty_slice_not_an_absence() -> Result<(), TestError> {
    assert_eq!(classify("*CHI:\tdog .")?, vec![("word", 0)]);
    assert_eq!(
        classify("*CHI:\tdog [/] dog .")?,
        vec![("retrace", 0), ("word", 0)]
    );
    Ok(())
}

/// A retrace hands back its node in either spelling, so a caller that wants
/// the retraced material does not have to re-match the enum.
#[test]
fn a_retrace_ref_yields_its_node_in_either_spelling() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let errors = ErrorCollector::new();
    for fragment in ["*CHI:\tdog [/] dog .", "*CHI:\tdog [/] [* p:w] dog ."] {
        let main = parser
            .parse_main_tier_fragment(fragment, 0, &errors)
            .into_option()
            .ok_or_else(|| TestError::Failure(format!("fragment did not parse: {fragment}")))?;
        let first = main
            .content
            .content
            .iter()
            .next()
            .ok_or_else(|| TestError::Failure("no content".to_owned()))?;
        match first.structure() {
            ContentStructure::Retrace(retrace) => {
                assert!(
                    !retrace.inner().content.content.is_empty(),
                    "the retraced material should be reachable for {fragment}"
                );
            }
            other => {
                return Err(TestError::Failure(format!(
                    "expected a retrace for {fragment}, got {other:?}"
                )));
            }
        }
    }
    Ok(())
}
