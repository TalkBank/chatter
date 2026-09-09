//! Every error code is demonstrated by a spec example, or says why it cannot be.
//!
//! # The question this answers
//!
//! `chatter validate` is the authority on whether a byte sequence is valid
//! CHAT. An error code is a rule that authority enforces. So for each code
//! there is one question worth gating: has anybody shown it firing on a real
//! file?
//!
//! The spec system already lowers every example to a real `.cha` and runs it
//! through both the parse and validation stages, recording what each stage
//! emitted in the observation snapshot. That observation is the evidence. A
//! code that appears in it has been demonstrated end to end. A code that never
//! appears has not, whatever its spec file asserts in prose.
//!
//! # The excusal, and the check that keeps it honest
//!
//! Three registry statuses legitimately have no example: `not_implemented`,
//! `deprecated`, and `unreachable_from_chat`, whose own documentation names
//! this exact situation ("Rule IS implemented, but no CHAT input can trigger
//! it"). An earlier draft of this check invented a parallel frontmatter key
//! before reading far enough to find that, which would have made two owners for
//! one fact.
//!
//! A status is therefore what drops a code out of the count, so a WRONG one is
//! a rule that ships with nothing asking for its example, and that is the
//! masking direction. The snapshot answers it: a code an example made FIRE is
//! implemented, reachable and not deprecated, whatever the registry says. That
//! is R1 below, a hard failure with no baseline, and it found E320 the day it
//! was written.
//!
//! # It was a Python script until 2026-09-08
//!
//! Same argument as `fabricated_ast`, and `test_hygiene`'s module doc makes it
//! in full: a check under `scripts/` reads the filesystem directly so no probe
//! can plant into it, and it needs hand-written wiring to run at all. As a
//! [`Gate`] it reads through a tree a probe can change, and its three rules
//! have probes instead of a hand-rolled test file.

use std::collections::{BTreeMap, BTreeSet};

use talkbank_spec_vocabulary::Status;
use talkbank_spec_vocabulary::observations::ObservationSnapshot;
use talkbank_spec_vocabulary::registry::{CodeRegistry, REGISTRY_PATH};

use crate::error_specs::SPEC_ERRORS;
use crate::gate::tree::RelPath;
use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, Tier, UnprovenRule, listing, report};

/// Where the observation snapshot lives, as one tree-relative spelling.
const SNAPSHOT: &str = "spec/observations/example-diagnostics.json";

/// Codes the registry calls `implemented` that no spec example demonstrates.
///
/// A RATCHET. This list may only SHRINK, and there are exactly two ways to
/// remove a code: write an example that triggers it, or establish that no CHAT
/// input reaches it and correct its `status` in the registry, whose vocabulary
/// already has the word. Both are real work on one code, and the second is not
/// a shortcut: it wants evidence in the spec file, not an assertion.
///
/// What each remaining code needs, as of 2026-09-08. All three have been
/// investigated; none is waiting for somebody to try harder.
///
/// - `E208`, `E232`: the model rule is implemented and the CANONICAL parser
///   never reaches it. `+hello` builds no `Word`, so the word-structure check
///   never sees one; the re2c backend DOES build it and reports E209/E232/E233
///   on the same file, which the parity baseline already carries as a
///   divergence. Recording re2c's output here would say a rule is demonstrated
///   while the validity AUTHORITY never fires it, so the snapshot stays
///   canonical-only. Two real actions, and both are the maintainer's: make the
///   canonical parser's recovery keep enough for the rule to run, or give the
///   registry a status for "implemented in the model, unreachable through this
///   lowering".
/// - `E324`: reachable only through the utterance FRAGMENT parser, the entry
///   the LSP and the fragment API use. The snapshot lowers each example to a
///   whole file and never takes that route, so the fragment path has no
///   coverage here at all. A fragment stage in the snapshot would demonstrate
///   this code and give that entry point its first.
/// - `E309`, `E319`, `E321`: reachable only under `--parser re2c`, measured
///   2026-09-08 (the canonical parser gives E602, E404 and E316 on the same
///   three inputs). Exactly the `E208`/`E232` case above, and it takes the
///   same answer: the snapshot is canonical-only on purpose, so these stay
///   tracked rather than being demonstrated by the oracle.
/// - `E302`: reachable, and only from a file with NO trailing newline, which
///   leaves the main tier flattened at EOF and routes it through the fragment
///   entry point. Every generated fixture ends in exactly one newline, put
///   there deliberately so a fixture is not testing the rule plus a
///   MISSING-newline recovery node, so no fixture can carry this input. With
///   the newline the same line reports E376 instead. Its named out-of-corpus
///   test is `e302_needs_a_file_that_does_not_end_in_a_newline`, in the CLI
///   crate's integration tests, which writes both byte sequences and asserts
///   the code fires for one and not the other.
///
/// # This list GREW on 2026-09-08, for the first time
///
/// Four codes arrived from `not_implemented`, a status that was FALSE for all
/// four: each fires, on a real `.cha` file, through `chatter validate`. That
/// status excuses a code from every rule this gate has, so the four were
/// invisible here as well as misdescribed there. The growth is a transfer out
/// of a false statement into a tracked one, and the ratchet's shrink-only rule
/// still governs what happens next: each of the four names what would remove
/// it.
const UNDEMONSTRATED: &[&str] = &["E208", "E232", "E302", "E309", "E319", "E321", "E324"];

/// Codes the registry calls `not_implemented` whose variant production code
/// still NAMES.
///
/// A RATCHET. This list may only SHRINK, and there are exactly two ways off
/// it: find an input that reaches the code, which makes the status false and
/// R1 or R2 takes over; or remove the last production site, which is what a
/// retirement looks like.
///
/// # Why this list has to exist
///
/// `not_implemented` excuses a code from R1, R2 and R3 alike, so it is the one
/// status under which a code can be wrong indefinitely and nothing measures
/// it. That is not hypothetical: thirteen codes were corrected out of it in the
/// week this list was written, eight because the example runner had no way to
/// ask for their opt-in rules and five because a `status_note` had tried a
/// handful of shapes and concluded that none can work. Every one had a
/// production emit site the whole time.
///
/// A production site is not proof the code fires: the site can be dominated,
/// or gated by a precondition nothing supplies. It IS a contradiction to
/// explain, and the explanation is what each entry below carries.
///
/// What each remaining code needs, as of 2026-09-08. All thirteen were
/// investigated that week; none is waiting for somebody to try harder.
///
/// - `E003`: reachable only through an EMPTY `SinToken`, which no parser can
///   build (both construct it through the fallible `SinToken::new` and drop
///   the error). Its only route is `serde`, and the in-tree test that pins it
///   builds one that way. A candidate for `unreachable_from_chat` once
///   somebody rules on whether the JSON boundary counts as CHAT.
/// - `E729`, `E731`: no production site CONSTRUCTS these; what the filter sees
///   is a `pub const` alias in `errors/codes/temporal.rs`, which is production
///   and does name the variant. They are CLAN CHECK 84 and 133, real rules
///   nobody has implemented, so the status is honest and the work is to
///   implement or adjudicate them.
///
///   `E101` was in this list for one run and the gate's other direction threw
///   it out, which is the check earning its place on the day it was written: a
///   hand-made survey of the same question had counted it as named because a
///   variant list in an LSP TEST mentions it, and the production filter here
///   does not read test files. E101 has no production site at all and is fully
///   covered by E326 for every stray line shape tried.
/// - `E304`, `E322`, `E323`: emit sites in `parse_prefix`, all three reached
///   only for a `main_tier` node with an absent or zero-width speaker or
///   colon, which the grammar cannot produce: damaged prefixes become a
///   document-level ERROR and E301 or E316 wins first.
/// - `E325`, `E364`, `E365`, `E508`: emit sites exist and every route to them
///   is blocked by an earlier diagnostic. Each was traced to the dominating
///   code that week.
/// - `E310`: reachable, but not through `chatter validate`. It needs a
///   document past the 32-bit coordinate space, which the whole-file guard now
///   refuses before any parse; `a_document_past_the_coordinate_space_is_refused`
///   in `talkbank-model` is its out-of-corpus test. Correcting the status is
///   the next action and wants a ruling on what `implemented` means for a code
///   whose only route is a four-gibibyte file.
/// - `E999`: its two production sites are inside the public fragment API, so
///   it is reachable from a LIBRARY caller and not from a file. Same shape as
///   E324, and the same fragment stage in the snapshot would reach both.
const UNIMPLEMENTED_BUT_NAMED: &[&str] = &[
    "E003", "E304", "E310", "E322", "E323", "E325", "E364", "E365", "E508", "E729", "E731", "E999",
];

/// Every implemented error code is demonstrated, or excused by a status the
/// snapshot does not disprove.
pub struct ErrorCodeDemonstrationGate;

impl Gate for ErrorCodeDemonstrationGate {
    fn name(&self) -> &'static str {
        "error codes are demonstrated"
    }

    fn check(&self, tree: ReadTree) -> Outcome {
        // `Presence`, not a bool called `missing` that holds the opposite. The
        // first draft did exactly that, correct by one `!`; a boolean has lost
        // the question it answers, and an inversion here declares the gate
        // unavailable when its inputs ARE present, which prints as a green skip
        // in the inner loop.
        for (input, presence) in [
            (SPEC_ERRORS, Presence::of(tree.dir_exists(SPEC_ERRORS))),
            (REGISTRY_PATH, Presence::of(tree.file_exists(REGISTRY_PATH))),
            (SNAPSHOT, Presence::of(tree.file_exists(SNAPSHOT))),
        ] {
            match presence {
                Presence::Present => {}
                Presence::Absent => {
                    return Outcome::unavailable(
                        format!(
                            "{input} is not in this checkout; a sparse or filtered clone omits it"
                        ),
                        Tier::PrePush,
                    );
                }
            }
        }

        let evidence = match Evidence::read(&tree) {
            Ok(evidence) => evidence,
            Err(failure) => return Outcome::failed(failure),
        };

        // R1: a status the snapshot disproves. No baseline: there is no honest
        // reason for this set to be non-empty, and what it can catch is bounded
        // by the snapshot rather than by anyone's effort.
        let falsified: Vec<String> = evidence
            .status
            .iter()
            .filter(|(code, status)| {
                matches!(
                    status,
                    Status::NotImplemented | Status::Deprecated | Status::UnreachableFromChat
                ) && evidence.fired.contains(*code)
            })
            .map(|(code, status)| {
                format!("{code} is registered {status:?} and a spec example emits it")
            })
            .collect();
        if !falsified.is_empty() {
            return Outcome::failed(listing(
                "FAIL: a code's registry status says it does not fire, and the\n\
                 observation snapshot records it firing. The status excuses the code\n\
                 from this gate, so a wrong one is a rule that ships with nothing\n\
                 asking for its example. Correct the status in\n\
                 spec/codes/error-codes.toml:",
                &falsified,
            ));
        }

        // R2: the undemonstrated set, both directions.
        let now = evidence.undemonstrated();
        let recorded: BTreeSet<&str> = UNDEMONSTRATED.iter().copied().collect();
        let appeared: Vec<&str> = now
            .iter()
            .map(String::as_str)
            .filter(|code| !recorded.contains(code))
            .collect();
        let closed: Vec<&str> = recorded
            .iter()
            .copied()
            .filter(|code| !now.contains(*code))
            .collect();
        if !appeared.is_empty() || !closed.is_empty() {
            return Outcome::failed(report([
                listing(
                    "FAIL: error code(s) the registry calls `implemented` that no spec\n\
                     example demonstrates. An unexercised rule is one nobody has shown\n\
                     firing on a real file, which is the only evidence that matters for\n\
                     a validity authority. Either write an example that triggers it, or,\n\
                     if you find no CHAT input can, correct its status in\n\
                     spec/codes/error-codes.toml, which already has a word for that:",
                    &appeared,
                ),
                listing(
                    "FAIL: these are now accounted for, so remove them from\n\
                     UNDEMONSTRATED in this commit. A ratchet that is not tightened when\n\
                     it is earned stops being one:",
                    &closed,
                ),
            ]));
        }

        // R4: the self-fulfilling excuse, both directions.
        //
        // R1 catches a `not_implemented` code the SNAPSHOT disproves, which
        // needs an example, which a `not_implemented` code does not have. So
        // the only mechanical signal left is that production code NAMES the
        // variant, and this is the one rule that reads it.
        let named_now: BTreeSet<&str> = evidence
            .status
            .iter()
            .filter(|(code, status)| {
                **status == Status::NotImplemented && evidence.applied.contains(*code)
            })
            .map(|(code, _)| code.as_str())
            .collect();
        let excused: BTreeSet<&str> = UNIMPLEMENTED_BUT_NAMED.iter().copied().collect();
        let unexplained: Vec<&str> = named_now.difference(&excused).copied().collect();
        let stale: Vec<&str> = excused.difference(&named_now).copied().collect();
        if !unexplained.is_empty() || !stale.is_empty() {
            return Outcome::failed(report([
                listing(
                    "FAIL: code(s) the registry calls `not_implemented` whose variant\n\
                     production code names. That status excuses a code from every other\n\
                     rule here, so it is the one place a wrong answer can sit forever.\n\
                     A production site is not proof the code fires, but it is a\n\
                     contradiction that owes an explanation: find an input, or add the\n\
                     code to UNIMPLEMENTED_BUT_NAMED saying what blocks it:",
                    &unexplained,
                ),
                listing(
                    "FAIL: these are excused as `not_implemented` with a production site\n\
                     and no longer have one, or are no longer `not_implemented`. Remove\n\
                     them from UNIMPLEMENTED_BUT_NAMED in this commit; a ratchet that is\n\
                     not tightened when it is earned stops being one:",
                    &stale,
                ),
            ]));
        }

        // R3: the split, which is the actionable half of a clean run.
        let (unapplied, missing_example): (Vec<&String>, Vec<&String>) = now
            .iter()
            .partition(|code| !evidence.applied.contains(*code));
        tree.clean(format!(
            "{} codes carry a spec, {} undemonstrated, at the baseline\n     \
             {} that no PRODUCTION file names, so nothing applies the rule: {}\n     \
             {} that production code names, so what is missing is the example: {}\n     \
             {} more called `not_implemented` while production names them, each \
             with a stated blocker",
            evidence.declared.len(),
            now.len(),
            unapplied.len(),
            join(&unapplied),
            missing_example.len(),
            join(&missing_example),
            UNIMPLEMENTED_BUT_NAMED.len(),
        ))
    }

    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a code whose status the snapshot disproves",
            "and a spec example emits it",
            |edit| {
                // E231 fires in the snapshot. Calling it not implemented is the
                // exact lie R1 exists to refuse, and the one it found on the
                // day it was written.
                let registry = edit.read(REGISTRY_PATH)?;
                let at = registry
                    .find("code = \"E231\"")
                    .ok_or_else(|| crate::gate::PlantFailed::new("E231 is not in the registry"))?;
                let status = registry[at..]
                    .find("status = ")
                    .map(|offset| at + offset)
                    .ok_or_else(|| {
                        crate::gate::PlantFailed::new("E231's entry declares no status")
                    })?;
                let end = match registry[status..].find('\n') {
                    Some(offset) => status + offset,
                    // A registry whose last line IS the status, with no
                    // trailing newline. Written out rather than defaulted:
                    // that is a different file, not a value to invent.
                    None => registry.len(),
                };
                let mut planted = String::with_capacity(registry.len());
                planted.push_str(&registry[..status]);
                planted.push_str("status = \"not_implemented\"");
                planted.push_str(&registry[end..]);
                edit.write(REGISTRY_PATH, planted);
                Ok(())
            },
        )
        .refusing(
            "an implemented code nothing demonstrates any more",
            // A fragment that lies on ONE line of the heading: the `\n\`
            // continuations mean a guess at where the text wraps is a guess,
            // and a probe that names text the report does not contain fails
            // for the wrong reason.
            "An unexercised rule is one nobody has shown",
            |edit| {
                // Every observation gone: the snapshot still parses and records
                // nothing, which is what a regenerated-but-broken snapshot
                // looks like.
                edit.write(
                    SNAPSHOT,
                    "{\n  \"generated_by\": \"probe\",\n  \"examples\": []\n}\n",
                );
                Ok(())
            },
        )
        .refusing(
            "a production site appears for a code called `not_implemented`",
            "whose variant",
            |edit| {
                // E101 is `not_implemented` and NO production file names
                // `ErrorCode::InvalidLineFormat`; the baseline records that,
                // because R4's other direction threw E101 out of it on the day
                // it was written. Giving it a production site is therefore the
                // only plant that reaches R4 without tripping something else.
                //
                // The two rules above it both shadow the obvious plants.
                // Flipping a code's status to `not_implemented` trips R1 when
                // the snapshot records it firing, and emptying the snapshot so
                // R1 stays quiet trips R2 over every implemented code at once.
                // R4's own territory is exactly a code with no snapshot record
                // and no example, which is the state that made the excuse
                // self-fulfilling.
                edit.write(
                    "crates/talkbank-model/src/probe_emit_site.rs",
                    "//! Planted by a probe.\nfn plant() -> ErrorCode { ErrorCode::InvalidLineFormat }\n",
                );
                Ok(())
            },
        )
        .refusing(
            "the snapshot is not JSON at all",
            "cannot read the observation snapshot",
            |edit| {
                edit.write(SNAPSHOT, "not json\n");
                Ok(())
            },
        )
        .refusing(
            "the spec directory is there and offers nothing",
            "specs",
            |edit| {
                // PRESENT and EMPTY, which the precondition cannot see: it asks
                // `dir_exists`, and the directory does exist. Every set
                // difference is then empty and every rule passes, and
                // `UNDEMONSTRATED` is expected to reach empty, so nothing else
                // here would notice. This is the vacuity both sibling gates
                // declare unprobed and this one closes.
                edit.hide_files_under(SPEC_ERRORS);
                Ok(())
            },
        )
        .refusing(
            "the crates tree cannot be ENUMERATED, so the split is a floor",
            "cannot enumerate the crates tree",
            |edit| {
                edit.fail_walk_under("crates");
                Ok(())
            },
        )
        .refusing(
            "a production file cannot be read, so the split is a floor",
            "a file could not be read",
            |edit| {
                edit.write_bytes(
                    "crates/talkbank-model/src/probe_unreadable.rs",
                    vec![0xFF, 0xFE],
                );
                Ok(())
            },
        )
        .declining(
            "the observation snapshot is not in this checkout",
            SNAPSHOT,
            Tier::PrePush,
            |edit| {
                edit.remove_file(SNAPSHOT);
                Ok(())
            },
        )
        .declining(
            "the spec directory is not in this checkout at all",
            SPEC_ERRORS,
            Tier::PrePush,
            |edit| {
                edit.remove_dir(SPEC_ERRORS);
                Ok(())
            },
        )
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}

/// Whether an input this gate needs is in the tree.
///
/// Two names rather than a bool, because they are two operator situations and a
/// bool has lost the question it answers.
#[derive(Clone, Copy)]
enum Presence {
    Present,
    Absent,
}

impl Presence {
    /// The one place a `bool` from the tree becomes the question it answers.
    fn of(exists: bool) -> Self {
        match exists {
            true => Self::Present,
            false => Self::Absent,
        }
    }
}

/// Everything the three rules are decided from, read once.
struct Evidence {
    /// Codes with a spec file.
    declared: BTreeSet<String>,
    /// Codes some example actually emitted, at either stage.
    fired: BTreeSet<String>,
    /// Each code's registry status.
    status: BTreeMap<String, Status>,
    /// Codes some non-generated, non-test production file NAMES by variant.
    applied: BTreeSet<String>,
}

impl Evidence {
    /// Read the three inputs, refusing any that cannot be parsed.
    ///
    /// # Errors
    ///
    /// When a file is unreadable or malformed. Never a partial answer: a set
    /// difference over a short set reports codes as undemonstrated that simply
    /// were not read.
    fn read(tree: &ReadTree) -> Result<Self, String> {
        let registry_text = tree
            .read_to_string(&RelPath::new(REGISTRY_PATH))
            .map_err(|err| format!("FAIL: cannot read the code registry: {err}"))?;
        let registry = CodeRegistry::parse(&registry_text)
            .map_err(|err| format!("FAIL: the code registry does not parse: {err}"))?;

        let snapshot_text = tree
            .read_to_string(&RelPath::new(SNAPSHOT))
            .map_err(|err| format!("FAIL: cannot read the observation snapshot: {err}"))?;
        let snapshot: ObservationSnapshot = serde_json::from_str(&snapshot_text)
            .map_err(|err| format!("FAIL: cannot read the observation snapshot: {err}"))?;

        // The CODE each spec declares in its own frontmatter, through the one
        // loader that resolves it against this registry, never the filename.
        //
        // Two readings were wrong before this. `looks_like_a_code` on the stem
        // answers yes to `E202_missing_form_type`, so the spec that DOCUMENTS
        // E202 was counted as a code of its own. Narrowing to an exact stem
        // then dropped four implemented codes out of the gate entirely, because
        // E202, E241, E243 and E604 have no file of their own: their only spec
        // is a suffixed one. All four fire today, so nothing was missed yet,
        // and the day one stopped firing the gate would have stayed green.
        // That is the masking direction, in the gate whose subject is masking.
        //
        // It also closes a third way to remove a code from the ratchet that the
        // doc on `UNDEMONSTRATED` says does not exist: RENAMING `E208.md` to
        // `E208_empty_replacement.md`, the convention four codes already use,
        // took E208 out of `declared`, into `closed`, and made this gate print
        // an instruction to delete its own ratchet entry.
        let specs = crate::error_specs::load_from(tree)
            .map_err(|why| format!("FAIL: the error specs do not load: {why}"))?;
        let declared: BTreeSet<String> = specs
            .iter()
            .map(|spec| spec.declared_code().to_string())
            .collect();
        // A FLOOR. An empty spec set makes every difference below empty and
        // every rule pass, and `UNDEMONSTRATED` is expected to reach empty, so
        // nothing else here would notice. `load_from` refuses an empty
        // DIRECTORY for this reason; this catches the case where it read files
        // and none declared a code.
        if declared.is_empty() {
            return Err("FAIL: no error spec declared a code, so every comparison \
                        below is against nothing and would report clean"
                .to_owned());
        }

        let mut fired = BTreeSet::new();
        for example in &snapshot.examples {
            fired.extend(example.parse.iter().cloned());
            fired.extend(example.validation.iter().cloned());
        }

        let status = registry
            .entries()
            .iter()
            .map(|entry| (entry.code().to_string(), entry.status()))
            .collect();

        let applied = applied_codes(tree, &registry)?;

        Ok(Self {
            declared,
            fired,
            status,
            applied,
        })
    }

    /// Codes with a spec file, demonstrated by nothing, excused by nothing.
    fn undemonstrated(&self) -> BTreeSet<String> {
        self.declared
            .iter()
            .filter(|code| !self.fired.contains(*code))
            .filter(|code| {
                !matches!(
                    self.status.get(*code),
                    Some(Status::NotImplemented | Status::Deprecated | Status::UnreachableFromChat)
                )
            })
            .cloned()
            .collect()
    }
}

/// Codes whose `ErrorCode` VARIANT a production file names.
///
/// Production code names a rule by its variant, never by its number, so the
/// number is the wrong thing to look for; and the number is exactly what a
/// maintainer writes in a comment beside code that does something else. Getting
/// this from prose put three codes in the wrong bucket and two more in the
/// other wrong bucket, and the bucket is the actionable half of a clean run.
///
/// # Errors
///
/// When the walk or a read fails, for the reason [`Evidence::read`] gives.
fn applied_codes(tree: &ReadTree, registry: &CodeRegistry) -> Result<BTreeSet<String>, String> {
    let files = tree
        .files_under("crates")
        .map_err(|err| format!("FAIL: cannot enumerate the crates tree: {err}"))?;
    let mut applied = BTreeSet::new();
    for path in files {
        if !path.extension_is("rs") {
            continue;
        }
        let as_str = path.as_str();
        // By CRATE, not by a path substring. `crates/talkbank-parser-tests/src`
        // does not contain `/tests/`, so 50 files of gate and harness code were
        // scanned as production and seven codes were counted APPLIED on the
        // strength of a CHECK-parity table in this very crate. That is the
        // defect `applied_codes` was written to fix, reintroduced one level up
        // by a heuristic.
        if as_str.starts_with("crates/talkbank-parser-tests/")
            || as_str.contains("/generated/")
            || as_str.contains("/tests/")
            || as_str.contains("/target/")
        {
            continue;
        }
        let name = path.file_name();
        // `ends_with("tests.rs")` also excludes `contests.rs`; a boundary is
        // what makes it a suffix of the NAME rather than of the word.
        if name.starts_with("generated_") || name == "tests.rs" || name.ends_with("_tests.rs") {
            continue;
        }
        let text = tree
            .read_to_string(&path)
            .map_err(|err| format!("FAIL: a file could not be read: {err}"))?;
        // Cut at the first inline test module, so a code named only by a unit
        // test in a production file is not an emit site, and blank the rest so
        // a comment or a string naming a variant is not a use of it.
        // `split_once`, not `split(..).next().unwrap_or_default()`: `split`
        // always yields a first piece, so that fallback stood for a state that
        // does not exist, in the spelling this project bans on sight.
        let head = text
            .split_once("#[cfg(test)]")
            .map_or(text.as_str(), |(before, _)| before);
        let code_only = crate::test_hygiene::blank_literals(head);
        for entry in registry.entries() {
            let needle = format!("ErrorCode::{}", entry.variant());
            if code_only.contains(&needle) {
                applied.insert(entry.code().to_string());
            }
        }
    }
    Ok(applied)
}

/// A space-separated list, or `none`, never a bare blank line.
fn join(codes: &[&String]) -> String {
    match codes.is_empty() {
        true => "none".to_owned(),
        false => codes
            .iter()
            .map(|code| code.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// What no plant of this gate reaches.
static UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "the SPLIT in the clean summary, which no verdict reads",
        "R3 is reported, never compared, so no plant of `check` can make it \
         wrong in a way this gate notices. It is the actionable half of a clean \
         run and it is derived rather than recorded, which is the reason it is \
         computed at all.",
    ),
    UnprovenRule::new(
        "ALIASED or STARRED imports: `use ErrorCode::*` then a bare variant",
        "The scan names `ErrorCode::<Variant>` as it is written at the call \
         site. No such module exists today, and following an import is a Rust \
         parser rather than a scan.",
    ),
    UnprovenRule::new(
        "the STALE direction of either compiled ratchet",
        "`UNDEMONSTRATED` and `UNIMPLEMENTED_BUT_NAMED` are `const` arrays \
         compiled into this gate, so a plant that edits their source in the \
         overlay changes nothing the running check reads. Both directions are \
         implemented and the tightening one is exercised by real commits \
         rather than by a probe. Measured, not assumed: a probe that added an \
         entry to the second list reported INERT, which is how this limit was \
         found. Reaching it would mean the baselines living in a file the tree \
         can hand over, and a data file nobody has to justify in review is a \
         worse ratchet than a compiled one somebody must edit.",
    ),
    UnprovenRule::new(
        "BLOCK COMMENTS, which `blank_literals` does not handle",
        "A variant named inside `/* ... */` counts as an emit site. Same limit \
         three sibling gates declare, and a plant of that shape would be a \
         false positive of the scanner rather than of the rule.",
    ),
];
