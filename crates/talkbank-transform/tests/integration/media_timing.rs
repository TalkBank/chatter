//! Media/timing reconciliation at the transform boundary.
//!
//! A successful timing transform must not serialize a document that still
//! claims its media is `unlinked`. The fixture is the committed CLAN CHECK 124
//! parity case, so this test exercises the same parsed structure that E552
//! rejects rather than assembling a parallel model by hand.

use std::path::PathBuf;

use talkbank_model::ParseValidateOptions;
use talkbank_transform::media_timing::{MediaTimingState, reconcile_media_timing};
use talkbank_transform::{parse::TreeSitterParser, parse_and_validate};

fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir
}

#[test]
fn timed_unlinked_document_becomes_linked_before_serialization() {
    let fixture = workspace_root().join(
        "crates/talkbank-parser-tests/tests/check_parity/fixtures/\
         CHECK_124_media_unlinked_with_bullet.cha",
    );
    let source = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read {}: {error}", fixture.display()));
    let parser = TreeSitterParser::new().expect("tree-sitter parser");
    let chat = parser.parse_chat_file(&source).expect_built();

    let reconciled = reconcile_media_timing(chat).expect("timed file has one usable @Media");
    assert!(matches!(reconciled, MediaTimingState::Timed(_)));
    let output = reconciled.to_chat_string();
    assert!(
        output.contains("@Media:\tCHECK_124_media_unlinked_with_bullet, audio\n"),
        "the linked output must retain the declaration and remove only `unlinked`: {output}"
    );

    let validation = parse_and_validate(&output, ParseValidateOptions::default().with_validation());
    assert!(
        validation.is_ok(),
        "the committed parity fixture has only the E552 contradiction, so the reconciled output must validate: {validation:?}"
    );
}
