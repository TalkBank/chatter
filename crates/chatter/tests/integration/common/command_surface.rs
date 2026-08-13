//! Shared release-facing manifest for the published `chatter` command surface.

#![allow(dead_code)]

/// Help scope for one command-surface group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceScope {
    /// Commands listed by `chatter --help`.
    TopLevel,
}

/// Functional family for one command-surface group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFamily {
    /// Validation and watch flows.
    Validation,
    /// Normalize and inspect commands over one CHAT file.
    Formatting,
    /// JSON or alignment conversion/inspection commands.
    Conversion,
    /// Cache maintenance flows.
    Cache,
    /// Schema-printing surface.
    Schema,
    /// Self-update / maintenance commands.
    Maintenance,
}

/// Release-readiness coverage expectations for one surface family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageExpectation {
    /// The command must appear in CLI help.
    HelpContract,
    /// The command family wants systematic option/argument matrix coverage.
    OptionMatrix,
    /// The command family has human-readable or structured output contracts.
    OutputContract,
    /// The command family depends on cache, watch, path, or other runtime state.
    StatefulPath,
}

/// One reviewed command-surface family entry.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceGroup {
    /// Whether the commands live at top level.
    pub scope: SurfaceScope,
    /// Functional family for this group.
    pub family: SurfaceFamily,
    /// Concrete published command names as shown in clap help.
    pub commands: &'static [&'static str],
    /// Coverage work that must exist for this group.
    pub coverage: &'static [CoverageExpectation],
    /// Brief rationale for why this grouping exists.
    pub note: &'static str,
}

const VALIDATION_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::StatefulPath,
];

const FORMATTING_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CONVERSION_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CACHE_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::StatefulPath,
];

const SCHEMA_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OutputContract,
];

const MAINTENANCE_COVERAGE: &[CoverageExpectation] = &[CoverageExpectation::HelpContract];

/// Reviewed release-facing command-surface groups.
pub const SURFACE_GROUPS: &[SurfaceGroup] = &[
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Validation,
        commands: &["validate", "watch"],
        coverage: VALIDATION_COVERAGE,
        note: "validation lifecycle and continuous feedback commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Formatting,
        commands: &["normalize", "clean", "new-file", "fix"],
        coverage: FORMATTING_COVERAGE,
        note: "single-file normalization, inspection, scaffold, and repair commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Conversion,
        commands: &["to-json", "from-json", "show-alignment"],
        coverage: CONVERSION_COVERAGE,
        note: "JSON conversion and alignment inspection commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Cache,
        commands: &["cache"],
        coverage: CACHE_COVERAGE,
        note: "stateful validation-cache maintenance surface",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Schema,
        commands: &["schema"],
        coverage: SCHEMA_COVERAGE,
        note: "JSON schema printing surface",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Maintenance,
        commands: &["update"],
        coverage: MAINTENANCE_COVERAGE,
        note: "in-process self-update against GitHub Releases (experimental)",
    },
];

/// Top-level commands that exist in `chatter --help` but are deliberately NOT
/// part of the release-facing manifest above, each with the reason.
///
/// This list exists because [`SURFACE_GROUPS`] alone could only ever be checked
/// in ONE direction. `top_level_help_lists_all_manifested_commands` asserts that
/// every manifested command reaches help, which catches a REMOVAL; nothing
/// caught an ADDITION, so a new top-level command could join the CLI without
/// appearing in any manifest, any coverage expectation, or any documentation,
/// and every gate stayed green. That is how `update` came to be absent from the
/// CLI reference page while the binary had shipped it.
///
/// Naming a command here is a deliberate act with a stated reason, so the
/// accounting is complete: every command is either published (and carries
/// coverage expectations) or explicitly excluded (and says why). Both
/// directions are asserted, so this list cannot rot either: an entry naming a
/// command that no longer exists fails just as loudly as an unaccounted-for
/// command.
pub const UNPUBLISHED_TOP_LEVEL: &[(&str, &str)] = &[
    ("adjudicate", "experimental: merge-conflict adjudication"),
    ("batch", "experimental: batch orchestration"),
    ("debug", "maintainer diagnostics, not a user-facing surface"),
    ("merge", "experimental: transcript reconciliation"),
    ("pipeline", "experimental: multi-stage pipeline driver"),
    ("rediarize", "experimental: speaker re-diarization"),
    ("sanity-scan", "experimental: corpus-wide sanity sweep"),
    ("speaker-id", "experimental: speaker identification"),
];

/// Look up the reviewed surface-group metadata for one published family.
pub fn surface_group(family: SurfaceFamily) -> &'static SurfaceGroup {
    SURFACE_GROUPS
        .iter()
        .find(|group| group.family == family)
        .expect("surface family should exist in the shared command manifest")
}
