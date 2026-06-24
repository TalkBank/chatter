//! `chatter adjudicate`, apply operator decisions to pending
//! adjudications, write resolved entries to the override file.
//!
//! Thin CLI shim around `talkbank_transform::adjudication`. The
//! scripted-TOML format and the `--interactive` terminal prompter
//! are the operator-facing seams; everything else is library code
//! with its own test coverage.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;

use tracing::{Level, info, span, warn};

use crate::exit_codes::{EXIT_INPUT_ERROR, EXIT_PRECONDITION};

use talkbank_transform::adjudication::{
    AdjudicationError, OperatorDecision, PendingAdjudications, PendingEntry, PendingKindData,
    Prompter, ScriptedPrompter, run_adjudication,
};
use talkbank_transform::speaker_id::{InsertedRoleSpec, OverrideFile, SpeakerAction};

/// Top-level entry for `chatter adjudicate`.
///
/// Exit-code contract (matches `adjudication-workflow.md`):
/// - 0: all pending entries decided; both files rewritten on disk.
/// - 1: I/O or TOML error on one of the three files.
/// - 2: operator-supplied decision rejected, or mode required and
///   neither `--scripted` nor `--interactive` supplied.
pub fn run_adjudicate(
    pending_path: &Path,
    override_path: &Path,
    scripted_path: Option<&Path>,
    interactive: bool,
    operator_override: Option<&str>,
) {
    let _span = span!(
        Level::INFO,
        "chatter_adjudicate",
        pending = %pending_path.display(),
    )
    .entered();

    let operator = operator_override.map(str::to_string).unwrap_or_else(|| {
        std::env::var("USER").unwrap_or_else(|_| {
            warn!(
                "$USER environment variable is unset and --operator not supplied; override-file \
                 entries will record operator as \"unknown\""
            );
            "unknown".to_string()
        })
    });

    let mut pending = match PendingAdjudications::read(pending_path) {
        Ok(p) => p,
        Err(e) => exit_with_error(pending_path, "pending-adjudications", e),
    };
    let mut prompter: Box<dyn Prompter> = match (interactive, scripted_path) {
        (true, None) => Box::new(TerminalPrompter),
        (false, Some(path)) => match ScriptedPrompter::read_toml(path) {
            Ok(p) => Box::new(p),
            Err(e) => exit_with_error(path, "scripted-decisions", e),
        },
        (true, Some(_)) => {
            eprintln!(
                "Error: --interactive and --scripted are mutually exclusive (clap should have caught this)"
            );
            std::process::exit(EXIT_PRECONDITION);
        }
        (false, None) => {
            eprintln!("Error: one of --scripted SPEC or --interactive must be supplied");
            std::process::exit(EXIT_PRECONDITION);
        }
    };
    let mut overrides = match OverrideFile::read_or_default(override_path) {
        Ok(o) => o,
        Err(e) => {
            warn!(
                "failed to read override-file {}: {}",
                override_path.display(),
                e
            );
            eprintln!("Error: override-file {}: {}", override_path.display(), e);
            std::process::exit(EXIT_INPUT_ERROR);
        }
    };

    let outcome = match run_adjudication(&mut pending, &mut overrides, prompter.as_mut(), operator)
    {
        Ok(o) => o,
        Err(e) => exit_with_error(pending_path, "adjudication", e),
    };
    info!("resolved {} adjudication(s)", outcome.resolved_count());

    if let Err(e) = overrides.write(override_path) {
        warn!(
            "failed to write override-file {}: {}",
            override_path.display(),
            e
        );
        eprintln!("Error: override-file {}: {}", override_path.display(), e);
        std::process::exit(EXIT_INPUT_ERROR);
    }
    if let Err(e) = pending.write(pending_path) {
        exit_with_error(pending_path, "pending-adjudications", e);
    }
}

/// `Prompter` implementation that reads one line per pending entry
/// from stdin. The terminal layer is intentionally dumb: it
/// `Display`-formats the pending entry's context to stdout, reads
/// the operator's response, and parses it into an
/// [`OperatorDecision`]. No business logic; per the design doc, all
/// apply-logic lives in the adjudication core, not the UI.
///
/// Currently supports `accept` / `a` for `AcceptSuggested`. Future
/// extensions (`override` with operator-supplied mapping, `defer` /
/// `skip`) ride the same trait.
struct TerminalPrompter;

impl Prompter for TerminalPrompter {
    fn ask(&mut self, entry: &PendingEntry) -> Result<OperatorDecision, AdjudicationError> {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        writeln!(out, "─────────────────────────────────────────────")?;
        writeln!(out, "Session: {}", entry.session_id)?;
        writeln!(out, "Kind: {:?}", entry.kind())?;
        match &entry.data {
            PendingKindData::SpeakerIdLowConfidence { suggested } => {
                writeln!(
                    out,
                    "Suggested mapping (the algorithm would have applied this):"
                )?;
                for (spk, action) in &suggested.mapping {
                    writeln!(out, "  {spk} = {action:?}")?;
                }
                writeln!(
                    out,
                    "Suggested inserted role: {} ({})",
                    suggested.inserted_role.code, suggested.inserted_role.tag
                )?;
            }
            PendingKindData::ParentRoleLookup {
                donor_speaker,
                speaker_mapping,
            } => {
                writeln!(
                    out,
                    "Parent-role lookup: donor speaker {donor_speaker:?} needs a role tag (MOT/FAT/etc.)"
                )?;
                writeln!(out, "Pre-recorded speaker mapping:")?;
                for (spk, action) in speaker_mapping {
                    writeln!(out, "  {spk} = {action:?}")?;
                }
            }
            PendingKindData::SanityScanMisclassification { suggested, reason } => {
                writeln!(out, "Sanity-scan flagged this session, reason:")?;
                writeln!(out, "  {reason}")?;
                writeln!(out, "Scan's suggested corrected mapping:")?;
                for (spk, action) in &suggested.mapping {
                    writeln!(out, "  {spk} = {action:?}")?;
                }
                writeln!(
                    out,
                    "Suggested inserted role: {} ({})",
                    suggested.inserted_role.code, suggested.inserted_role.tag
                )?;
            }
        }
        if let (Some(margin), Some(threshold)) = (entry.margin, entry.threshold_used) {
            writeln!(out, "Margin: {margin:.2}× (threshold {threshold:.2}×)")?;
        }
        if !entry.scores.is_empty() {
            writeln!(out, "Per-speaker Jaccard scores:")?;
            for (spk, score) in &entry.scores {
                writeln!(out, "  {spk} = {score:.4}")?;
            }
        }
        // Kind-specific prompt syntax, different decisions are valid
        // for different kinds.
        let prompt_hint = match &entry.data {
            PendingKindData::SpeakerIdLowConfidence { .. } => {
                "Decision [accept | override CODE TAG SPK=action [SPK=action ...] [note...]]: "
            }
            PendingKindData::ParentRoleLookup { .. } => "Decision [choose CODE TAG [note...]]: ",
            // Sanity-scan accepts the same decisions as
            // speaker-id-low-confidence: `accept` the scan's
            // suggested swap, or `override` with the operator's own.
            PendingKindData::SanityScanMisclassification { .. } => {
                "Decision [accept | override CODE TAG SPK=action [SPK=action ...] [note...]]: "
            }
        };
        write!(out, "{prompt_hint}")?;
        out.flush()?;
        drop(out);

        let stdin = std::io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let trimmed = line.trim();
        parse_operator_response(trimmed, entry)
    }
}

/// Parse one operator-response line into an [`OperatorDecision`].
/// Whitespace-tokenize first; the leading keyword selects the
/// decision variant. CODE and TAG tokens are preserved
/// case-sensitively (unlike the keyword); everything after them is
/// the optional note.
fn parse_operator_response(
    line: &str,
    entry: &PendingEntry,
) -> Result<OperatorDecision, AdjudicationError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let keyword = tokens.first().map(|t| t.to_ascii_lowercase());
    match keyword.as_deref() {
        Some("a" | "accept" | "accept-suggested") => {
            Ok(OperatorDecision::AcceptSuggested { note: None })
        }
        Some("choose") => match tokens.as_slice() {
            [_, code, tag, note_words @ ..] => {
                let note = if note_words.is_empty() {
                    None
                } else {
                    Some(note_words.join(" "))
                };
                Ok(OperatorDecision::ChooseRole {
                    inserted_role: InsertedRoleSpec {
                        code: (*code).to_string(),
                        tag: (*tag).to_string(),
                    },
                    note,
                })
            }
            _ => Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail:
                    "choose decision requires CODE and TAG tokens (e.g., \"choose MOT Mother\")"
                        .to_string(),
            }),
        },
        Some("override") => parse_override_mapping(&tokens, entry),
        Some(_) | None => Err(AdjudicationError::PrompterFailed {
            session_id: entry.session_id.clone(),
            detail: format!(
                "unrecognized operator input {line:?}; supported: \"accept\" / \"a\" / \"choose CODE TAG [note...]\" / \"override CODE TAG SPK=action [SPK=action ...] [note...]\""
            ),
        }),
    }
}

/// Parse `override CODE TAG SPK=action [SPK=action ...] [note...]`
/// into [`OperatorDecision::OverrideMapping`]. The assignment list
/// is consumed greedily while tokens match the `SPK=action` shape;
/// the first non-assignment token starts the optional trailing note.
fn parse_override_mapping(
    tokens: &[&str],
    entry: &PendingEntry,
) -> Result<OperatorDecision, AdjudicationError> {
    // Expect: ["override", CODE, TAG, ...]
    let (code, tag, rest) = match tokens {
        [_, code, tag, rest @ ..] => (*code, *tag, rest),
        _ => {
            return Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail:
                    "override decision requires CODE and TAG (e.g., \"override INV Investigator PAR0=rename\")"
                        .to_string(),
            });
        }
    };

    let mut mapping: BTreeMap<String, SpeakerAction> = BTreeMap::new();
    let mut split_idx = rest.len();
    for (i, token) in rest.iter().enumerate() {
        match parse_speaker_assignment(token) {
            AssignmentParse::Valid(spk, action) => {
                mapping.insert(spk, action);
            }
            AssignmentParse::Malformed => {
                // The token has an `=` (looks like an assignment)
                // but the action keyword is unrecognized. Surfacing
                // this as an error, rather than silently demoting
                // to note text, prevents the operator's
                // intended mapping from being lost to a typo.
                return Err(AdjudicationError::PrompterFailed {
                    session_id: entry.session_id.clone(),
                    detail: format!(
                        "malformed assignment {token:?}; expected SPK=rename or SPK=drop"
                    ),
                });
            }
            AssignmentParse::NotAnAssignment => {
                split_idx = i;
                break;
            }
        }
    }
    if mapping.is_empty() {
        return Err(AdjudicationError::PrompterFailed {
            session_id: entry.session_id.clone(),
            detail:
                "override decision requires at least one SPK=action assignment (e.g., PAR0=rename)"
                    .to_string(),
        });
    }
    let note_words = &rest[split_idx..];
    let note = if note_words.is_empty() {
        None
    } else {
        Some(note_words.join(" "))
    };

    Ok(OperatorDecision::OverrideMapping {
        mapping,
        inserted_role: InsertedRoleSpec {
            code: code.to_string(),
            tag: tag.to_string(),
        },
        note,
    })
}

/// Outcome of parsing one token of the override-mapping
/// assignment list. The three-arm split (rather than `Option`)
/// lets the caller distinguish a typo'd-action token (`PAR0=dropp`)
/// from a not-an-assignment token (`audio`), the typo is a hard
/// error, the latter starts the trailing note.
enum AssignmentParse {
    /// Token parsed as a valid `SPK=action` assignment.
    Valid(String, SpeakerAction),
    /// Token has `=` (looks like an assignment) but the action
    /// keyword is unrecognized, or the SPK token is empty.
    Malformed,
    /// Token has no `=`, caller treats this as the first note
    /// word.
    NotAnAssignment,
}

/// Parse one `SPK=action` token into an [`AssignmentParse`].
fn parse_speaker_assignment(token: &str) -> AssignmentParse {
    let Some((spk, action)) = token.split_once('=') else {
        return AssignmentParse::NotAnAssignment;
    };
    if spk.is_empty() {
        return AssignmentParse::Malformed;
    }
    match action.to_ascii_lowercase().as_str() {
        "rename" => AssignmentParse::Valid(spk.to_string(), SpeakerAction::Rename),
        "drop" => AssignmentParse::Valid(spk.to_string(), SpeakerAction::Drop),
        _ => AssignmentParse::Malformed,
    }
}

/// Render an `AdjudicationError` to stderr and exit with the
/// contract-defined code. `Io` / `Toml` exit 1 (file-level issues),
/// `PrompterFailed` / `DecisionKindMismatch` exit 2 (operator-supplied
/// decision rejected).
fn exit_with_error(path: &Path, label: &str, e: AdjudicationError) -> ! {
    warn!("adjudication failed on {} {}: {}", label, path.display(), e);
    eprintln!("Error: {} {}: {}", label, path.display(), e);
    let code = match e {
        AdjudicationError::FileIo { .. }
        | AdjudicationError::TerminalIo(_)
        | AdjudicationError::Toml(_) => EXIT_INPUT_ERROR,
        AdjudicationError::PrompterFailed { .. }
        | AdjudicationError::DecisionKindMismatch { .. } => EXIT_PRECONDITION,
    };
    std::process::exit(code);
}
