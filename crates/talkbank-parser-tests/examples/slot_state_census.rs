//! The slot-state census: which states has each typed position ever been
//! seen in, over an admitted CHAT population?
//!
//! The hand-written parser carries a recovery arm for every state its
//! position's type admits. Coverage says which arms no test reaches; this
//! census says whether any CHAT file in the repository (the reference corpus,
//! the error corpus with one fixture per spec example, the parser suite, the
//! parity fixtures) has ever put that position into that state. A required
//! position seen only `Present` across all of them has recovery arms no input
//! here has reached, which is the evidence a verdict on them starts from:
//! construct the input (a spec example) if the grammar can produce the state,
//! or narrow the type if it cannot.
//!
//! ```text
//! cargo run -p talkbank-parser-tests --example slot_state_census [-- <dir>...]
//! ```
//!
//! With no directories, admit the reference and error corpora. Explicit roots
//! override that finite population. Discovery, source reads and tree production
//! must all succeed before a census is printed; no files are silently skipped.
//! Output is Markdown: one table over every position, then two action lists.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::conformance::{
    Container, Observation, Observed, Position, dispatch, walk_all,
};

/// Per position: per observed state, how many visits and one file that
/// showed it.
type Census = BTreeMap<(&'static str, Position, Container), BTreeMap<Observed, (usize, String)>>;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let dirs = if args.is_empty() {
        vec![
            root.join("corpus/reference"),
            root.join("crates/talkbank-parser-tests/tests/error_corpus"),
        ]
    } else {
        args
    };
    let corpora = dirs
        .iter()
        .map(|dir| ChatCorpus::read(dir))
        .collect::<Result<Vec<_>, _>>()?;
    let fixtures: BTreeMap<_, _> = corpora
        .iter()
        .flat_map(ChatCorpus::fixtures)
        .map(|fixture| (fixture.path(), fixture))
        .collect();

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang)?;

    let mut census: Census = BTreeMap::new();
    let mut population = Sha256::new();
    for (path, fixture) in &fixtures {
        let tree = parser.parse(fixture.source(), None).ok_or_else(|| {
            std::io::Error::other(format!("no parse tree for {}", path.display()))
        })?;
        let relative = path.strip_prefix(&root).unwrap_or(path);
        let shown = relative
            .to_str()
            .ok_or_else(|| std::io::Error::other("census fixture path is not UTF-8"))?;
        // Sorted path, NUL delimiter, fixed-width source digest. Bind the
        // observation to the exact admitted population for later reuse.
        population.update(shown.as_bytes());
        population.update([0]);
        population.update(Sha256::digest(fixture.source().as_bytes()));
        walk_all(tree.root_node(), &mut |node| {
            let mut raw: Vec<Observation> = Vec::new();
            dispatch(node, &mut raw);
            for o in raw {
                let per_state = census
                    .entry((o.rule_kind, o.position, o.container))
                    .or_default();
                let cell = per_state
                    .entry(o.observed)
                    .or_insert_with(|| (0, shown.to_owned()));
                cell.0 += 1;
            }
        });
    }

    println!("# Slot-state census\n");
    let population_sha256: String = population
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    println!("Source population SHA-256: `{population_sha256}`\n");
    println!(
        "{} admitted files parsed under {} root(s); {} positions observed.\n",
        fixtures.len(),
        dirs.len(),
        census.len()
    );
    println!(
        "| rule | carrier.field | container | Present | Missing | Error | Unexpected | Absent | Empty | first non-Present example |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    let states = [
        Observed::Present,
        Observed::Missing,
        Observed::Error,
        Observed::Unexpected,
        Observed::Absent,
        Observed::Empty,
    ];
    for ((rule, slot, container), per_state) in &census {
        let counts: Vec<String> = states
            .iter()
            .map(|s| {
                per_state
                    .get(s)
                    .map_or_else(|| ".".to_string(), |(n, _)| n.to_string())
            })
            .collect();
        let example = per_state
            .iter()
            .find(|(s, _)| **s != Observed::Present && **s != Observed::Empty)
            .map_or("", |(_, (_, f))| f.as_str());
        println!(
            "| {rule} | {slot} | {container:?} | {} | {example} |",
            counts.join(" | ")
        );
    }

    let only_present: Vec<_> = census
        .iter()
        .filter(|((_, _, c), per)| {
            *c == Container::Required && per.keys().all(|s| *s == Observed::Present)
        })
        .map(|((r, s, _), _)| format!("`{r}.{s}`"))
        .collect();
    println!(
        "\n## Required positions never seen in a recovery state ({})\n",
        only_present.len()
    );
    println!(
        "These states were not observed in this population. This is not a proof of unreachability.\n"
    );
    for p in &only_present {
        println!("- {p}");
    }
    let absent_required: Vec<_> = census
        .iter()
        .filter(|((_, _, c), per)| *c == Container::Required && per.contains_key(&Observed::Absent))
        .map(|((r, s, _), per)| format!("`{r}.{s}` (e.g. {})", per[&Observed::Absent].1))
        .collect();
    println!(
        "\n## Required positions seen `Absent` ({})\n",
        absent_required.len()
    );
    println!("Evidence on whether a required position can be empty at all.\n");
    for p in &absent_required {
        println!("- {p}");
    }
    Ok(())
}
