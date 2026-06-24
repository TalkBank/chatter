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
    /// Validation, watch, and lint flows.
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
        commands: &["validate", "watch", "lint"],
        coverage: VALIDATION_COVERAGE,
        note: "validation lifecycle and continuous feedback commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Formatting,
        commands: &["normalize", "clean", "new-file"],
        coverage: FORMATTING_COVERAGE,
        note: "single-file normalization, inspection, and scaffold commands",
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
        note: "self-update launcher that runs the bundled chatter-update program",
    },
];

/// Look up the reviewed surface-group metadata for one published family.
pub fn surface_group(family: SurfaceFamily) -> &'static SurfaceGroup {
    SURFACE_GROUPS
        .iter()
        .find(|group| group.family == family)
        .expect("surface family should exist in the shared command manifest")
}
