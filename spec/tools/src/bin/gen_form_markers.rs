//! Generate every site that carries the CHAT form-marker inventory.
//!
//! One owner, `spec/form_markers/form_marker_registry.json`; three derived
//! sites. Run it after any registry change:
//!
//! ```bash
//! just form-markers-gen
//! ```
//!
//! The re2c output additionally requires regenerating the vendored lexer
//! (`just verify-vendored-lexer`), and a change to the model's shape or doc
//! comments requires regenerating the JSON Schema. Both are stated on every run
//! rather than left to be remembered; see `spec/form_markers/README.md` for why
//! neither can be a test in this workspace.

use anyhow::Context;
use anyhow::Result;
use generators::form_markers::registry::FormMarkerRegistry;
use generators::form_markers::render;
use generators::repo_paths::RepoRoot;
use std::path::Path;

fn main() -> Result<()> {
    let repo_root = RepoRoot::resolve(None)?;
    let registry = FormMarkerRegistry::load(repo_root.as_path()).with_context(|| {
        format!(
            "loading the form-marker registry under {}",
            repo_root.as_path().display()
        )
    })?;

    println!("form-marker registry: {} markers", registry.markers().len());

    // Iterates the same list the drift gate checks, so this binary cannot write
    // a set of files different from the set that is verified.
    for output in render::OUTPUTS {
        let rendered =
            (output.render)(&registry).with_context(|| format!("rendering {}", output.what))?;
        write(repo_root.as_path(), output, &rendered)?;
    }

    println!(
        "\nTwo follow-ups this generator cannot do for you:\n\
         \x20 1. regenerate the vendored re2c lexer: `just verify-vendored-lexer`\n\
         \x20 2. regenerate the JSON Schema if the model's shape or docs moved:\n\
         \x20    cargo test -p talkbank-transform --tests generate_schema"
    );

    Ok(())
}

/// Write `content` to `relative` under `repo_root`, reporting whether it moved.
///
/// Reports "unchanged" rather than staying silent: a regeneration that changes
/// nothing and a regeneration that never ran look identical in a terminal, and
/// that ambiguity has already cost this workspace a session.
fn write(repo_root: &Path, output: &render::GeneratedFile, content: &str) -> Result<()> {
    let relative = output.path;
    let path = repo_root.join(relative);
    let parent = path
        .parent()
        .with_context(|| format!("{relative} has no parent directory"))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };

    if existing.as_deref() == Some(content) {
        println!("  unchanged: {relative} ({})", output.what);
        return Ok(());
    }

    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    println!("  written:   {relative} ({})", output.what);
    Ok(())
}
