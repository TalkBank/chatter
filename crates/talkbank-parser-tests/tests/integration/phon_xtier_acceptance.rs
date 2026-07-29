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

//! Acceptance tests for wild-corpus Phon `%x`-tier conventions.
//!
//! The Phon `%x`-tier content rules (E735-E746) were specified against
//! Greg Hedlund's format note, which never confronted two conventions Phon
//! actually exports at scale (thousands of instances across the phon data
//! repos):
//!
//! 1. **Pause fillers.** When a record's main tier carries a pause, Phon
//!    mirrors the pause token (`(.)`, `(..)`, `(...)`) at the same word
//!    position on EVERY word-aligned phonology tier (`%mod`, `%pho`,
//!    `%xmodsyl`, `%xphosyl`, and as a `(..)↔(..)` pair on `%xphoaln`) so
//!    the word indices stay in lockstep. A pause filler is not a
//!    syllabified word; demanding phone:CODE structure of it is wrong.
//! 2. **Syllable-boundary notation.** `%mod`/`%pho` words may carry the
//!    `^` syllable-boundary marker (and IPA `.` syllable breaks), e.g.
//!    `ˈbɔ^hɔɪ`. The `%xphoaln` tier aligns bare segments, so its
//!    reconstruction comparison must be insensitive to boundary markers,
//!    exactly as it already is to the stress markers `ˈ`/`ˌ`.
//!
//! These fixtures are minimized from real corpus records (Goad/Sonya and
//! Cornwell-style shape records in `phon-eng-french-data`). Genuine
//! misalignments (index-shift chains) must STILL be reported; the last
//! test pins that the relaxation does not swallow them.

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;

/// Parse + validate a CHAT document and return every diagnostic code.
fn diagnostic_codes(content: &str) -> Vec<String> {
    let parser = TreeSitterParser::new().expect("parser init");
    let parse_errors = ErrorCollector::new();
    let parse_result = parser.parse_chat_file_fragment(content, 0, &parse_errors);
    let mut codes: Vec<String> = parse_errors
        .to_vec()
        .iter()
        .map(|e| e.code.to_string())
        .collect();
    match parse_result {
        ParseOutcome::Parsed(mut chat_file) => {
            let validation_errors = ErrorCollector::new();
            chat_file.validate_with_alignment(&validation_errors, None);
            codes.extend(
                validation_errors
                    .to_vec()
                    .iter()
                    .map(|e| e.code.to_string()),
            );
        }
        other => panic!("fixture must parse, got {other:?}"),
    }
    codes
}

fn assert_no_codes(codes: &[String], banned: &[&str]) {
    for code in banned {
        assert!(
            !codes.iter().any(|c| c == code),
            "expected no {code}, got diagnostics: {codes:?}"
        );
    }
}

/// Pause fillers mirrored across %mod/%pho/%xmodsyl/%xphosyl/%xphoaln are
/// valid alignment units, not malformed syllabification words.
///
/// Minimized from data/phon-eng-french-data/Eng-NA/Goad/Sonya/10913.cha
/// ("a book (..) cup ."), which draws E735 x2 (one per syl tier) before
/// the fix.
#[test]
fn pause_fillers_are_valid_syl_and_phoaln_units() {
    let content = "@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|corpus|CHI|||||Target_Child|||\n\
*CHI:\ta book (..) cup .\n\
%mod:\tə bʊk (..) kʌp\n\
%pho:\tə bʊk̚kʰɪ̥ (..) kʰʌp̚\n\
%xmodsyl:\tə:N b:Oʊ:Nk:C (..) k:Oʌ:Np:C\n\
%xphosyl:\tə:N b:Oʊ:Nk̚:Ckʰ:Oɪ̥:N (..) kʰ:Oʌ:Np̚:C\n\
%xphoaln:\tə↔ə b↔b,ʊ↔ʊ,k↔k̚,∅↔kʰ,∅↔ɪ̥ (..)↔(..) k↔kʰ,ʌ↔ʌ,p↔p̚\n\
@End\n";
    let codes = diagnostic_codes(content);
    assert_no_codes(&codes, &["E735", "E737", "E738", "E740", "E741"]);
}

/// All three pause lengths are accepted as fillers on the syl tiers.
#[test]
fn all_pause_lengths_accepted_as_fillers() {
    let content = "@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|corpus|CHI|||||Target_Child|||\n\
*CHI:\tcat (.) dog (...) cup .\n\
%mod:\tkæt (.) dɔɡ (...) kʌp\n\
%xmodsyl:\tk:Oæ:Nt:C (.) d:Oɔ:Nɡ:C (...) k:Oʌ:Np:C\n\
@End\n";
    let codes = diagnostic_codes(content);
    assert_no_codes(&codes, &["E735", "E737"]);
}

/// `^` syllable-boundary markers (with stress) in the source %mod/%pho word
/// do not fail the segment-level %xphoaln reconstruction comparison.
///
/// Shape minimized from the wild 'ˈbɔ^hɔɪ' / 'ˈfɹaɪt̚^tɪndə' records.
#[test]
fn caret_syllable_boundaries_ignored_in_phoaln_reconstruction() {
    let content = "@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|corpus|CHI|||||Target_Child|||\n\
*CHI:\tboy .\n\
%mod:\tˈbɔ^hɔɪ\n\
%pho:\tbɔhɔɪ\n\
%xphoaln:\tb↔b,ɔ↔ɔ,h↔h,ɔɪ↔ɔɪ\n\
@End\n";
    let codes = diagnostic_codes(content);
    assert_no_codes(&codes, &["E740", "E741"]);
}

/// IPA `.` syllable breaks in the source %pho word are likewise ignored by
/// the segment-level reconstruction comparison ('ko.çɔ̃' shape).
#[test]
fn ipa_dot_syllable_breaks_ignored_in_phoaln_reconstruction() {
    let content = "@UTF8\n\
@Begin\n\
@Languages:\tfra\n\
@Participants:\tCHI Target_Child\n\
@ID:\tfra|corpus|CHI|||||Target_Child|||\n\
*CHI:\tcochon .\n\
%mod:\tkoʃɔ̃\n\
%pho:\tko.çɔ̃\n\
%xphoaln:\tk↔k,o↔o,ʃ↔ç,ɔ̃↔ɔ̃\n\
@End\n";
    let codes = diagnostic_codes(content);
    assert_no_codes(&codes, &["E740", "E741"]);
}

/// The relaxation must not swallow genuine mismatches: a pause filler on the
/// syl tier standing where the %mod word is a real word is still a
/// reconstruction error, and a truly different phone sequence still fails.
#[test]
fn genuine_mismatches_still_reported() {
    // %xmodsyl word 2 is a pause filler but %mod word 2 is 'dɔɡ'.
    let filler_vs_word = "@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|corpus|CHI|||||Target_Child|||\n\
*CHI:\tcat dog .\n\
%mod:\tkæt dɔɡ\n\
%xmodsyl:\tk:Oæ:Nt:C (..)\n\
@End\n";
    let codes = diagnostic_codes(filler_vs_word);
    assert!(
        codes.iter().any(|c| c == "E737"),
        "pause filler standing in for a real %mod word must still fail \
         reconstruction, got: {codes:?}"
    );

    // Segment content genuinely differs ('kæt' vs 'dɔɡ' shape).
    let wrong_phones = "@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|corpus|CHI|||||Target_Child|||\n\
*CHI:\tcat .\n\
%mod:\tdɔɡ\n\
%pho:\tdɔɡ\n\
%xphoaln:\tk↔k,æ↔æ,t↔t\n\
@End\n";
    let codes = diagnostic_codes(wrong_phones);
    assert!(
        codes.iter().any(|c| c == "E740" || c == "E741"),
        "genuinely different phones must still fail reconstruction, got: {codes:?}"
    );
}
