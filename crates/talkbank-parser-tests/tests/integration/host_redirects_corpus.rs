//! Host dependents of a spliced `%mor` item keep depending on the same word.
//!
//! The fixtures are the L2 `%mor`/`%gra` shapes `@s` spans produce, written
//! as CHAT and parsed (no models, nothing fabricated): a host utterance with
//! its primary parse, and one donor utterance per span whose first items and
//! relations are the span's reparse, already block-relative. Each span is
//! spliced the way an L2 caller does, one span at a time, with the span root
//! anchored where its last word was before that splice: under the same host
//! head, with the same relation ([`anchor_of`]).

use std::num::NonZeroUsize;

use talkbank_model::alignment::indices::{GraHeadRef, SemanticWordIndex1};
// Imported from `dependent_tier`, where callers find them beside the tiers.
use talkbank_model::model::dependent_tier::{
    AttachmentRelation, BlockChunk, CoordinatedMutationError, HostRedirects, ItemTarget, SpanRoot,
    SplicedBlock,
};
use talkbank_model::model::{GraTier, Mor, MorTier};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::strict_parse;

/// The `%mor` and `%gra` tiers of each utterance of `body` (the lines
/// between the headers and `@End`), in order.
fn tiers(body: &str) -> Vec<(MorTier, GraTier)> {
    let source = format!(
        "@UTF8\n@Begin\n@Languages:\tdeu, eng, spa\n@Participants:\tCHI Target_Child\n\
         @ID:\tdeu|test|CHI|||||Target_Child|||\n{body}@End\n"
    );
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("fixture parses");
    file.utterances()
        .map(|utterance| {
            (
                utterance.mor_tier().expect("%mor").clone(),
                utterance.gra_tier().expect("%gra").clone(),
            )
        })
        .collect()
}

/// A donor's first `items` items and the relations of their chunks: the
/// span's reparse as a splice takes it.
fn block(donor: &(MorTier, GraTier), items: usize) -> SplicedBlock {
    let (mor, gra) = donor;
    let mors: Vec<Mor> = mor.items()[..items].to_vec();
    let chunks: usize = mors.iter().map(Mor::count_chunks).sum();
    SplicedBlock::new(mors, gra.relations()[..chunks].to_vec()).expect("the donor block is a tree")
}

/// Host chunk `n`, as a span root or a refusal names it.
fn chunk(n: usize) -> SemanticWordIndex1 {
    SemanticWordIndex1::new(n).expect("non-zero")
}

/// Block chunk `n`, as a redirect names it.
fn block_chunk(n: usize) -> BlockChunk {
    BlockChunk::new(NonZeroUsize::new(n).expect("non-zero"))
}

/// The span root that takes the place of host relation `index` (1-based),
/// as an L2 caller reads it before a splice: under that relation's head,
/// with its relation, or the utterance's root.
fn anchor_of(gra: &GraTier, index: usize) -> SpanRoot {
    let relation = gra
        .relations()
        .iter()
        .find(|relation| relation.index == index)
        .expect("relation present");
    match relation.head_ref() {
        GraHeadRef::Root => SpanRoot::UtteranceRoot,
        GraHeadRef::Word(head) => SpanRoot::HostChunk {
            chunk: head,
            relation: AttachmentRelation::new(relation.relation.clone())
                .expect("a dependent relation"),
        },
    }
}

/// A span root under host chunk `n` with a dependent relation.
fn under(n: usize) -> SpanRoot {
    SpanRoot::from_gra_head(
        GraHeadRef::Word(chunk(n)),
        AttachmentRelation::new("DEP").expect("a dependent relation"),
    )
}

/// Splice `donor`'s first item over `range` of a copy of `host`, and assert
/// that the attempt left both of the host's tiers unchanged: every caller
/// expects a refusal, which must be atomic.
fn refused_atomically(
    host: &(MorTier, GraTier),
    donor: &(MorTier, GraTier),
    range: std::ops::Range<usize>,
    root: SpanRoot,
    redirects: HostRedirects,
) -> Result<(), CoordinatedMutationError> {
    let (mut mor, mut gra) = host.clone();
    let outcome = mor.splice_range_coordinated(&mut gra, range, block(donor, 1), root, redirects);
    assert_eq!(mor, host.0, "a refusal changed %mor");
    assert_eq!(gra, host.1, "a refusal changed %gra");
    outcome
}

/// `dont@s:eng mal geh .` (`dont -> geh`, `mal -> dont`) and a donor
/// utterance whose `dont` is `do~n't`, one item of two chunks.
const GROWING_SPLICE: &str = "*CHI:\tdont@s:eng mal geh .\n\
     %mor:\tx|dont adv|mal v|gehen .\n\
     %gra:\t1|3|AUX 2|1|ADV 3|0|ROOT 4|3|PUNCT\n\
     *CHI:\tdont .\n\
     %mor:\taux|do~neg|not .\n\
     %gra:\t1|0|AUX 2|1|NEG 3|1|PUNCT\n";

fn written(gra: &GraTier) -> Vec<String> {
    gra.relations().iter().map(ToString::to_string).collect()
}

/// `ich glaube it's@s:eng working@s:eng und don't@s:eng stop@s:eng .` with
/// `und -> stop` and `stop -> working`. The two spans expand `it's` to
/// `it~'s` and `don't` to `do~n't`; afterwards `und` still depends on `stop`
/// (`6|9|CC`) and `stop` on `working` (`9|5|...`). Collapsing to the first
/// chunk of the new block made them `6|7|CC` (und -> do) and `9|3` (stop ->
/// it).
#[test]
fn host_dependents_follow_their_word_across_two_l2_spans() {
    let parsed = tiers(
        "*CHI:\tich glaube its@s:eng working@s:eng und dont@s:eng stop@s:eng .\n\
         %mor:\tpro|ich v|glauben x|its x|working conj|und x|dont x|stop .\n\
         %gra:\t1|2|SUBJ 2|0|ROOT 3|4|SUBJ 4|2|COMP 5|7|CC 6|7|AUX 7|4|CONJ 8|2|PUNCT\n\
         *CHI:\tits working .\n\
         %mor:\tpro|it~aux|be part|work .\n\
         %gra:\t1|3|SUBJ 2|3|AUX 3|0|COMP 4|3|PUNCT\n\
         *CHI:\tdont stop .\n\
         %mor:\taux|do~neg|not v|stop .\n\
         %gra:\t1|3|AUX 2|3|NEG 3|0|CONJ 4|3|PUNCT\n",
    );
    let (mut mor, mut gra) = parsed[0].clone();

    // Span 1: `its working` (items 2..4) as `it~'s working`.
    let root = anchor_of(&gra, 4);
    mor.splice_range_coordinated(
        &mut gra,
        2..4,
        block(&parsed[1], 2),
        root,
        HostRedirects::ByItem,
    )
    .expect("span 1");

    // Span 2: `dont stop` (items 5..7) as `do~n't stop`.
    let root = anchor_of(&gra, 8);
    mor.splice_range_coordinated(
        &mut gra,
        5..7,
        block(&parsed[2], 2),
        root,
        HostRedirects::ByItem,
    )
    .expect("span 2");

    assert_eq!(
        written(&gra),
        [
            "1|2|SUBJ",
            "2|0|ROOT",
            "3|5|SUBJ",
            "4|5|AUX",
            "5|2|COMP",
            "6|9|CC",
            "7|9|AUX",
            "8|9|NEG",
            "9|5|CONJ",
            "10|2|PUNCT",
        ]
    );
}

/// `we talked about los@s:spa niños@s:spa .` with `about -> niños`: the span
/// keeps its item shapes, so `about` keeps depending on `niños` (`3|5|CASE`),
/// not on the first word of the span (`3|4`).
#[test]
fn a_dependent_of_the_last_word_of_a_span_is_not_moved_to_its_first() {
    let parsed = tiers(
        "*CHI:\twe talked about los@s:spa niños@s:spa .\n\
         %mor:\tpro|we v|talk prep|about x|los x|niños .\n\
         %gra:\t1|2|SUBJ 2|0|ROOT 3|5|CASE 4|5|DET 5|2|OBL 6|2|PUNCT\n\
         *CHI:\tlos niños .\n\
         %mor:\tdet|el n|niño .\n\
         %gra:\t1|2|DET 2|0|OBL 3|2|PUNCT\n",
    );
    let (mut mor, mut gra) = parsed[0].clone();
    let root = anchor_of(&gra, 5);
    mor.splice_range_coordinated(
        &mut gra,
        3..5,
        block(&parsed[1], 2),
        root,
        HostRedirects::ByItem,
    )
    .expect("span");
    assert_eq!(
        written(&gra),
        [
            "1|2|SUBJ",
            "2|0|ROOT",
            "3|5|CASE",
            "4|5|DET",
            "5|2|OBL",
            "6|2|PUNCT"
        ]
    );
}

/// A word that becomes two chunks both headed outside it (`it~'s`, both
/// depending on `working`) has no head chunk to read off. That refuses only
/// when a host relation depends on that word, and it refuses before changing
/// anything; a stated per-item redirect places the dependent.
#[test]
fn an_ambiguous_head_chunk_refuses_only_when_needed_and_a_stated_one_places_it() {
    let parsed = tiers(
        "*CHI:\tits@s:eng working@s:eng .\n\
         %mor:\tx|its x|working .\n\
         %gra:\t1|2|SUBJ 2|0|ROOT 3|2|PUNCT\n\
         *CHI:\tits@s:eng working@s:eng .\n\
         %mor:\tx|its x|working .\n\
         %gra:\t1|2|SUBJ 2|0|ROOT 3|1|PUNCT\n\
         *CHI:\tits working .\n\
         %mor:\tpro|it~aux|be part|work .\n\
         %gra:\t1|3|SUBJ 2|3|AUX 3|0|ROOT 4|3|PUNCT\n",
    );
    let donor = &parsed[2];

    // Nothing outside depends on `its`: the ambiguity is never consulted.
    let (mut mor, mut gra) = parsed[0].clone();
    mor.splice_range_coordinated(
        &mut gra,
        0..2,
        block(donor, 2),
        SpanRoot::UtteranceRoot,
        HostRedirects::ByItem,
    )
    .expect("no dependent needs the ambiguous word");

    // The terminator depends on `its`: refused, and both tiers unchanged.
    let (host_mor, host_gra) = parsed[1].clone();
    let (mut mor, mut gra) = (host_mor.clone(), host_gra.clone());
    let refused = mor.splice_range_coordinated(
        &mut gra,
        0..2,
        block(donor, 2),
        SpanRoot::UtteranceRoot,
        HostRedirects::ByItem,
    );
    assert!(
        matches!(
            refused,
            Err(CoordinatedMutationError::NoUniqueHeadChunk {
                dependent: 3,
                item: 0,
                head_chunks: 2,
            })
        ),
        "{refused:?}"
    );
    assert_eq!(mor, host_mor);
    assert_eq!(gra, host_gra);

    // Stated for `its` alone: its dependents go to `'s` (chunk 2 of the
    // block), and `working` follows its counterpart `work`.
    mor.splice_range_coordinated(
        &mut gra,
        0..2,
        block(donor, 2),
        SpanRoot::UtteranceRoot,
        HostRedirects::PerItem(vec![
            ItemTarget::Chunk(block_chunk(2)),
            ItemTarget::Counterpart,
        ]),
    )
    .expect("stated redirects");
    assert_eq!(
        written(&gra),
        ["1|3|SUBJ", "2|3|AUX", "3|0|ROOT", "4|2|PUNCT"]
    );
}

/// Redirects that do not fit the replacement are refused before any change:
/// by item with different item counts, per item with the wrong number of
/// targets, a target outside the new block, and a counterpart the block does
/// not have.
#[test]
fn redirects_that_do_not_fit_are_refused_atomically() {
    let parsed = tiers(
        "*CHI:\tlos@s:spa niños@s:spa .\n\
         %mor:\tx|los x|niños .\n\
         %gra:\t1|2|DET 2|0|ROOT 3|2|PUNCT\n\
         *CHI:\tlosniños .\n\
         %mor:\tn|losniños .\n\
         %gra:\t1|0|ROOT 2|1|PUNCT\n",
    );
    let attempt = |redirects: HostRedirects| {
        refused_atomically(
            &parsed[0],
            &parsed[1],
            0..2,
            SpanRoot::UtteranceRoot,
            redirects,
        )
    };
    assert!(matches!(
        attempt(HostRedirects::ByItem),
        Err(CoordinatedMutationError::RedirectItemCountsDiffer {
            old_items: 2,
            new_items: 1,
        })
    ));
    assert!(matches!(
        attempt(HostRedirects::PerItem(vec![ItemTarget::Chunk(
            BlockChunk::FIRST
        )])),
        Err(CoordinatedMutationError::RedirectCountMismatch {
            targets: 1,
            old_items: 2,
        })
    ));
    assert_eq!(
        attempt(HostRedirects::PerItem(vec![
            ItemTarget::Chunk(BlockChunk::FIRST),
            ItemTarget::Chunk(block_chunk(2)),
        ])),
        Err(CoordinatedMutationError::RedirectOutOfBlock {
            item: 1,
            target: block_chunk(2),
            new_chunks: 1,
        })
    );
    // The second old item has no counterpart in a one-item block.
    assert_eq!(
        attempt(HostRedirects::PerItem(vec![
            ItemTarget::Chunk(BlockChunk::FIRST),
            ItemTarget::Counterpart,
        ])),
        Err(CoordinatedMutationError::NoCounterpart {
            item: 1,
            new_items: 1,
        })
    );
}

/// `dont@s:eng mal geh .` with `dont -> geh` and `mal -> dont`. The span
/// `dont` becomes `do~n't`, one item of two chunks whose head chunk is `do`
/// (`n't` depends on it): `mal` follows its word to `do` (the unique head
/// chunk, `3|1`), and the span root, anchored at `geh` as numbered before
/// the splice (chunk 3), still depends on `geh`, now chunk 4 (`1|4`).
#[test]
fn the_span_root_and_a_dependent_follow_their_words_through_a_growing_splice() {
    let parsed = tiers(GROWING_SPLICE);
    let (mut mor, mut gra) = parsed[0].clone();
    let root = anchor_of(&gra, 1);
    mor.splice_range_coordinated(
        &mut gra,
        0..1,
        block(&parsed[1], 1),
        root,
        HostRedirects::ByItem,
    )
    .expect("span");
    assert_eq!(
        written(&gra),
        ["1|4|AUX", "2|1|NEG", "3|1|ADV", "4|0|ROOT", "5|4|PUNCT"]
    );
}

/// A span root named inside the replaced range, or past the host, has no
/// word to depend on after the splice, and one at the utterance's root
/// beside the host's root would make two: refused, with both tiers
/// unchanged.
#[test]
fn a_span_root_the_splice_cannot_place_is_refused_atomically() {
    let parsed = tiers(GROWING_SPLICE);
    let attempt = |root: SpanRoot| {
        refused_atomically(&parsed[0], &parsed[1], 0..1, root, HostRedirects::ByItem)
    };
    assert_eq!(
        attempt(under(1)),
        Err(CoordinatedMutationError::SpanRootInReplacedRange {
            chunk: chunk(1),
            first: chunk(1),
            last: chunk(1),
        })
    );
    assert_eq!(
        attempt(under(9)),
        Err(CoordinatedMutationError::SpanRootOutOfHost {
            chunk: chunk(9),
            host_chunks: 4,
        })
    );
    // `geh` (chunk 3) is the host's root, outside the range: a span root at
    // the utterance's root would be a second one.
    assert_eq!(
        attempt(SpanRoot::UtteranceRoot),
        Err(CoordinatedMutationError::UtteranceRootTaken {
            host_root: chunk(3)
        })
    );
}

/// `x@s y@s z .` with `z -> x` (`3|1|DEP`): a span root anchored at `z`
/// would make the span depend on a word that depends on the span, a cycle
/// with no root (`x -> z -> x`). Refused, with both tiers unchanged.
#[test]
fn a_span_root_anchored_under_the_span_itself_is_refused_atomically() {
    let parsed = tiers(
        "*CHI:\tx@s y@s z .\n\
         %mor:\tx|x x|y n|z .\n\
         %gra:\t1|0|ROOT 2|3|DEP 3|1|DEP 4|1|PUNCT\n\
         *CHI:\tx y .\n\
         %mor:\tn|x n|y .\n\
         %gra:\t1|0|ROOT 2|1|DEP 3|1|PUNCT\n",
    );
    let (host_mor, host_gra) = parsed[0].clone();
    let (mut mor, mut gra) = (host_mor.clone(), host_gra.clone());
    let refused = mor.splice_range_coordinated(
        &mut gra,
        0..2,
        block(&parsed[1], 2),
        under(3),
        HostRedirects::ByItem,
    );
    assert_eq!(
        refused,
        Err(CoordinatedMutationError::SpanRootDependsOnSpan {
            chunk: chunk(3),
            through: chunk(1),
        }),
        "written {:?}",
        written(&gra)
    );
    assert_eq!(mor, host_mor, "a refusal changed %mor");
    assert_eq!(gra, host_gra, "a refusal changed %gra");
}

/// A host whose `%gra` does not number each relation by its chunk is
/// refused, with both tiers unchanged. The splice reads every head as a
/// chunk number, so on such a host it cannot say which word a head names;
/// it used to splice anyway and shift the misnumbered relation's index by
/// arithmetic that wraps below zero on a shrinking splice.
#[test]
fn a_host_numbered_out_of_chunk_order_is_refused_atomically() {
    let parsed = tiers(
        "*CHI:\tdont@s:eng mal geh .\n\
         %mor:\tx|dont adv|mal v|gehen .\n\
         %gra:\t1|3|AUX 3|1|ADV 2|0|ROOT 4|3|PUNCT\n\
         *CHI:\tdont .\n\
         %mor:\taux|do~neg|not .\n\
         %gra:\t1|0|AUX 2|1|NEG 3|1|PUNCT\n",
    );
    assert_eq!(
        refused_atomically(
            &parsed[0],
            &parsed[1],
            0..1,
            under(4),
            HostRedirects::ByItem
        ),
        Err(CoordinatedMutationError::HostIndexOutOfOrder {
            chunk: chunk(2),
            index: 3,
        })
    );
}
