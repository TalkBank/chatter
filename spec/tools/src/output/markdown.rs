//! # Markdown Documentation Generator
//!
//! Generates publishable error documentation in Markdown format.
//!
//! Each generated page surfaces the implementation status from the source
//! spec (`spec/errors/*.md`) as a visible badge so researchers can tell at a
//! glance whether the validator actually enforces the documented check.

use crate::spec::by_code::{CodeSpecs, CodeSpecsView, SpecsByCode};
use crate::spec::error::ErrorSpec;
use crate::spec::metadata::{SpecErrorCode, Status};

/// Render the short badge label shown in the status callout and metadata.
///
/// Every arm is spelled out because [`Status`] is a closed set: there is no
/// "unknown value" to pass through any more, since one cannot survive the
/// loader. The old signature took a `&str` and ended `other => other`, which was
/// documented as letting "spec authors notice typos in generated docs" -- a typo
/// now stops the run and names its file, which is where it should have been
/// noticed.
///
/// `Deprecated` renders its own spec-file name, exactly as the string version
/// did by falling through, and takes it from [`Status::as_str`] rather than
/// respelling it here: a literal `"deprecated"` in this arm would go stale the
/// day the vocabulary is renamed, which is the drift this whole module exists
/// to remove.
fn status_badge(status: Status) -> &'static str {
    match status {
        Status::Implemented => "✅ Active",
        Status::NotImplemented => "⏳ Planned",
        // Active too: the rule fires. What it cannot do is be triggered from a
        // CHAT file, so it carries no corpus fixture and names its own test.
        Status::UnreachableFromChat => "✅ Active (not reachable from CHAT)",
        Status::Deprecated => Status::Deprecated.as_str(),
    }
}

/// The compact glyph for a listing, and the ONE owner of it.
///
/// The index had its own inline `match` over `Status`, a third hand-written
/// table beside this and [`status_callout`], and it disagreed with
/// [`status_badge`]: `UnreachableFromChat` rendered `?` in the index while its
/// own page said `✅ Active (not reachable from CHAT)`. E768 is in that state,
/// so the landing page reported an ACTIVE rule with a symbol its own legend
/// did not define.
///
/// Both active states are `✅` here, which is what the badge already said in
/// words. `Deprecated` keeps `?`, and [`LEGEND`] explains it.
fn status_icon(status: Status) -> &'static str {
    match status {
        Status::Implemented | Status::UnreachableFromChat => "✅",
        Status::NotImplemented => "⏳",
        Status::Deprecated => "?",
    }
}

/// The legend the index prints, kept beside the glyphs it explains.
///
/// The previous legend named two glyphs where the table could emit three.
const LEGEND: &str = "Status: ✅ = active in the validator, ⏳ = documented but not yet enforced, ? = deprecated.\n\n";

/// Render the callout that explains the badge meaning to readers.
///
/// Lives immediately under the title so researchers scanning the docs see
/// the enforcement state before any other metadata.
///
/// `None` is "this status needs no sentence", NOT an empty one. It returned
/// `"\n\n"` for those two, inherited from an older `_` arm, and the caller
/// pasted a `"; "` separator before it unconditionally, so every deprecated and
/// every unreachable page published a blockquote ending in a dangling
/// semicolon: `> deprecated; ` and `> ✅ Active (not reachable from CHAT); `.
/// Making the absent case a variant means the caller has to decide, and it
/// cannot decide by accident.
fn status_callout(status: Status) -> Option<&'static str> {
    match status {
        Status::Implemented => Some("This check is active in the validator."),
        Status::NotImplemented => Some(
            "This check is documented but not yet enforced by the validator. The error code will not fire until implementation is complete.",
        ),
        // A badge that already says everything: "deprecated" and "not reachable
        // from CHAT" need no gloss. Written out so that giving either a real
        // callout stays a choice rather than an edit to a catch-all.
        Status::Deprecated | Status::UnreachableFromChat => None,
    }
}

/// Where a spec's rendering sits on the page it is part of.
///
/// ONE fact with two consequences (the title text and the depth of every
/// heading below it), which is why it is one enum and not two parameters. The
/// alternative considered and rejected was rendering each spec at `#` and then
/// demoting its headings in the resulting string, which is re-parsing text this
/// module had just serialized.
#[derive(Debug, Clone, Copy)]
enum SpecPlacement {
    /// The spec IS the page: one spec claims this code.
    WholePage,
    /// The spec is one section of a page shared with other specs for the code.
    SectionOfCode,
}

impl SpecPlacement {
    /// The heading line that opens this spec's rendering.
    ///
    /// The whole-page form repeats the code because it is the page title a
    /// reader arrives at holding a code. The section form does not, because the
    /// page's own title already carries it, and repeating it in every section
    /// would bury the one thing that distinguishes the sections: their names.
    fn title(self, spec: &ErrorSpec) -> String {
        match self {
            Self::WholePage => format!("# {}: {}\n\n", spec.error.code, spec.error.name),
            Self::SectionOfCode => format!("## {}\n\n", spec.error.name),
        }
    }

    /// The marker for the sections inside this spec's rendering.
    fn section(self) -> &'static str {
        match self {
            Self::WholePage => "##",
            Self::SectionOfCode => "###",
        }
    }

    /// The marker one level below [`Self::section`], for numbered examples.
    fn subsection(self) -> &'static str {
        match self {
            Self::WholePage => "###",
            Self::SectionOfCode => "####",
        }
    }
}

/// The published page for one code, from EVERY spec that claims it.
///
/// # Why this takes the code's specs rather than a spec
///
/// It used to take one `&ErrorSpec`, chosen by a `BTreeMap` keyed on the code,
/// which meant twelve specs across eleven codes were loaded and then silently
/// discarded. See [`crate::spec::by_code`] for why there is no winner to pick
/// and why several specs under one code is a legitimate state.
#[must_use]
pub fn generate_error_page(code: &SpecErrorCode, specs: &CodeSpecs) -> String {
    match specs.view() {
        // Byte-identical to what this module has always emitted, so the eleven
        // pages that change are exactly the ones that were wrong.
        CodeSpecsView::Sole(spec) => render_spec(spec, SpecPlacement::WholePage),
        CodeSpecsView::Several { first, rest } => {
            let mut output = format!("# {code}\n\n");
            output.push_str(
                "> Several rules are reported under this code. Each section below is a\n\
                 > separate specification with its own status.\n\n",
            );
            for spec in std::iter::once(first).chain(rest) {
                output.push_str(&render_spec(spec, SpecPlacement::SectionOfCode));
            }
            output
        }
    }
}

/// One spec's rendering: title, status, metadata, description, examples, rule.
///
/// Takes the whole spec. It used to take the definition plus three loose
/// metadata values, on the stated grounds that "`ErrorDefinition` does not
/// carry category-level metadata" -- true while a spec held a `Vec` of
/// definitions, and false since it holds one. The signature was reporting that
/// design problem, and a previous diff had answered it by ADDING a fourth
/// argument rather than noticing.
fn render_spec(spec: &ErrorSpec, placement: SpecPlacement) -> String {
    let error = &spec.error;
    let status = spec.metadata.status;
    let kind = spec.metadata.kind;
    let mut output = String::new();
    let badge = status_badge(status);

    output.push_str(&placement.title(spec));

    // Status callout (blockquote) placed first so it is the first operational
    // fact the reader sees. The separator belongs to the callout, not to the
    // badge, so a status with no callout does not publish one.
    match status_callout(status) {
        Some(callout) => output.push_str(&format!("> {badge}; {callout}\n\n")),
        None => output.push_str(&format!("> {badge}\n\n")),
    }

    // KIND, NOT SEVERITY. A spec cannot state a severity, because severity is
    // not a property of a code: `talkbank_model`'s
    // `diagnostic_kind::severity(kind, profile)` computes it, and the same
    // finding is `Error` under Strict, `Warning` under Editor and nothing at
    // all under Lint. The old `**Severity**: {}` line published one profile's
    // answer as though it were absolute, and for 236 of 238 specs the value it
    // published had been invented by `unwrap_or_else`, not declared. Kind IS
    // static, IS required of every spec, and is the input that severity is
    // derived FROM.
    output.push_str(&format!("**Kind**: {kind}\n\n"));
    // Where in a transcript the fault occurs: the distinct set of the
    // examples' levels, a fact about examples since the Phase 2 move. A spec
    // with no examples has nothing to say, so the line is omitted rather
    // than published blank.
    if let Some(level) = spec.levels().rendered() {
        output.push_str(&format!("**Level**: {level}\n\n"));
    }
    output.push_str(&format!("**Status**: {}\n\n", badge));

    // Description
    output.push_str(&format!("{} Description\n\n", placement.section()));
    output.push_str(&format!("{}\n\n", spec.metadata.description.full()));

    // Examples
    if !error.examples.is_empty() {
        output.push_str(&format!("{} Examples\n\n", placement.section()));
        for (i, example) in error.examples.iter().enumerate() {
            output.push_str(&format!("{} Example {}\n\n", placement.subsection(), i + 1));
            output.push_str("```chat\n");
            output.push_str(&example.input);
            output.push_str("\n```\n\n");
            // NO `**Error**:` line: it printed a field no spec file can
            // declare. See `ErrorDefinition::chat_rule` for the whole story.
        }
    }

    // What CHAT requires, which is what a maintainer needs in order to fix the
    // file. Published under the spec's OWN heading rather than renamed to "How
    // to Fix": for some specs it is a rule statement and for others a pointer
    // to the CHAT manual, and calling all of them remediation would overclaim.
    // A spec declaring no such section gets no section, rather than an empty
    // one. Why the section this replaced was always empty is recorded once, on
    // `ErrorDefinition::chat_rule`.
    if let Some(rule) = &error.chat_rule {
        output.push_str(&format!("{} CHAT Rule\n\n", placement.section()));
        output.push_str(&format!("{rule}\n\n"));
    }

    output
}

/// Generate the index page: ONE table, one row per SPEC.
///
/// # Why a flat table and not sections
///
/// This grouped by nothing despite a `// Group by category` comment: it emitted
/// one `##` heading per SPEC, so 236 specs produced 236 headings, 31 of them
/// exact duplicates, each over a table with exactly one row. `## internal (E0x)`
/// appeared twice in a row.
///
/// A reader arrives here holding a code out of a diagnostic and wants that
/// code's page, which is a lookup rather than a browse. So: one table, sorted by
/// code, carrying `Name`, `Kind`, `Level` and `Status` as columns. That also
/// stops the document's STRUCTURE encoding a taxonomy: the sections were built
/// from a free-text `Category` whose values had never been normalised, so
/// `header_validation` and `Header validation` were two places to look for one
/// concept. `Category` was deleted outright on 2026-08-19, being a published
/// grouping no generation decision read; the flat table predates that and is
/// what made the deletion cheap.
///
/// # One row per SPEC, not per code
///
/// Eleven codes are claimed by more than one spec. A row per CODE could only be
/// produced by discarding the others, which is exactly what this index used to
/// do: twelve specs appeared in no row and on no page.
///
/// When measured pre-Phase-2 (file-level `Level`, `Layer` still a field),
/// seven of the eleven differed in one of those, so no single row could
/// describe them together: E519 covers header-level rules AND an
/// utterance-level rule. Since the Phase 2 move the grouping compares the
/// DERIVED set of each spec's example levels (`by_code.rs`), and this
/// renderer just publishes what the grouping hands it. Specs a code cannot
/// distinguish are published anyway (E202's two rows are byte identical),
/// because silently collapsing them is the defect this whole grouping exists
/// to prevent. R8 deletes the stubs.
///
/// So a contested code contributes several adjacent rows with the same code and
/// the same link, distinguished by `Name` and `Level`.
///
/// Takes the SAME grouping `build_error_docs` generates the pages from, so a row
/// cannot describe one spec and link to a page built from another. This used to
/// build its own map and rely on a comment saying the two rules matched.
pub fn generate_error_index(by_code: &SpecsByCode) -> String {
    let mut output = String::new();
    output.push_str("# CHAT Error Reference\n\n");
    output.push_str("Every error and warning code, in code order. Follow a code for its\n");
    output.push_str("description, its examples, and the CHAT rule it enforces.\n\n");
    output.push_str(LEGEND);

    output.push_str("| Code | Name | Kind | Level | Status |\n");
    output.push_str("|------|------|------|-------|--------|\n");
    for spec in by_code.specs() {
        // A no-example spec has no fault sites to name; the blank cell IS
        // that fact. The explicit match (not `unwrap_or_default`) keeps the
        // presentation decision visible, per the no-silent-defaults rule.
        #[expect(
            clippy::manual_unwrap_or_default,
            reason = "an empty cell is a stated presentation decision, not a default"
        )]
        let level_cell = match spec.levels().rendered() {
            Some(levels) => levels,
            None => String::new(),
        };
        output.push_str(&format!(
            "| [{}]({}.md) | {} | {} | {} | {} |\n",
            spec.error.code,
            spec.error.code,
            spec.error.name,
            spec.metadata.kind,
            level_cell,
            status_icon(spec.metadata.status),
        ));
    }
    output.push('\n');

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::error::*;

    /// Build a whole spec, because the renderer takes a whole spec.
    ///
    /// These tests used to hand-assemble an `ErrorDefinition` plus three loose
    /// metadata values to satisfy a four-argument signature. That is how the
    /// EMPTY `## How to Fix` section survived for as long as it did: the
    /// fixtures filled in a `suggestion` the loader never set, so the tests
    /// could only see a page production does not generate.
    fn spec(code: &str, name: &str, status: Status, chat_rule: Option<&str>) -> ErrorSpec {
        ErrorSpec {
            metadata: ErrorMetadata {
                description: "Word contains illegal untranscribed marker"
                    .parse()
                    .expect("a non-empty description parses"),
                status,
                kind: ErrorKind::Invalidity,
            },
            error: ErrorDefinition {
                code: code.parse().expect("a well-formed code"),
                name: name.to_string(),
                chat_rule: chat_rule.map(str::to_string),
                // One example, because `level` is a fact about examples since
                // the Phase 2 move: a page's Level line renders their distinct
                // set and is omitted when there are none.
                examples: vec![ErrorExample {
                    input: "@UTF8\n@Begin\nxx .\n@End".to_string(),
                    level: "word".parse().expect("a non-empty level parses"),
                    claim: talkbank_spec_vocabulary::frontmatter::Claim::Violates,
                    source: None,
                }],
            },
            source_path: std::path::PathBuf::from("spec/errors/test.md"),
        }
    }

    /// Render a page the way production does: group, then ask for the entry.
    ///
    /// Goes through `SpecsByCode::group` rather than building a `CodeSpecs`
    /// directly, so these tests exercise the only constructor there is. A
    /// `CodeSpecs::sole` helper added for the tests would be a second route to
    /// the type, and a proof type is only as strong as its weakest constructor.
    fn page_for(specs: Vec<ErrorSpec>) -> String {
        let grouped = SpecsByCode::group(specs);
        match grouped.codes().next() {
            Some((code, code_specs)) => generate_error_page(code, code_specs),
            None => String::from("<no specs grouped>"),
        }
    }

    /// Active-status pages should advertise themselves as enforced, and state
    /// the spec's declared KIND.
    ///
    /// This asserted `**Severity**: error` until 2026-08-15, pinning a value
    /// that was fabricated for all but two specs and that no spec can state at
    /// all. The reasoning is at the `**Kind**` line in `generate_error_page`,
    /// where the rule lives.
    #[test]
    fn test_generate_error_page_active() {
        let output = page_for(vec![spec(
            "E241",
            "IllegalUntranscribed",
            Status::Implemented,
            Some("Untranscribed speech must be marked 'xxx'."),
        )]);
        assert!(output.contains("# E241"));
        assert!(output.contains("IllegalUntranscribed"));
        assert!(output.contains("**Kind**: Invalidity"));
        assert!(output.contains("**Level**: word"));
        assert!(output.contains("**Status**: ✅ Active"));
        assert!(output.contains("> ✅ Active; This check is active in the validator."));
        assert!(output.contains("## CHAT Rule\n\nUntranscribed speech must be marked 'xxx'."));
    }

    /// Not-implemented specs should be clearly marked as planned so readers
    /// do not expect runtime enforcement.
    #[test]
    fn test_generate_error_page_planned() {
        let output = page_for(vec![spec(
            "E321",
            "SomePlannedCheck",
            Status::NotImplemented,
            None,
        )]);
        assert!(output.contains("**Status**: ⏳ Planned"));
        assert!(output.contains("> ⏳ Planned; "));
        assert!(output.contains("not yet enforced by the validator"));
        // A spec declaring no `## CHAT Rule` publishes no such section, rather
        // than an empty one under a heading that promises content.
        assert!(!output.contains("## CHAT Rule"));
    }

    /// Several specs under one code: EVERY one of them reaches the page.
    ///
    /// This is the case the previous `BTreeMap<&str, &ErrorSpec>` could not
    /// express, so the second spec was dropped and no test could have caught
    /// it: the fixture would have had to contain two specs for one code, and
    /// the type being tested took one spec.
    ///
    /// It is a POLICY test, not an invariant a type could hold: "publish every
    /// spec, as sections, under the code's own title" is a presentation choice
    /// with real alternatives (a page each under distinct names, say).
    #[test]
    fn several_specs_for_one_code_all_reach_the_page() {
        let output = page_for(vec![
            spec("E519", "L1 language code", Status::Implemented, None),
            spec(
                "E519",
                "Word-level language code",
                Status::NotImplemented,
                None,
            ),
        ]);

        // The page is titled by the CODE, since no single spec owns it.
        assert!(
            output.starts_with("# E519\n"),
            "page should be titled by the code, got: {output}"
        );
        assert!(output.contains("Several rules are reported under this code"));

        // Both names present, each as its own section, and each keeping its own
        // status rather than inheriting the first spec's.
        assert!(output.contains("## L1 language code"));
        assert!(output.contains("## Word-level language code"));
        assert!(output.contains("**Status**: ✅ Active"));
        assert!(output.contains("**Status**: ⏳ Planned"));

        // Headings demote, so the sections nest under their spec rather than
        // reading as siblings of it.
        assert!(output.contains("### Description"));
        assert!(!output.contains("\n## Description"));
    }

    /// The index gives a contested code one row PER SPEC, not one row.
    #[test]
    fn index_lists_every_spec_of_a_contested_code() {
        let grouped = SpecsByCode::group(vec![
            spec("E519", "L1 language code", Status::Implemented, None),
            spec(
                "E519",
                "Word-level language code",
                Status::NotImplemented,
                None,
            ),
        ]);
        let index = generate_error_index(&grouped);

        assert!(index.contains("| L1 language code |"));
        assert!(index.contains("| Word-level language code |"));
        // Both rows link to the one page the code has.
        assert_eq!(index.matches("[E519](E519.md)").count(), 2);
    }
}
