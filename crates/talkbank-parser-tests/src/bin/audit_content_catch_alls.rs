//! List the modules still carrying a content-enum `_ =>` catch-all.
//!
//! A renderer, nothing else. The rule, the ratchet list and every type live in
//! `talkbank_parser_tests::content_catch_alls`; the GATE that fails CI is
//! `tests/integration/gates.rs`, which runs the same `Audit` this prints.
//! This binary exists because the per-site listing is useful to a human
//! cleaning a module up, which is the one thing a test assertion is bad at.
//!
//! Usage:
//!   cargo run -p talkbank-parser-tests --bin audit_content_catch_alls

use std::process::ExitCode;

use talkbank_parser_tests::content_catch_alls::Audit;
use talkbank_parser_tests::gate::Tree;

fn main() -> ExitCode {
    let tree = Tree::live().into_read();
    let audit = Audit::of(&tree);

    for hit in audit.hits() {
        println!("{}:{}", hit.file, hit.line);
    }
    println!();

    audit.outcome(&tree).into_exit_code()
}
