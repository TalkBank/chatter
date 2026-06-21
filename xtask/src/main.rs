use std::env;
use std::error::Error;
use std::path::Path;
use std::process;

type DynError = Box<dyn Error>;
type Result<T> = std::result::Result<T, DynError>;

mod docs_sync;

fn main() {
    if let Err(error) = run_main() {
        eprintln!("xtask error: {error}");
        process::exit(1);
    }
}

fn run_main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("help") => {
            print_help();
            Ok(())
        }
        Some("lint-docs-sync") => {
            if args.next().is_some() {
                return Err(usage_error());
            }
            docs_sync::run(repo_root()?)
        }
        _ => Err(usage_error()),
    }
}

fn usage_error() -> DynError {
    "usage: cargo run -q -p xtask -- {help|lint-docs-sync}".into()
}

fn print_help() {
    println!("chatter xtask commands");
    println!();
    println!("  help");
    println!("      Show this command summary.");
    println!("  lint-docs-sync");
    println!(
        "      Check public docs that must stay synchronized with the current chatter surface."
    );
}

fn repo_root() -> Result<&'static Path> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| -> DynError { "xtask crate must live under repo root".into() })
}
