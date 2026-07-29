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

//! AppleDouble sidecar files (`._name.cha`, created by macOS on non-HFS volumes
//! such as USB drives and network shares) must never be discovered as CHAT
//! transcripts by directory-walking commands. The skip is by NAME (`._` prefix),
//! independent of the file's content, so these tests give the sidecar valid CHAT
//! content: a content-based skip would let it through, only a name-based skip
//! catches it.

use std::fs;
use talkbank_parser_tests::test_error::TestError;
use tempfile::tempdir;

const VALID_CHAT: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thello world .
@End
";

/// `chatter to-json <dir>` must skip `._*.cha` AppleDouble sidecars: the real
/// file converts, the sidecar produces no output even though its content parses.
#[test]
fn to_json_skips_appledouble_sidecar() -> Result<(), TestError> {
    let dir = tempdir()?;
    let out = tempdir()?;
    fs::write(dir.path().join("good.cha"), VALID_CHAT)?;
    fs::write(dir.path().join("._good.cha"), VALID_CHAT)?;

    assert_cmd::cargo::cargo_bin_cmd!("chatter")
        .arg("to-json")
        .arg(dir.path())
        .arg("--output-dir")
        .arg(out.path())
        .assert()
        .success();

    assert!(
        out.path().join("good.json").exists(),
        "the real transcript should convert"
    );
    assert!(
        !out.path().join("._good.json").exists(),
        "the AppleDouble sidecar must be skipped, not converted"
    );
    Ok(())
}
