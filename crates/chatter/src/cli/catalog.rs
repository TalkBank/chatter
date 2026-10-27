//! Executable syntax and semantic discovery share one declaration.
//!
//! Every variant must declare either a leaf contract or a child namespace.
//! Clap supplies the names, arguments and constraints; no second argument
//! schema is maintained here.

use clap::CommandFactory;
use serde::Serialize;

macro_rules! command_tree {
    ($(#[$meta:meta])* pub enum $name:ident {
        $($(#[$variant_meta:meta])* $variant:ident $({ $($fields:tt)* })? => $shape:expr,)*
    }) => {
        $(#[$meta])*
        #[derive(clap::Subcommand)]
        pub enum $name {
            $($(#[$variant_meta])* $variant $({ $($fields)* })?,)*
        }
        impl crate::cli::catalog::Catalog for $name {
            fn shapes() -> Vec<crate::cli::catalog::Shape> {
                vec![$($shape,)*]
            }
        }
    };
}
pub(crate) use command_tree;

pub(crate) trait Catalog {
    fn shapes() -> Vec<Shape>;
}

pub(crate) enum Shape {
    Leaf(Contract),
    Namespace(fn() -> Vec<Shape>),
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Contract {
    Validate,
    Fix,
    Format,
    New,
    Watch,
    Schema,
    ToJson,
    FromJson,
    Alignment,
    Words,
    Overlaps,
    Linkers,
    Checks,
    Discovery,
    CacheStats,
    CacheClear,
    Map,
    Match,
    Replay,
    Suggest,
    Rediarize,
    Scan,
    ScriptedReview,
    InteractiveReview,
    LanguageRuns,
    RetagLanguage,
    JoinRetrace,
    Redact,
    Update,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Maturity {
    General,
    Experimental,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Effects {
    ReadOnly,
    ValidationCache,
    ArtifactDestination,
    PreviewOrApply,
    SpeakerEvidenceAndArtifact,
    ProviderRequestAndPending,
    PendingRecords,
    ReviewPublication,
    ExecutableReplacement,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum StdoutProduct {
    NoResult,
    ValidationResults,
    HumanReport,
    ChatArtifact,
    JsonArtifact,
    TextOrJsonReport,
    StreamingValidation,
    SchemaOrUrl,
    Discovery,
}

#[derive(Serialize)]
struct Semantics {
    operation: Contract,
    maturity: Maturity,
    effects: Effects,
    stdout: StdoutProduct,
    prerequisites: &'static str,
    completion: &'static str,
    stderr: &'static str,
}

impl Contract {
    fn describe(self) -> Semantics {
        use Contract::*;
        use Effects as E;
        use StdoutProduct as S;
        let (effects, stdout, prerequisites) = match self {
            Validate => (
                E::ValidationCache,
                S::ValidationResults,
                "Readable CHAT inputs; --audit has a separate report destination",
            ),
            Fix | LanguageRuns | RetagLanguage | JoinRetrace => (
                E::PreviewOrApply,
                S::HumanReport,
                "Readable CHAT inputs; --apply authorizes publication; fix semantic repairs additionally require explicit --code selection",
            ),
            Format | New | FromJson | Redact => (
                E::ArtifactDestination,
                S::ChatArtifact,
                "Artifact goes to stdout unless --output names a destination",
            ),
            ToJson => (
                E::ArtifactDestination,
                S::JsonArtifact,
                "File input: stdout or --output. Directory input: --output-dir required; --prune explicitly removes orphan outputs",
            ),
            Watch => (
                E::ValidationCache,
                S::StreamingValidation,
                "Readable CHAT path; runs until interrupted",
            ),
            Schema => (E::ReadOnly, S::SchemaOrUrl, "None"),
            Alignment => (E::ReadOnly, S::HumanReport, "Readable CHAT input"),
            Words => (E::ReadOnly, S::TextOrJsonReport, "Readable CHAT input"),
            Overlaps | Linkers => (
                E::ArtifactDestination,
                S::HumanReport,
                "Readable CHAT inputs; optional new report destination",
            ),
            Checks => (E::ReadOnly, S::HumanReport, "None"),
            Discovery => (E::ReadOnly, S::Discovery, "None"),
            CacheStats => (
                E::ReadOnly,
                S::TextOrJsonReport,
                "Readable validation cache",
            ),
            CacheClear => (
                E::PreviewOrApply,
                S::HumanReport,
                "Select --all or --prefix; --apply authorizes removal",
            ),
            Map | Replay | Rediarize => (
                E::ArtifactDestination,
                S::ChatArtifact,
                "Readable CHAT and explicit mapping, recorded decision, or diarization inputs",
            ),
            Match => (
                E::SpeakerEvidenceAndArtifact,
                S::ChatArtifact,
                "Readable donor and reference; insufficient confidence exits 4; requested evidence paths may be written",
            ),
            Suggest => (
                E::ProviderRequestAndPending,
                S::HumanReport,
                "Configured LLM provider and required pending destination; never publishes a transcript",
            ),
            Scan => (
                E::PendingRecords,
                S::HumanReport,
                "Readable CHAT directory and override file; required pending destination",
            ),
            ScriptedReview => (
                E::ReviewPublication,
                S::NoResult,
                "Pending file, override destination, complete script and operator identity",
            ),
            InteractiveReview => (
                E::ReviewPublication,
                S::NoResult,
                "Terminal stdin and stderr, pending file, override destination and operator identity",
            ),
            Update => (
                E::ExecutableReplacement,
                S::HumanReport,
                "Network and a supported installer-managed executable",
            ),
        };
        let maturity = match self {
            Map | Match | Replay | Suggest | Rediarize | Scan | ScriptedReview
            | InteractiveReview | LanguageRuns | RetagLanguage | JoinRetrace | Redact | Update => {
                Maturity::Experimental
            }
            Validate | Fix | Format | New | Watch | Schema | ToJson | FromJson | Alignment
            | Words | Overlaps | Linkers | Checks | Discovery | CacheStats | CacheClear => {
                Maturity::General
            }
        };
        Semantics {
            operation: self,
            maturity,
            effects,
            stdout,
            prerequisites,
            completion: match self {
                Watch => "Streaming until interrupted; no finite success receipt",
                _ => "Wait for process exit; output alone does not establish success",
            },
            stderr: "Diagnostics, progress and logging; preserve separately from stdout",
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ValueArity {
    Bounded { minimum: usize, maximum: usize },
    Variadic { minimum: usize },
}

#[derive(Serialize)]
struct Argument {
    id: String,
    long: Option<String>,
    short: Option<char>,
    position: Option<usize>,
    required: bool,
    values: Vec<String>,
    defaults: Vec<String>,
    conflicts: Vec<String>,
    arity: ValueArity,
    help: String,
}

#[derive(Serialize)]
struct Leaf {
    path: Vec<String>,
    about: String,
    semantics: Semantics,
    arguments: Vec<Argument>,
    help: String,
}

#[derive(Serialize)]
struct Discovery {
    schema: &'static str,
    commands: Vec<Leaf>,
}

fn collect(
    command: &clap::Command,
    shapes: Vec<Shape>,
    path: &[String],
    leaves: &mut Vec<Leaf>,
) -> Result<(), String> {
    let children: Vec<_> = command
        .get_subcommands()
        .filter(|child| child.get_name() != "help")
        .collect();
    if children.len() != shapes.len() {
        return Err(format!(
            "command catalog does not match executable tree at {}",
            path.join(" ")
        ));
    }
    for (child, shape) in children.into_iter().zip(shapes) {
        let mut path = path.to_vec();
        path.push(child.get_name().to_owned());
        match shape {
            Shape::Namespace(children) => collect(child, children(), &path, leaves)?,
            Shape::Leaf(contract) => {
                let arguments = child
                    .get_arguments()
                    .filter(|arg| !arg.is_hide_set())
                    .map(|arg| {
                        let range = arg.get_num_args().unwrap_or_else(|| {
                            if arg.get_action().takes_values() {
                                1.into()
                            } else {
                                0.into()
                            }
                        });
                        Argument {
                            id: arg.get_id().to_string(),
                            long: arg.get_long().map(str::to_owned),
                            short: arg.get_short(),
                            position: arg.get_index(),
                            required: arg.is_required_set(),
                            values: arg
                                .get_possible_values()
                                .iter()
                                .map(|v| v.get_name().to_owned())
                                .collect(),
                            defaults: arg
                                .get_default_values()
                                .iter()
                                .map(|v| v.to_string_lossy().into_owned())
                                .collect(),
                            conflicts: child
                                .get_arg_conflicts_with(arg)
                                .iter()
                                .map(|v| v.get_id().to_string())
                                .collect(),
                            arity: match range.max_values() {
                                usize::MAX => ValueArity::Variadic {
                                    minimum: range.min_values(),
                                },
                                maximum => ValueArity::Bounded {
                                    minimum: range.min_values(),
                                    maximum,
                                },
                            },
                            help: arg
                                .get_long_help()
                                .or_else(|| arg.get_help())
                                .map(ToString::to_string)
                                .unwrap_or_default(),
                        }
                    })
                    .collect();
                leaves.push(Leaf {
                    path,
                    about: child
                        .get_about()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    semantics: contract.describe(),
                    arguments,
                    help: child.clone().render_long_help().to_string(),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn run(
    path: &[String],
    format: super::OutputFormat,
) -> crate::exit_codes::CommandResult {
    let mut command = super::Cli::command();
    command.build();
    let mut leaves = Vec::new();
    collect(&command, super::Commands::shapes(), &[], &mut leaves).map_err(|error| {
        eprintln!("Error building command catalog: {error}");
        crate::exit_codes::CommandStop::Reported(crate::exit_codes::ReportedFailure::Internal)
    })?;
    if !path.is_empty() {
        leaves.retain(|leaf| leaf.path.starts_with(path));
        if leaves.is_empty() {
            eprintln!("Error: unknown command path: {}", path.join(" "));
            return Err(crate::exit_codes::ReportedFailure::Precondition.into());
        }
    }
    let discovery = Discovery {
        schema: "chatter.commands.v1",
        commands: leaves,
    };
    match format {
        super::OutputFormat::Json => outln!(
            "{}",
            serde_json::to_string_pretty(&discovery).map_err(|error| {
                eprintln!("Error encoding command catalog: {error}");
                crate::exit_codes::CommandStop::Reported(
                    crate::exit_codes::ReportedFailure::Internal,
                )
            })?
        ),
        super::OutputFormat::Text => {
            for leaf in discovery.commands {
                if path.is_empty() {
                    outln!("chatter {} — {}", leaf.path.join(" "), leaf.about);
                } else {
                    outln!("{}", leaf.help);
                    outln!("Effects: {}", leaf.semantics.prerequisites);
                }
            }
        }
    }
    Ok(())
}
