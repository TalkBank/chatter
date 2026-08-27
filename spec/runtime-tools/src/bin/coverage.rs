//! Check coverage of construct and error specifications
//!
//! Analyzes which constructs and errors are documented.
//! For errors, cross-references against the ErrorCode enum to report
//! full coverage metrics.

use clap::Parser;
use generators::spec::error::Demonstration;
use generators::spec::{ConstructSpec, ErrorSpec, SpecsByCode};
use talkbank_model::ErrorCode;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// CLI arguments: which coverage to report, and where the specs are.
#[derive(Parser)]
#[command(name = "coverage")]
#[command(about = "Check construct and error coverage")]
#[command(group(clap::ArgGroup::new("what").required(true).multiple(true)
    .args(["constructs", "errors"])))]
struct Args {
    /// Check construct coverage
    #[arg(long, group = "what")]
    constructs: bool,

    /// Check error coverage
    #[arg(long, group = "what")]
    errors: bool,

    /// Root directory for specs
    #[arg(short, long, default_value = "spec")]
    spec_dir: PathBuf,
}

/// Reports coverage of construct and error specs, cross-referencing against the ErrorCode enum.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // No `if neither { bail }`: the arg group above makes that cell
    // unrepresentable, so clap refuses it at parse time with its own usage
    // message instead of the program discovering it after startup.
    if args.constructs {
        check_construct_coverage(&args.spec_dir.join("constructs"))?;
    }

    if args.errors {
        check_error_coverage(&args.spec_dir.join("errors"))?;
    }

    Ok(())
}

fn check_construct_coverage(dir: &PathBuf) -> anyhow::Result<()> {
    println!("=== Construct Coverage ===\n");

    let specs = ConstructSpec::load_all(dir)
        .map_err(|e| anyhow::anyhow!("Failed to load construct specs: {}", e))?;

    let mut by_level = std::collections::HashMap::new();

    for spec in &specs {
        let entry = by_level
            .entry(spec.metadata.level.clone())
            .or_insert_with(Vec::new);
        entry.push(&spec.metadata.category);
    }

    for (level, categories) in by_level {
        println!("{} ({} categories):", level, categories.len());
        for category in categories {
            println!("  - {}", category);
        }
        println!();
    }

    println!("Total: {} construct specifications\n", specs.len());

    Ok(())
}

/// Name the specs whose examples never demonstrate the rule they are FOR.
///
/// Since R2 this population is exactly the specs whose examples all claim
/// `subsumed_by`: the parser-specificity worklist, stated per spec with its
/// targets. (The gate that used to baseline it, `SpecSelfDemonstrationGate`,
/// was deleted when the required claim made an assertion-free example
/// unwritable; this report is the worklist view, not a gate.)
///
/// The targets tell an author which kind of problem they have: `subsumed_by
/// E316` means the input does not parse and the rule is unreachable today, so
/// the fix is in the parser; a specific other code usually means the fixture
/// is simply wrong.
fn report_undemonstrated<'a>(specs: impl Iterator<Item = &'a ErrorSpec>) {
    let mut rows: Vec<(String, String)> = Vec::new();
    for spec in specs {
        if let Demonstration::Absent { declared } = spec.demonstration() {
            let codes: Vec<String> = declared.iter().map(ToString::to_string).collect();
            rows.push((spec.source_file().to_owned(), codes.join(", ")));
        }
    }
    if rows.is_empty() {
        return;
    }

    println!(
        "Specs demonstrating nothing about their own code ({}), with what they \
         assert instead:",
        rows.len()
    );
    for (file, declared) in &rows {
        println!("  {file:46} declares {declared}");
    }
    println!();
}

/// Name the specs under one code that agree on `Level` and `Layer`.
///
/// A POINTER for an author, not a verdict: this called itself the residue queue
/// until the entries were run, and it was wrong about half of them. What it
/// reports is worth looking at; what it MEANS is decided by validating each
/// spec's examples and comparing the diagnostics.
fn report_residue(grouped: &SpecsByCode) {
    let residue = grouped.indistinguishable();
    if residue.is_empty() {
        return;
    }

    println!(
        "Specs agreeing on Level and Layer under one code ({}). SUGGESTIVE, \
         not a verdict: validate each spec's examples and compare the \
         diagnostics to see what they actually trigger.",
        residue.len()
    );
    for group in &residue {
        // Rendered by the type, so this text and the gate's failure message
        // cannot drift.
        println!("  {group}");
    }
    println!();
}

fn check_error_coverage(dir: &PathBuf) -> anyhow::Result<()> {
    println!("=== Error Coverage ===\n");

    // The registry is a property of the CHECKOUT: `--spec-dir` chooses which
    // spec tree to read, not which codes exist.
    let root = generators::repo_paths::RepoRoot::resolve(None)?;
    let registry = talkbank_spec_vocabulary::registry::CodeRegistry::load(root.as_path())?;
    let specs = ErrorSpec::load_all(dir, &registry)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    // Grouped ONCE, and this is the owner afterwards. A plain `code -> spec`
    // map over the loaded vector silently kept whichever contested spec sorted
    // last, which is the defect `SpecsByCode`'s module doc exists to explain.
    let grouped = SpecsByCode::group(specs);

    // Reported before anything else, because these are the only sections
    // naming WORK rather than counting coverage.
    report_undemonstrated(grouped.specs());
    report_residue(&grouped);

    // What each CODE's specs establish between them. Keyed by `String` because
    // this map is JOINED with `enum_codes`, which is built from
    // by REGEX-SCRAPING `error_code.rs`'s source text. So the other side of
    // this join is matched text, not a typed vocabulary, and there is nothing
    // to parse it into that would mean anything. (`spec-runtime-tools`'
    // artifacts.rs has a similar `String` key for a different reason: a real
    // cross-workspace enum join.)
    let mut spec_codes: BTreeMap<String, SpecCoverage> = BTreeMap::new();
    for (code, code_specs) in grouped.codes() {
        // A code is covered by an example if ANY of its specs carries one.
        // Stated, rather than arrived at by whichever file the map overwrote
        // last.
        let coverage = code_specs
            .iter()
            .map(|spec| SpecCoverage::of(&spec.demonstration()))
            .fold(SpecCoverage::Stub, SpecCoverage::either);
        spec_codes.insert(code.to_string(), coverage);
    }

    // `ErrorCode::iter()` IS the declaration, so there is no path, no regex,
    // no "enum file not found" mode, and no basis to announce.
    //
    // This binary lived in `spec/tools`, which deliberately cannot see the live
    // model, so it scraped `error_code.rs` with a regex and degraded to a
    // "spec-only" mode when the file was missing. That mode announced a
    // denominator it never computed: with the map empty, every code failed the
    // lookup below and continued, so the report read `0/0 (0.0%)` beneath a
    // sentence claiming it measured the specs. `error_code_specs.rs` had already
    // deleted exactly this shape and says why: using the type deletes the path,
    // the regex, the guard, and the failure mode.
    let enum_codes: BTreeMap<String, String> = ErrorCode::iter()
        .map(|code| (code.as_str().to_string(), format!("{code:?}")))
        .collect();

    // Cross-reference
    let all_codes: BTreeSet<String> = enum_codes
        .keys()
        .chain(spec_codes.keys())
        .cloned()
        .collect();

    let mut with_spec = 0;
    let mut with_example = 0;
    let mut missing: Vec<(String, String)> = Vec::new();
    let mut extra: Vec<String> = Vec::new(); // in specs but not in enum

    // Group by the code's BAND (E2xx, E5xx), which is a numeric prefix and
    // was never the deleted `Category` metadata field.
    let mut by_band: BTreeMap<String, BandStats> = BTreeMap::new();

    for code in &all_codes {
        // ONE lookup, and the variant carries the name. Testing `contains_key`
        // and then `get(..).cloned().unwrap_or_default()` searched the map
        // twice and ended in a fabricated empty name the guard made
        // unreachable, which is the silent-default shape with a live guard
        // hiding it.
        let Some(variant) = enum_codes.get(code) else {
            if spec_codes.contains_key(code) {
                extra.push(code.clone());
            }
            continue;
        };

        // Both arms below count the code, so the bookkeeping common to them
        // is hoisted: it used to be three identical lines in each branch.
        let stats = by_band.entry(band_of(code)).or_default();
        stats.total += 1;

        match spec_codes.get(code) {
            Some(SpecCoverage::Exampled) => {
                with_spec += 1;
                with_example += 1;
                stats.with_spec += 1;
                stats.with_example += 1;
            }
            Some(SpecCoverage::Stub) => {
                with_spec += 1;
                stats.with_spec += 1;
            }
            None => missing.push((code.clone(), variant.clone())),
        }
    }

    let total_enum = enum_codes.len();

    // Print the band breakdown.
    println!("Code band breakdown:");
    println!(
        "{:<25} {:>5} {:>5} {:>8} {:>8}",
        "Code band", "Total", "Specs", "Examples", "Stubs"
    );
    println!("{}", "-".repeat(56));
    for (band, stats) in &by_band {
        println!(
            "{:<25} {:>5} {:>5} {:>8} {:>8}",
            band,
            stats.total,
            stats.with_spec,
            stats.with_example,
            stats.with_spec - stats.with_example,
        );
    }
    println!("{}", "-".repeat(56));
    println!(
        "{:<25} {:>5} {:>5} {:>8} {:>8}",
        "TOTAL",
        total_enum,
        with_spec,
        with_example,
        with_spec - with_example,
    );
    println!();

    // Print coverage percentage
    let pct = if total_enum > 0 {
        (with_spec as f64 / total_enum as f64) * 100.0
    } else {
        0.0
    };
    println!("Coverage: {with_spec}/{total_enum} ({pct:.1}%) of the ErrorCode enum");
    println!("  With CHAT examples: {}", with_example);
    // Derived, not counted. Every per-category row already printed
    // `with_spec - with_example` while the TOTAL row printed a separately
    // incremented `stubs`: two representations of one number, and the
    // hand-maintained one is always the one that drifts.
    println!("  Stub specs (no example): {}", with_spec - with_example);
    println!();

    // Print missing codes
    if !missing.is_empty() {
        println!("Missing specs ({}):", missing.len());
        for (code, variant) in &missing {
            println!("  {}, {}", code, variant);
        }
        println!();
    }

    // Print extra codes (in specs but not in enum)
    if !extra.is_empty() {
        println!("Extra specs (not in enum): {:?}", extra);
        println!();
    }

    Ok(())
}

fn band_of(code: &str) -> String {
    if code.starts_with('W') {
        return "Warnings (Wxxx)".to_string();
    }
    let prefix = &code[1..2];
    match prefix {
        "0" | "1" => "Internal (E0xx/E1xx)".to_string(),
        "2" => "Word errors (E2xx)".to_string(),
        "3" => "Parser errors (E3xx)".to_string(),
        "4" => "Dep. tier (E4xx)".to_string(),
        "5" => "Header errors (E5xx)".to_string(),
        "6" => "Tier errors (E6xx)".to_string(),
        "7" => "Temporal/media (E7xx)".to_string(),
        "9" => "Unknown (E9xx)".to_string(),
        _ => format!("Other ({}xx)", prefix),
    }
}

#[derive(Default)]
struct BandStats {
    total: usize,
    with_spec: usize,
    with_example: usize,
}

/// What a loaded spec contributes to coverage: an example, or nothing yet.
///
/// A stub and an exampled spec are different outcomes the report counts
/// separately, so they are variants and the counting is a `match` a new outcome
/// would break, rather than a `bool` two call sites test independently.
#[derive(Debug, Clone, Copy)]
enum SpecCoverage {
    /// The spec carries at least one example.
    Exampled,
    /// The spec exists but has no example yet.
    Stub,
}

impl SpecCoverage {
    /// The better of two coverages, for folding a code's several specs.
    ///
    /// `Exampled` wins: a code is exampled if any spec claiming it carries an
    /// example. Written as a `match` over the pair so a third variant cannot
    /// be added without deciding where it sits.
    fn either(self, other: Self) -> Self {
        match (self, other) {
            (Self::Stub, Self::Stub) => Self::Stub,
            (Self::Exampled, _) | (_, Self::Exampled) => Self::Exampled,
        }
    }

    /// Derived from the spec's own classification, never from a `bool`.
    ///
    /// This took `examples.is_empty()` until 2026-08-20, which was a third
    /// hand-rolled spelling of what `Demonstration` names. `of(!is_empty())`
    /// also type-checked and would have inverted every count in the report
    /// with nothing to notice.
    fn of(demonstration: &Demonstration) -> Self {
        match demonstration {
            Demonstration::NoExamples => Self::Stub,
            Demonstration::ByExample | Demonstration::Absent { .. } => Self::Exampled,
        }
    }
}
