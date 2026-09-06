//! Inventory curated CHECK-to-Chatter mappings without claiming runtime parity.
//!
//! The compiled ErrorCode registry is generated from the spec code registry.
//! Runtime parity evidence belongs to the CHECK fixture harness, not this join.

use crate::check_error_map::check_error_number;
use crate::test_error::TestError;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use talkbank_model::errors::ErrorCode;

#[derive(serde::Deserialize)]
struct CheckCodeJson {
    code: u16,
    messages: Vec<String>,
    n_call_sites: u32,
}

#[derive(serde::Deserialize)]
struct CheckReferenceJson {
    codes: Vec<CheckCodeJson>,
}

struct CheckRule {
    id: u16,
    message: String,
}

/// A mapping always contains a first known code. Neither state carries runtime
/// evidence, so neither can be rendered as verified semantic/behavioral parity.
enum MappingEvidence {
    Unmapped,
    Curated {
        first: ErrorCode,
        rest: Vec<ErrorCode>,
    },
}

impl MappingEvidence {
    fn codes(&self) -> impl Iterator<Item = &ErrorCode> {
        let (first, rest) = match self {
            Self::Unmapped => (None, &[][..]),
            Self::Curated { first, rest } => (Some(first), rest.as_slice()),
        };
        first.into_iter().chain(rest.iter())
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::Unmapped => "unmapped",
            Self::Curated { .. } => "curated mapping",
        }
    }
}

struct MappingResult {
    check: CheckRule,
    evidence: MappingEvidence,
}

/// Render mapping evidence from a committed CHECK reference.
///
/// # Errors
/// Returns an error for unreadable/malformed references or duplicate codes.
pub fn report(reference: &Path) -> Result<String, TestError> {
    report_json(&fs::read_to_string(reference)?)
}

/// Render a CHECK reference JSON document after validating its code identities.
///
/// # Errors
/// Returns malformed-reference, duplicate-code or rendering errors.
pub fn report_json(reference: &str) -> Result<String, TestError> {
    let mappings = parse_check_rules(reference)?
        .into_iter()
        .map(map_rule)
        .collect::<Vec<_>>();
    render_report(&mappings)
}

/// Regenerate the mapping inventory in this checkout.
///
/// # Errors
/// Returns reference, repository-location, or output-write errors.
pub fn regenerate() -> Result<(), TestError> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| TestError::Failure("Cannot resolve repository root".to_owned()))?;
    let output = root.join("docs/audits/check-parity-audit.md");
    fs::write(
        &output,
        report(&manifest.join("clan-check-reference/check-error-codes.json"))?,
    )?;
    println!("Wrote CHECK mapping inventory to {}", output.display());
    Ok(())
}

fn parse_check_rules(content: &str) -> Result<Vec<CheckRule>, TestError> {
    let reference: CheckReferenceJson = serde_json::from_str(content)?;
    let mut ids = BTreeSet::new();
    let mut rules = Vec::new();
    for row in reference.codes {
        if !ids.insert(row.code) {
            return Err(TestError::Failure(format!(
                "duplicate CHECK code {}",
                row.code
            )));
        }
        if row.n_call_sites == 0 {
            continue;
        }
        if row.messages.is_empty() {
            return Err(TestError::Failure(format!(
                "emitted CHECK code {} has no message",
                row.code
            )));
        }
        rules.push(CheckRule {
            id: row.code,
            message: row.messages.join(" / "),
        });
    }
    rules.sort_by_key(|rule| rule.id);
    Ok(rules)
}

fn map_rule(rule: CheckRule) -> MappingResult {
    let supplemental = map_by_id(rule.id);
    let mut codes: Vec<ErrorCode> = ErrorCode::iter()
        .copied()
        .filter(|code| check_error_number(code) == rule.id || supplemental.contains(code))
        .collect();
    codes.sort_by_key(ErrorCode::as_str);
    let mut codes = codes.into_iter();
    let evidence = match codes.next() {
        Some(first) => MappingEvidence::Curated {
            first,
            rest: codes.collect(),
        },
        None => MappingEvidence::Unmapped,
    };
    MappingResult {
        check: rule,
        evidence,
    }
}

fn map_by_id(id: u16) -> &'static [ErrorCode] {
    match id {
        6 => &[ErrorCode::DuplicateHeader],
        7 => &[ErrorCode::MissingEndHeader],
        18 => &[ErrorCode::SpeakerNotDefined, ErrorCode::UndeclaredSpeaker],
        21 => &[ErrorCode::MissingSpeaker],
        // 22 = unmatched `[`. chatter recognizes `[` as a content-annotation
        // opener and reports the specific ContentAnnotationParseError.
        22 => &[ErrorCode::ContentAnnotationParseError],
        23 => &[ErrorCode::UnmatchedContentAnnotationEnd],
        24 => &[ErrorCode::UnbalancedOverlap],
        25 => &[ErrorCode::MissingOverlapEnd],
        31 => &[ErrorCode::MissingTerminator],
        36 => &[ErrorCode::MissingTerminator],
        38 | 47 => &[ErrorCode::IllegalDigits],
        40 | 140 => &[
            ErrorCode::DuplicateDependentTier,
            ErrorCode::MorCountMismatchTooFew,
            ErrorCode::MorCountMismatchTooMany,
            ErrorCode::MorGraCountMismatch,
        ],
        41 | 155 => &[ErrorCode::InvalidWordFormat],
        50 => &[ErrorCode::MissingTerminator],
        51 => &[ErrorCode::UnbalancedOverlap, ErrorCode::MissingOverlapEnd],
        52 => &[ErrorCode::StructuralOrderError],
        55 | 56 => &[ErrorCode::UnbalancedShortening],
        57 => &[ErrorCode::IllegalCharactersInWord],
        60 => &[ErrorCode::SpeakerNotDefined],
        69 => &[ErrorCode::EmptyLanguagesHeader],
        70 => &[ErrorCode::EmptyWordContent],
        81 => &[ErrorCode::InvalidMediaBullet],
        82 => &[ErrorCode::InvalidTimestamp],
        83 => &[
            ErrorCode::TierBeginTimeNotMonotonic,
            ErrorCode::TimestampBackwards,
        ],
        84 => &[ErrorCode::SpeakerSelfOverlap],
        // CHECK 85 previously mapped to E700, retired because it had no emit site.
        // Leave it unmapped until an active diagnostic is adjudicated.
        89 | 90 => &[ErrorCode::InvalidMediaBullet, ErrorCode::InvalidTimestamp],
        91 => &[ErrorCode::SyntaxError],
        // W210/W211 retired 2026-07-16; the spacing-family analogs now
        // live at E243 (illegal chars), E750 (space inside angle
        // group), E751 (glued pause), and E757 (code glued to word).
        92 | 93 | 160 | 161 => &[
            ErrorCode::IllegalCharactersInWord,
            ErrorCode::SpaceInsideAngleGroup,
            ErrorCode::PauseGluedToWord,
            ErrorCode::CodeGluedToFollowingContent,
        ],
        94 => &[
            ErrorCode::MorCountMismatchTooFew,
            ErrorCode::MorCountMismatchTooMany,
            ErrorCode::PhoCountMismatchTooFew,
            ErrorCode::PhoCountMismatchTooMany,
            ErrorCode::SinCountMismatchTooFew,
            ErrorCode::SinCountMismatchTooMany,
            ErrorCode::MorGraCountMismatch,
        ],
        // 107 = "Only single commas are allowed" = consecutive commas.
        107 => &[ErrorCode::ConsecutiveCommas],
        110 => &[ErrorCode::InvalidMediaBullet],
        117 => &[
            ErrorCode::UnbalancedCADelimiter,
            ErrorCode::UnmatchedUnderlineBegin,
            ErrorCode::UnmatchedUnderlineEnd,
        ],
        118 => &[ErrorCode::InvalidMediaBullet],
        // CLAN 119 "Missing word after code" is the dangling-retrace case
        // (`word [/] .`), the same retrace family as 52/151/159.
        119 => &[ErrorCode::StructuralOrderError],
        120 => &[ErrorCode::TertiaryLanguageNeedsExplicitCode],
        121 => &[ErrorCode::InvalidLanguageCode],
        122 => &[ErrorCode::InvalidLanguageCode],
        // 126 (E548) and 127 (E547) are supplied by check_error_map's
        // check_error_number (the single source of truth) via the union in
        // map_rule; not duplicated here.
        // 128/130 = unmatched ‹ / 〔 (non-standard CHAT brackets). chatter does
        // not model these as annotation openers; it rejects them as unparsable
        // content (E316), which still satisfies the "at least as strict" policy.
        // Their closing counterparts 129/131 map to E346.
        128 => &[ErrorCode::UnparsableContent],
        129 => &[ErrorCode::UnmatchedContentAnnotationEnd],
        130 => &[ErrorCode::UnparsableContent],
        131 => &[ErrorCode::UnmatchedContentAnnotationEnd],
        136 | 137 => &[ErrorCode::UnbalancedQuotation],
        // 138/139 = curly single quotes U+2019/U+2018 used as a word character.
        // chatter rejects them as a recognized illegal-character node and emits
        // E256 (CHAT requires the ASCII apostrophe).
        138 | 139 => &[ErrorCode::IllegalCurlyQuote],
        141 => &[
            ErrorCode::ReplacementOnFragment,
            ErrorCode::ReplacementOnNonword,
            ErrorCode::ReplacementOnFiller,
        ],
        142 => &[ErrorCode::InvalidParticipantRole],
        143 => &[ErrorCode::InvalidIDFormat],
        151 => &[ErrorCode::StructuralOrderError],
        153 => &[ErrorCode::InvalidAgeFormat],
        156 => &[ErrorCode::IllegalCharactersInWord],
        // 158 = `[: ...]` replacement must be a real word; chatter flags the
        // `xxx` (untranscribed) case via ReplacementContainsUntranscribed.
        158 => &[ErrorCode::ReplacementContainsUntranscribed],
        159 => &[ErrorCode::StructuralOrderError],
        _ => &[],
    }
}

fn render_report(mappings: &[MappingResult]) -> Result<String, TestError> {
    let mapped = mappings
        .iter()
        .filter(|row| matches!(row.evidence, MappingEvidence::Curated { .. }))
        .count();
    let covered: BTreeSet<&str> = mappings
        .iter()
        .flat_map(|row| row.evidence.codes())
        .map(ErrorCode::as_str)
        .collect();
    let mut out = String::from("# CHECK Mapping Inventory\n\n");
    out.push_str("Generated by `cargo run -p talkbank-parser-tests --bin audit_check_parity`.\n\n");
    out.push_str("This is a curated mapping inventory, not a runtime parity report. A mapping does not prove equal behavior, semantic completeness, relative strictness, or an intentional divergence. An absent mapping does not prove a missing validator.\n\n");
    writeln!(
        out,
        "- Emitted CHECK codes in the committed reference: {}",
        mappings.len()
    )?;
    writeln!(out, "- CHECK codes with curated mappings: {mapped}")?;
    writeln!(
        out,
        "- CHECK codes without curated mappings: {}",
        mappings.len() - mapped
    )?;
    writeln!(
        out,
        "- Compiled Chatter error codes (from the spec registry): {}\n",
        ErrorCode::all().len()
    )?;
    out.push_str("## Evidence and reproduction\n\n");
    out.push_str("The CHECK reference is `crates/talkbank-parser-tests/clan-check-reference/check-error-codes.json`, extracted from CLAN source. Chatter codes come from `ErrorCode::iter()`, whose enum is generated from `spec/codes/error-codes.toml`. No Rust-source regex or message-keyword fallback is used.\n\n");
    out.push_str("Runtime expectations and documented divergences live in `crates/talkbank-parser-tests/tests/check_parity/manifest.json`. Run the CHECK fixture harness to obtain behavioral evidence; this generator does not execute CHECK or Chatter validation and reports no verified-parity count. See the book's CHECK Parity Audit chapter and spec workflow.\n\n");
    out.push_str("## Curated mapping\n\n| CHECK | Message | Chatter codes | Evidence |\n|---:|---|---|---|\n");
    for row in mappings {
        let codes = row
            .evidence
            .codes()
            .map(|code| format!("`{code}`"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            out,
            "| {} | {} | {} | {} |",
            row.check.id,
            row.check.message.replace('|', "\\|"),
            codes,
            row.evidence.label()
        )?;
    }
    out.push_str("\n## Chatter codes without a curated CHECK mapping\n\n");
    out.push_str("These are unmapped codes, not automatically enhancements. Status is read from the compiled spec registry.\n\n| Code | Variant | Enforcement |\n|---|---|---|\n");
    for code in ErrorCode::iter().filter(|code| !covered.contains(code.as_str())) {
        writeln!(out, "| `{code}` | `{code:?}` | {:?} |", code.check_status())?;
    }
    Ok(out)
}
