//! The slot-state census: which states has each typed position ever been
//! seen in, over every CHAT file in the repository?
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
//! With no directories, every `*.cha` under the repository root except
//! `target/` and `node_modules/`. Output is Markdown on stdout: one table over
//! every position, then the two lists a reader acts on.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use talkbank_parser_tests::conformance::{Container, Observation, Observed, dispatch, walk_all};

/// Per position: per observed state, how many visits and one file that
/// showed it.
type Census =
    BTreeMap<(&'static str, &'static str, Container), BTreeMap<Observed, (usize, String)>>;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn chat_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| {
            walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    name != "target" && name != "node_modules" && name != ".git"
                })
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
                .map(walkdir::DirEntry::into_path)
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let dirs = if args.is_empty() {
        vec![root.clone()]
    } else {
        args
    };
    let files = chat_files(&dirs);

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang)?;

    let mut census: Census = BTreeMap::new();
    let mut parsed = 0usize;
    for path in &files {
        let source = std::fs::read_to_string(path)?;
        let Some(tree) = parser.parse(&source, None) else {
            eprintln!("skipped (no tree): {}", path.display());
            continue;
        };
        parsed += 1;
        let shown = path
            .strip_prefix(&root)
            .map_or_else(|_| path.display().to_string(), |p| p.display().to_string());
        walk_all(tree.root_node(), &mut |node| {
            let mut raw: Vec<Observation> = Vec::new();
            dispatch(node, &mut raw);
            for o in raw {
                let per_state = census
                    .entry((o.rule_kind, o.slot, o.container))
                    .or_default();
                let cell = per_state
                    .entry(o.observed)
                    .or_insert_with(|| (0, shown.clone()));
                cell.0 += 1;
            }
        });
    }

    println!("# Slot-state census\n");
    println!(
        "{parsed} files parsed under {} root(s); {} positions observed.\n",
        dirs.len(),
        census.len()
    );
    println!(
        "| rule | slot | container | Present | Missing | Error | Unexpected | Absent | Empty | first non-Present example |"
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
        "Their `Missing`, `Error` and `Absent` arms in the hand-written parser are reached by no file here.\n"
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
