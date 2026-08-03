// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! `%gra` relation-label vocabulary validation, end to end through the CLI.
//!
//! A `%gra` relation label is a Universal Dependencies relation, optionally
//! followed by a language-specific subtype: `HEAD` or `HEAD-SUBTYPE`. UD
//! fixes the set of HEADs at 37 universal relations and deliberately leaves
//! SUBTYPES open and language-specific, so this is the only part of the
//! label that can be checked against a closed vocabulary.
//!
//! Why this matters: nothing validated relation labels before, in chatter or
//! in CLAN CHECK. A corrupted label rides silently into every downstream
//! analysis. The motivating case was a real one found in the wild corpora on
//! 2026-07-26, `13|3|PUNCTT` in a file that both validators passed.
//!
//! Grounding for the rule (survey of 13,270 corpus files, 12.5% of the
//! mirror, 2026-07-26): 39 distinct relation HEADS occur, of which exactly
//! two fall outside UD's universal 37, `IOB` (17 occurrences, a truncation
//! of `IOBJ`) and `PAD` (1). Every language-specific subtype in the corpora
//! passes untouched, because subtypes are not checked. So the rule flags
//! real defects with no legitimate-data fallout.

use predicates::prelude::*;
use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

/// A minimal, otherwise-valid file whose only defect is the relation HEAD
/// `PUNCTT` on the final token. Modelled on the real wild-corpus case.
const CHAT_WITH_UNKNOWN_RELATION_HEAD: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
*CHI:	the dog .
%mor:	det|the-Def-Art noun|dog .
%gra:	1|2|DET 2|0|ROOT 3|2|PUNCTT
@End
"#;

/// The same file with the head spelled correctly. Control: proves the
/// rejection above is attributable to the label and not to the scaffold.
const CHAT_WITH_VALID_RELATIONS: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
*CHI:	the dog .
%mor:	det|the-Def-Art noun|dog .
%gra:	1|2|DET 2|0|ROOT 3|2|PUNCT
@End
"#;

/// A legitimate language-specific subtype attested in the corpora
/// (`NMOD-POSS`, 138,746 occurrences in the survey sample). Guards against
/// the over-strict implementation that checks the whole label against a
/// closed list instead of just the head.
const CHAT_WITH_LANGUAGE_SPECIFIC_SUBTYPE: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
*CHI:	his dog .
%mor:	pron|his-Prs-Gen-S3 noun|dog .
%gra:	1|2|NMOD-POSS 2|0|ROOT 3|2|PUNCT
@End
"#;

#[test]
fn validate_rejects_a_gra_relation_head_outside_the_ud_universal_set() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("unknown_relation_head.cha");
    fs::write(&file_path, CHAT_WITH_UNKNOWN_RELATION_HEAD)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("Invalid: 1"));
    Ok(())
}

#[test]
fn validate_accepts_the_same_file_with_a_correctly_spelled_head() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("valid_relations.cha");
    fs::write(&file_path, CHAT_WITH_VALID_RELATIONS)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Invalid: 0"));
    Ok(())
}

#[test]
fn validate_accepts_a_language_specific_relation_subtype() -> Result<(), TestError> {
    let dir = tempdir()?;
    let file_path = dir.path().join("subtype.cha");
    fs::write(&file_path, CHAT_WITH_LANGUAGE_SPECIFIC_SUBTYPE)?;

    crate::common::chatter_cmd()
        .arg("validate")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Invalid: 0"));
    Ok(())
}
