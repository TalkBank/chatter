//! Every declared error code has a spec file under `spec/errors/`.
//!
//! Why gates live in library modules rather than a binary's `main`: see
//! [`crate::gate`]. This one's own history is that it was
//! `error_coverage::test_error_code_spec_coverage`, the most deceptive member
//! of that family, because it genuinely RAN in CI and appeared in the passing
//! list while computing `missing_specs`, printing them, and asserting nothing.

use std::collections::BTreeSet;

use talkbank_model::ErrorCode;

use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, Tier, UnprovenRule, listing};

/// The declared-code to spec-file correspondence.
pub struct ErrorCodeSpecGate;

/// The registry every spec resolves against, in one spelling, owned by the
/// vocabulary crate rather than retyped here.
const REGISTRY: &str = talkbank_spec_vocabulary::registry::REGISTRY_PATH;

/// Rules of this gate no plant can reach.
///
/// See [`crate::gate::UnprovenRule`]. The first is the one that matters: the
/// gate's own rule has two sides, and only one of them is on the filesystem.
const UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "the DECLARED side of the coverage rule",
        "`ErrorCode::iter()` is compiled in from `generated_error_code.rs`, so \
         no tree edit adds or removes a declared code without a rebuild. Every \
         plant can only take a SPEC away. The same set-difference branch is \
         exercised, but the scenario the gate exists for (a code added to the \
         registry, `just spec-gen` run, the spec file forgotten) is not \
         reproducible by planting.",
    ),
    UnprovenRule::new(
        "an EMPTY declared set",
        "`missing.is_empty()` returns clean whenever `declared` is empty, \
         printing `0 declared error code(s) all have specs`. Only a defect in \
         the derive macro could produce it and no tree plant can. The summary \
         would at least SAY zero, which is why this is a note rather than a \
         change.",
    ),
    UnprovenRule::new(
        "per-file read failure and the directory-walk failure",
        "Reachable only through permissions or IO faults, never through \
         content. The non-UTF-8 case IS reachable and is probed by the \
         catch-all gate's own suite over the same overlay.",
    ),
    UnprovenRule::new(
        "the non-UTF-8 FILENAME arm, which `continue`s rather than reporting",
        "Unreachable by construction: the enumeration already guarantees a \
         UTF-8, code-shaped stem. Worth naming because it is the one place in \
         this path that SKIPS silently.",
    ),
    UnprovenRule::new(
        "everything about spec CONTENT",
        "The gate reads FILENAMES for coverage and never looks at what a spec \
         says, so a one-line stub satisfies it. Status, examples, claims and \
         whether the rule is implemented are somebody else's gates. No probe \
         can make it fail on that, because it is not a rule here.",
    ),
];

impl Gate for ErrorCodeSpecGate {
    fn name(&self) -> &'static str {
        "error codes have spec files"
    }

    fn check(&self, tree: ReadTree) -> Outcome {
        // ABSENT and BROKEN are different facts. A filtered clone that omits
        // `spec/` cannot answer the coverage question at all, and reporting
        // that as a corpus defect sends somebody hunting for a spec file they
        // never had. From the pre-push tier upward it is fatal either way.
        //
        // Each input is checked as the KIND it is. The loop that used to stand
        // here asked `!dir_exists(input) && !file_exists(input)` for both, a
        // disjunction that accepted a FILE named `spec/errors` and a DIRECTORY
        // named `spec/codes/error-codes.toml`, which is one wrong state per
        // input bought for one line saved.
        let missing = if !tree.dir_exists(crate::error_specs::SPEC_ERRORS) {
            Some(crate::error_specs::SPEC_ERRORS)
        } else if !tree.file_exists(REGISTRY) {
            Some(REGISTRY)
        } else {
            None
        };
        if let Some(input) = missing {
            return Outcome::unavailable(
                format!("{input} is not in this checkout; a sparse or filtered clone omits it"),
                Tier::PrePush,
            );
        }

        // `ErrorCode::iter()` IS the declaration: the derive macro generates it
        // from the same `#[code("...")]` attributes this used to grep for as
        // TEXT, against a hardcoded path to `errors/codes/error_code.rs`. The
        // text scan also needed a guard against its own extraction silently
        // returning the empty set, which is a test guarding an invariant the
        // type already carries. Using the type deletes the path, the regex, the
        // guard, and the failure mode.
        //
        // Since R1 those attributes are generated from
        // `spec/codes/error-codes.toml`, so this asks a CLEANER question than
        // it used to: not "do two vocabularies agree" (they are one now, and a
        // spec naming an unregistered code no longer loads at all) but "is
        // every registered code DOCUMENTED by at least one spec file". That is
        // coverage, and it is worth its own gate.
        let declared: BTreeSet<ErrorCode> = ErrorCode::iter().copied().collect();

        // The directory walk and the `<CODE>_<slug>.md` convention live in
        // `error_specs`, which owns the reason the split is on the FIRST
        // underscore: `starts_with` would let a hypothetical `E21` claim
        // `E210.md` and report coverage it does not have. That reasoning
        // used to be written out here as well, which is two owners for one
        // rule and two places to change it.
        let specs = match crate::error_specs::load_from(&tree) {
            Ok(specs) => specs,
            Err(why) => return Outcome::failed(why),
        };
        let specified = crate::error_specs::specified_codes(&specs);

        let missing: Vec<&str> = declared
            .difference(&specified)
            .map(ErrorCode::as_str)
            .collect();

        // NO exemption list, deliberately. There was one, with eleven entries,
        // and every single one was dead: eight named codes that are declared
        // AND have a spec, three named codes the model no longer declares, nine
        // of them under one comment reading "deprecated". Keeping the empty list
        // plus its both-directions machinery meant thirty-five lines that could
        // not execute. If an exemption is ever genuinely needed, copy the shape
        // from `HARNESS_CANNOT_TRIGGER` in
        // `spec/runtime-tools/tests/error_spec_codes.rs`: a stated reason, and a
        // check in both directions so a dead entry fails.
        if missing.is_empty() {
            return tree.clean(format!(
                "{} declared error code(s) all have specs",
                declared.len()
            ));
        }

        Outcome::failed(listing(
            &format!(
                "FAIL: {} registered error code(s) have no spec file in {}.\n\
                 Write `<CODE>_<slug>.md`:",
                missing.len(),
                crate::error_specs::spec_dir(tree.root()).display()
            ),
            missing,
        ))
    }

    /// One probe per rule the gate can reach, and they are NOT all the same
    /// rule: the coverage difference is this gate's own, while the loader's
    /// five refusals run first and would mask it. That is why each plant is
    /// applied alone, and why a probe that fails inside `load` says nothing
    /// about whether the comparison works. The suite says which is which.
    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a registered code whose spec file is deleted",
            "registered error code(s) have no spec file in",
            |edit| {
                edit.remove_file("spec/errors/E002.md");
                Ok(())
            },
        )
        // Both preconditions, because the loop reads two inputs and a
        // gate that checked only the first would answer a registry-less
        // checkout with a read failure rather than a skip.
        .declining(
            "the spec directory is not in this checkout at all",
            "spec/errors is not in this checkout",
            Tier::PrePush,
            |edit| {
                edit.remove_dir(crate::error_specs::SPEC_ERRORS);
                Ok(())
            },
        )
        .declining(
            "the code registry is not in this checkout",
            REGISTRY,
            Tier::PrePush,
            |edit| {
                edit.remove_file(REGISTRY);
                Ok(())
            },
        )
        .refusing(
            "a spec file renamed so the enumeration no longer offers it",
            "registered error code(s) have no spec file in",
            |edit| {
                let text = edit.read("spec/errors/E002.md")?;
                edit.remove_file("spec/errors/E002.md");
                edit.write("spec/errors/notes.md", text);
                Ok(())
            },
        )
        .refusing(
            "the spec directory is there and holds no spec",
            "no spec files under",
            |edit| {
                edit.hide_files_under(crate::error_specs::SPEC_ERRORS);
                Ok(())
            },
        )
        .refusing(
            "a spec documenting a code nothing registers",
            "is not declared in",
            |edit| {
                edit.write(
                    "spec/errors/E998_probe.md",
                    "+++\ncode = 'E998'\nname = 'Probe'\n+++\n\n## Description\n\nProbe.\n",
                );
                Ok(())
            },
        )
        .refusing(
            "a filename and a frontmatter naming different codes",
            "One spec, one code.",
            |edit| edit.replace_once("spec/errors/E203.md", "code = 'E203'", "code = 'E207'"),
        )
        .refusing(
            "a spec file with no frontmatter block",
            "no `+++` frontmatter",
            |edit| {
                let text = edit.read("spec/errors/E002.md")?;
                edit.write("spec/errors/E002.md", text.replacen("+++\n", "", 1));
                Ok(())
            },
        )
        .refusing(
            "frontmatter carrying a field the schema does not define",
            "unknown field `bogus`",
            |edit| {
                edit.replace_once(
                    "spec/errors/E002.md",
                    "name = 'TestError'\n",
                    "name = 'TestError'\nbogus = 'x'\n",
                )
            },
        )
        .refusing(
            "a registry violating a cross-entry rule",
            "is registered twice",
            |edit| {
                let text = edit.read(REGISTRY)?;
                edit.write(
                    REGISTRY,
                    format!(
                        "{text}\n[[code]]\ncode = \"E001\"\n\
                             variant = \"InternalErrorDuplicateProbe\"\n\
                             summary = \"Duplicate probe.\"\n\
                             kind = \"Invalidity\"\nstatus = \"implemented\"\n"
                    ),
                );
                Ok(())
            },
        )
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}
