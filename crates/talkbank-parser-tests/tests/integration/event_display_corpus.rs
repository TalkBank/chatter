//! Standalone non-word presentation preserves CHAT and propagates sink refusal.

use super::{assert_display_refusals, assert_writer_refusals};
use std::{fmt::Display, path::Path};
use talkbank_model::{
    WriteChat,
    alignment::helpers::{ContentItem, walk_content},
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

fn check(value: &(impl WriteChat + Display), path: &Path) {
    let mut chat = String::new();
    value.write_chat(&mut chat).expect("accepting writer");
    assert_eq!(value.to_string(), chat, "{}", path.display());
    assert!(assert_writer_refusals(value, path) > 0);
    assert!(assert_display_refusals(value, path) > 0);
}

#[test]
fn reference_nonword_display_preserves_chat_and_every_writer_refusal() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnessed = [0; 13];
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference syntax");
        for utterance in file.utterances() {
            // The canonical traversal owns nesting. No domain filter removes
            // retraced or nonalignable content from the presentation contract.
            walk_content(&utterance.main.content.content, None, &mut |item| {
                let index = match item {
                    ContentItem::OtherSpokenEvent(value) => {
                        check(value, fixture.path());
                        0
                    }
                    ContentItem::LongFeatureBegin(value) => {
                        check(value, fixture.path());
                        1
                    }
                    ContentItem::LongFeatureEnd(value) => {
                        check(value, fixture.path());
                        2
                    }
                    ContentItem::NonvocalBegin(value) => {
                        check(value, fixture.path());
                        3
                    }
                    ContentItem::NonvocalEnd(value) => {
                        check(value, fixture.path());
                        4
                    }
                    ContentItem::NonvocalSimple(value) => {
                        check(value, fixture.path());
                        5
                    }
                    ContentItem::Separator(value) => {
                        check(value, fixture.path());
                        6
                    }
                    ContentItem::Event(value) => {
                        check(value, fixture.path());
                        7
                    }
                    ContentItem::Pause(value) => {
                        check(value, fixture.path());
                        8
                    }
                    ContentItem::Action(value) => {
                        check(value, fixture.path());
                        9
                    }
                    ContentItem::OverlapPoint(value) => {
                        check(value, fixture.path());
                        10
                    }
                    ContentItem::Freecode(value) => {
                        check(value, fixture.path());
                        11
                    }
                    ContentItem::InternalBullet(value) => {
                        check(value, fixture.path());
                        12
                    }
                    // Word/replacement display has separate canonical owners;
                    // underline markers have no standalone Display adapter.
                    ContentItem::Word(_)
                    | ContentItem::ReplacedWord(_)
                    | ContentItem::UnderlineBegin(_)
                    | ContentItem::UnderlineEnd(_) => return,
                };
                witnessed[index] += 1;
            });
        }
    }
    assert!(
        witnessed.iter().all(|count| *count > 0),
        "every adapter needs a reference witness: {witnessed:?}"
    );
}
