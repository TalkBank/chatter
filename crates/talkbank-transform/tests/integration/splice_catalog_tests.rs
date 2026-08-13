//! Tests for `talkbank_transform::splice::catalog`.
//!
//! Every fixture here is either verbatim from a committed
//! `crates/talkbank-parser-tests/tests/error_corpus/validation_errors/*.cha`
//! fixture, or a minimal inline literal in the style already used by
//! `crates/talkbank-transform/src/splice/admit.rs`'s own tests (never a new
//! ad hoc `.cha` file on disk, per the root `CLAUDE.md` danger rule 9).
//! Each test asserts the actual spliced BYTES via [`apply_edits`], not just
//! that a fix exists, since the whole point of this catalog is that a
//! plausible-looking edit can still corrupt content (see the module docs on
//! `catalog.rs` for two cases exactly like that in the LSP source this
//! catalog was ported from).

use talkbank_model::ErrorCollector;
use talkbank_model::errors::ParseError;
use talkbank_parser::TreeSitterParser;

use talkbank_model::model::TranscriptName;
use talkbank_transform::splice::{BatchSafety, EditTarget, FixKind, apply_edits, catalog_fix};

/// Parse `source`, run the same alignment-aware validation pass
/// `chatter validate` runs (parsing alone only catches PARSER-layer codes;
/// most of the codes this catalog covers are VALIDATION-layer and never
/// fire from `parse_chat_file_streaming` on its own, confirmed empirically
/// against every fixture below), and return the first diagnostic whose code
/// is `code`, or an `Err` naming every code that actually fired.
///
/// Per the root `CLAUDE.md` danger rule 7 ("test failures are bugs until
/// proven otherwise"): when a fixture does not produce the expected code,
/// the fixture (or, here, the helper) is wrong, never the assertion. This
/// helper's first draft ran parsing only, per this crate's own precedent in
/// `parse_chat_file_streaming`'s doc comment; every non-parser-layer test
/// below failed against a real run with zero diagnostics, which is what
/// motivated adding the `validate_with_alignment` pass (the same two-step
/// pipeline `crates/talkbank-parser-tests/tests/integration/
/// validation_error_corpus.rs` documents as its own test strategy).
fn single_error_with_code(source: &str, code: &str) -> Result<ParseError, String> {
    let parser = TreeSitterParser::new().map_err(|error| format!("parser init failed: {error}"))?;
    let parse_errors = ErrorCollector::new();
    let mut file = parser.parse_chat_file_streaming(source, &parse_errors);

    let validation_errors = ErrorCollector::new();
    file.validate_with_alignment(&validation_errors, TranscriptName::Anonymous);

    let mut diagnostics = parse_errors.into_vec();
    diagnostics.extend(validation_errors.into_vec());

    diagnostics
        .iter()
        .find(|error| error.code.as_str() == code)
        .cloned()
        .ok_or_else(|| {
            let seen: Vec<&str> = diagnostics
                .iter()
                .map(|error| error.code.as_str())
                .collect();
            format!("expected {code} but parse+validate produced {seen:?}")
        })
}

/// E241 is the mechanical case the whole first consumer rests on.
#[test]
fn e241_yields_a_mechanical_xxx_replacement() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                  @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tI said xx today .\n@End";
    let error = single_error_with_code(source, "E241")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E241")?;
    assert_eq!(fix.safety, BatchSafety::Mechanical);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            assert_eq!(edits.len(), 1);
            match edits[0].target() {
                EditTarget::Replace(span) => assert_eq!(&source[span.to_range()], "xx"),
                EditTarget::InsertAt(_) => return Err("E241 must replace, not insert".to_string()),
            }
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(out.contains("I said xxx today ."), "got {out:?}");
        }
        FixKind::Alternatives(_) => return Err("E241 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// A missing terminator has three valid answers, so it must never be
/// batch-applied to a corpus. This is E305 (`MissingTerminator`), not E301:
/// the seed source's `"E301" | "E305"` match arm was verified against a
/// real parse of this exact source to be wrong for E301 (see `catalog.rs`
/// module docs); E301 actually means "empty speaker code" and never fires
/// here at all.
#[test]
fn missing_terminator_is_ambiguous_and_offers_every_alternative() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n*CHI:\thi\n@End\n";
    let error = single_error_with_code(source, "E305")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E305")?;
    assert_eq!(fix.safety, BatchSafety::Ambiguous);
    match fix.kind {
        FixKind::Alternatives(alts) => {
            assert_eq!(alts.len(), 3);
            let mut texts: Vec<String> = Vec::new();
            for alt in alts {
                texts.push(apply_edits(source, &alt.edits).map_err(|error| error.to_string())?);
            }
            assert!(texts.iter().any(|t| t.contains("hi .")), "got {texts:?}");
            assert!(texts.iter().any(|t| t.contains("hi ?")), "got {texts:?}");
            assert!(texts.iter().any(|t| t.contains("hi !")), "got {texts:?}");
        }
        FixKind::Deterministic(_) => return Err("E305 must not be deterministic".to_string()),
    }
    Ok(())
}

/// Guards the classification itself: a future entry cannot quietly become
/// batch-applicable by being added to the wrong tier.
#[test]
fn comma_deletion_is_semantic_not_mechanical() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                  @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\twww, the rest .\n@End";
    let error = single_error_with_code(source, "E259")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E259")?;
    assert_eq!(fix.safety, BatchSafety::Semantic);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(out.contains("www the rest ."), "got {out:?}");
        }
        FixKind::Alternatives(_) => return Err("E259 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// A comma directly against the tab that opens the tier is the tier's
/// first character; deleting only the comma byte would leave the space
/// after it as a new leading space on the tier (E758
/// `LeadingSpaceOnMainTier`), trading one invalidity for another.
/// `chatter fix`'s post-splice re-parse check caught exactly this
/// (2026-07-31); this pins the widened deletion directly against the
/// catalog function so a regression fails here, not only at the CLI
/// boundary.
#[test]
fn e259_tier_initial_comma_deletion_does_not_leave_a_leading_space() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|test|CHI|||||Child|||\n*CHI:\t, xx .\n@End\n";
    let error = single_error_with_code(source, "E259")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E259")?;
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(
                out.contains("*CHI:\txx .\n"),
                "leading space survived: {out:?}"
            );
            assert!(
                !out.contains("*CHI:\t xx"),
                "leading space survived: {out:?}"
            );
            assert!(
                single_error_with_code(&out, "E758").is_err(),
                "the fix introduced a NEW E758 leading-space diagnostic: {out:?}"
            );
        }
        FixKind::Alternatives(_) => return Err("E259 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// The bug this catalog exists to not repeat: the LSP's seed source
/// replaces the WHOLE diagnostic span (the entire word `"ˈˈhello"`) with a
/// single `"ˈ"`, which would delete `"hello"`. This asserts the word
/// content survives.
#[test]
fn e244_collapses_consecutive_stress_marks_without_deleting_the_word() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                  @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t\u{2C8}\u{2C8}hello .\n@End";
    let error = single_error_with_code(source, "E244")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E244")?;
    assert_eq!(fix.safety, BatchSafety::Mechanical);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(
                out.contains("\u{2C8}hello ."),
                "stress mark not collapsed: {out:?}"
            );
            assert!(
                !out.contains("\u{2C8}\u{2C8}"),
                "duplicate stress mark survived: {out:?}"
            );
        }
        FixKind::Alternatives(_) => return Err("E244 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// The second bug this catalog exists to not repeat: the diagnostic span
/// for E258 covers exactly ONE comma, so the seed's "replace with ','"
/// action is a no-op. This asserts the pair actually collapses to one.
#[test]
fn e258_collapses_the_comma_pair_not_a_noop_replace() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                  @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello ,, world .\n@End";
    let error = single_error_with_code(source, "E258")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E258")?;
    assert_eq!(fix.safety, BatchSafety::Mechanical);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(out.contains("hello , world ."), "got {out:?}");
            assert!(!out.contains(",,"), "comma pair survived: {out:?}");
        }
        FixKind::Alternatives(_) => return Err("E258 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E306: deletes the whole empty-utterance line, not just the terminator
/// the diagnostic span narrowly covers.
#[test]
fn e306_deletes_the_whole_empty_utterance_line() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n*CHI:\t.\n@End";
    let error = single_error_with_code(source, "E306")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E306")?;
    assert_eq!(fix.safety, BatchSafety::Semantic);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(
                !out.contains("*CHI:\t.\n"),
                "empty utterance line survived: {out:?}"
            );
            assert!(
                out.contains("@ID:\teng|corpus|CHI|||||Child|||\n@End"),
                "@End must now directly follow @ID once the utterance line is gone: {out:?}"
            );
        }
        FixKind::Alternatives(_) => return Err("E306 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E308: appends the undeclared speaker, read directly from the
/// diagnostic's own span, to the `@Participants` line.
#[test]
fn e308_appends_the_undeclared_speaker_to_participants() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n*MOT:\thello world .\n@End";
    let error = single_error_with_code(source, "E308")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E308")?;
    assert_eq!(fix.safety, BatchSafety::Semantic);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(
                out.contains("@Participants:\tCHI Child, MOT Participant\n"),
                "got {out:?}"
            );
        }
        FixKind::Alternatives(_) => return Err("E308 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E501: real `DuplicateHeader` means two `@Begin` lines, the opposite of
/// what the seed source's action addressed (it inserted a missing
/// `@Begin`). The correct fix deletes the flagged (second) occurrence.
#[test]
fn e501_deletes_the_duplicate_begin_line() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n@End\n";
    let error = single_error_with_code(source, "E501")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E501")?;
    assert_eq!(fix.safety, BatchSafety::Mechanical);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert_eq!(
                out,
                "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n@End\n"
            );
        }
        FixKind::Alternatives(_) => return Err("E501 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E502: appends `@End` at end of file, inserting a leading newline first
/// since this fixture's last line has none of its own.
#[test]
fn e502_appends_end_header_at_eof() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .";
    let error = single_error_with_code(source, "E502")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E502")?;
    assert_eq!(fix.safety, BatchSafety::Mechanical);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(out.ends_with("hello world .\n@End\n"), "got {out:?}");
        }
        FixKind::Alternatives(_) => return Err("E502 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E503: prepends `@UTF8` as the very first line. This is the concrete
/// case `EditTarget::InsertAt(0)` exists for.
#[test]
fn e503_prepends_utf8_header() -> Result<(), String> {
    let source = "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n@End";
    let error = single_error_with_code(source, "E503")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E503")?;
    assert_eq!(fix.safety, BatchSafety::Mechanical);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(out.starts_with("@UTF8\n@Begin\n"), "got {out:?}");
        }
        FixKind::Alternatives(_) => return Err("E503 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E504: only the `@Participants`-message case gets a fix (matching the
/// seed source's own scoping); it inserts right after `@Begin`, not after
/// `@UTF8` like the seed source's `insert_after_utf8` call actually did
/// despite its action title claiming otherwise (see `catalog.rs` docs).
#[test]
fn e504_inserts_participant_placeholder_after_begin() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@End";
    let error = single_error_with_code(source, "E504")?;
    if !error.message.contains("@Participants") {
        return Err(format!(
            "fixture assumption failed: expected the @Participants E504, got {:?}",
            error.message
        ));
    }
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E504")?;
    assert_eq!(fix.safety, BatchSafety::Semantic);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert_eq!(
                out,
                "@UTF8\n@Begin\n@Participants:\tCHI Child\n@Languages:\teng\n@End"
            );
        }
        FixKind::Alternatives(_) => return Err("E504 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E507: fills the empty `@Languages:` header with a guessed default
/// (`eng`), replacing only the header key span, never the newline after it.
#[test]
fn e507_fills_in_default_language() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n@End";
    let error = single_error_with_code(source, "E507")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E507")?;
    assert_eq!(fix.safety, BatchSafety::Semantic);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(
                out.starts_with("@UTF8\n@Begin\n@Languages:\teng\n@Participants:"),
                "got {out:?}"
            );
        }
        FixKind::Alternatives(_) => return Err("E507 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E604: the diagnostic anchors to the main tier the orphaned `%gra`
/// belongs to, not to the `%gra` line itself; the fix must delete the
/// FOLLOWING line, never the main tier the diagnostic's span sits on.
#[test]
fn e604_deletes_the_orphaned_gra_tier_not_the_main_tier() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                  @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello world .\n\
                  %gra:\t1|2|NSUBJ 2|0|ROOT\n@End";
    let error = single_error_with_code(source, "E604")?;
    let fix = catalog_fix(&error, source).ok_or("no catalog entry for E604")?;
    assert_eq!(fix.safety, BatchSafety::Semantic);
    match fix.kind {
        FixKind::Deterministic(edits) => {
            let out = apply_edits(source, &edits).map_err(|error| error.to_string())?;
            assert!(
                out.contains("*CHI:\thello world .\n"),
                "main tier was deleted: {out:?}"
            );
            assert!(
                !out.contains("%gra"),
                "orphaned %gra tier survived: {out:?}"
            );
        }
        FixKind::Alternatives(_) => return Err("E604 must not be ambiguous".to_string()),
    }
    Ok(())
}

/// E301 is verified to mean "empty speaker code" today (`*:\thello world
/// .`), a genuinely different diagnostic from the missing-terminator fix
/// the seed source aliased it to. No catalog entry is the correct answer
/// here, not a ported (wrong) fix.
#[test]
fn e301_has_no_catalog_entry_despite_the_seed_aliasing_it_to_missing_terminator()
-> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
                  @ID:\teng|corpus|CHI|||||Child|||\n*:\thello world .\n@End";
    let error = single_error_with_code(source, "E301")?;
    assert!(
        catalog_fix(&error, source).is_none(),
        "E301 must not get the terminator fix"
    );
    Ok(())
}

/// E242's real diagnostic ("Unbalanced quotation") has no relation to the
/// seed source's `"+..."` trailing-off insertion, and no single answer is
/// derivable from the diagnostic alone (a missing open and a missing close
/// both look the same). No catalog entry.
#[test]
fn e242_has_no_catalog_entry_because_the_seed_fix_is_unrelated() -> Result<(), String> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
                  @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t\"hello .\n@End";
    let error = single_error_with_code(source, "E242")?;
    assert!(
        catalog_fix(&error, source).is_none(),
        "E242 has no verified single-answer fix"
    );
    Ok(())
}
