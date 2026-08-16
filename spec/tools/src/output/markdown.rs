//! # Markdown Documentation Generator
//!
//! Generates publishable error documentation in Markdown format.
//!
//! Each generated page surfaces the implementation status from the source
//! spec (`spec/errors/*.md`) as a visible badge so researchers can tell at a
//! glance whether the validator actually enforces the documented check.

use crate::spec::error::{ErrorDefinition, ErrorKind, ErrorSpec};
use crate::spec::metadata::Status;

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

/// Render the callout that explains the badge meaning to readers.
///
/// Lives immediately under the title so researchers scanning the docs see
/// the enforcement state before any other metadata.
fn status_callout(status: Status) -> &'static str {
    match status {
        Status::Implemented => "This check is active in the validator.\n\n",
        Status::NotImplemented => {
            "This check is documented but not yet enforced by the validator. The error code will not fire until implementation is complete.\n\n"
        }
        // Both produced the bare separator under the old `_` arm, and both keep
        // it. Written out so that giving either a real callout is a choice
        // rather than an edit to a catch-all.
        Status::Deprecated | Status::UnreachableFromChat => "\n\n",
    }
}

/// Generate a Markdown page for a single error.
///
/// `status` and `kind` come from the owning `ErrorSpec`'s metadata (see
/// [`crate::spec::error::ErrorMetadata`]). They are passed explicitly because
/// `ErrorDefinition` does not carry category-level metadata.
pub fn generate_error_page(error: &ErrorDefinition, status: Status, kind: ErrorKind) -> String {
    let mut output = String::new();
    let badge = status_badge(status);

    // Title
    output.push_str(&format!("# {}: {}\n\n", error.code, error.name));

    // Status callout (blockquote) placed first so it is the first operational
    // fact the reader sees.
    output.push_str(&format!("> {}; ", badge));
    output.push_str(status_callout(status));

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
    output.push_str(&format!("**Status**: {}\n\n", badge));

    // Description
    output.push_str("## Description\n\n");
    output.push_str(&format!("{}\n\n", error.description));

    // Examples
    if !error.examples.is_empty() {
        output.push_str("## Examples\n\n");
        for (i, example) in error.examples.iter().enumerate() {
            output.push_str(&format!("### Example {}\n\n", i + 1));
            output.push_str("```chat\n");
            output.push_str(&example.input);
            output.push_str("\n```\n\n");
            output.push_str(&format!("**Error**: {}\n\n", example.expected_message));
        }
    }

    // How to fix
    output.push_str("## How to Fix\n\n");
    output.push_str(&format!("{}\n\n", error.suggestion));

    // Help URL
    if let Some(url) = &error.help_url {
        output.push_str("## More Information\n\n");
        output.push_str(&format!("[CHAT Manual]({})\n\n", url));
    }

    output
}

/// Generate index page for all errors.
///
/// The Status column uses a compact icon (✅ / ⏳) so category tables stay
/// readable. The per-page view spells out the full badge.
pub fn generate_error_index(specs: &[ErrorSpec]) -> String {
    let mut output = String::new();

    output.push_str("# CHAT Error Reference\n\n");
    output.push_str("Complete reference for all CHAT parser and validation errors.\n\n");
    output.push_str(
        "Status legend: ✅ = active in the validator, ⏳ = documented but not yet enforced.\n\n",
    );

    // Group by category
    for spec in specs {
        // Derived from the spec's own code rather than a stored copy. A spec
        // with no definition renders no heading at all rather than an invented
        // `## Category ()`: `ErrorSpec` is built with exactly one definition
        // per file, so this skips nothing that exists. The right cure is
        // `error: ErrorDefinition` instead of a `Vec`, which would delete this
        // and roughly eight nested loops across both crates.
        let Some(first) = spec.errors.first() else {
            continue;
        };
        output.push_str(&format!(
            "## {} ({})\n\n",
            spec.metadata.category,
            first.code.hundred_block()
        ));
        output.push_str(&format!("{}\n\n", spec.metadata.description));

        output.push_str("| Code | Name | Kind | Status |\n");
        output.push_str("|------|------|----------|--------|\n");

        // Status is attached to the spec (category), not each ErrorDefinition,
        // so all errors in a spec share the same icon.
        let status_icon = match spec.metadata.status {
            Status::Implemented => "✅",
            Status::NotImplemented => "⏳",
            // Both fell to `_ => "?"` before and still do.
            Status::Deprecated | Status::UnreachableFromChat => "?",
        };

        for error in &spec.errors {
            output.push_str(&format!(
                "| [{}]({}.md) | {} | {} | {} |\n",
                error.code, error.code, error.name, spec.metadata.kind, status_icon
            ));
        }

        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::error::*;

    /// Active-status pages should advertise themselves as enforced, and state
    /// the spec's declared KIND.
    ///
    /// This asserted `**Severity**: error` until 2026-08-15, pinning a value
    /// that was fabricated for all but two specs and that no spec can state at
    /// all. The reasoning is at the `**Kind**` line in `generate_error_page`,
    /// where the rule lives.
    #[test]
    fn test_generate_error_page_active() {
        let error = ErrorDefinition {
            code: "E241".parse().expect("E241 is a well-formed code"),
            name: "IllegalUntranscribed".to_string(),
            description: "Word contains illegal untranscribed marker".to_string(),
            suggestion: "Use 'xxx' for unintelligible speech".to_string(),
            help_url: Some("https://talkbank.org/errors/E241".to_string()),
            references: ErrorReference::default(),
            examples: vec![],
        };

        let output = generate_error_page(&error, Status::Implemented, ErrorKind::Invalidity);
        assert!(output.contains("# E241"));
        assert!(output.contains("IllegalUntranscribed"));
        assert!(output.contains("**Kind**: Invalidity"));
        assert!(output.contains("**Status**: ✅ Active"));
        assert!(output.contains("> ✅ Active; This check is active in the validator."));
    }

    /// Not-implemented specs should be clearly marked as planned so readers
    /// do not expect runtime enforcement.
    #[test]
    fn test_generate_error_page_planned() {
        let error = ErrorDefinition {
            code: "E321".parse().expect("E321 is a well-formed code"),
            name: "SomePlannedCheck".to_string(),
            description: "Planned check".to_string(),
            suggestion: "TBD".to_string(),
            help_url: None,
            references: ErrorReference::default(),
            examples: vec![],
        };

        let output = generate_error_page(&error, Status::NotImplemented, ErrorKind::Invalidity);
        assert!(output.contains("**Status**: ⏳ Planned"));
        assert!(output.contains("> ⏳ Planned; "));
        assert!(output.contains("not yet enforced by the validator"));
    }
}
