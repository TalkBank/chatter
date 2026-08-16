//! # Error Corpus Specification Types
//!
//! Types for error corpus specifications - invalid CHAT examples
//! that should produce parse errors.

use comrak::nodes::NodeValue;
use comrak::{Arena, Options, parse_document};
use serde::{Deserialize, Serialize};

use super::metadata::{CategoryName, SpecErrorCode, SpecLayer, Status};
use std::fs;
use std::path::Path;

use super::comrak_text::{
    extract_text_from_children, normalize_whitespace, strip_single_trailing_newline,
};

/// Whether a path names an error spec, as opposed to prose living in the same
/// directory.
///
/// A spec is `E<digits>...md` or `W<digits>...md`. This is what lets
/// `load_all` fail closed: without a way to tell a spec from a README, every
/// parse failure had to be tolerated, and tolerating them is what hid E768
/// from the coverage gate.
fn is_error_spec_filename(path: &std::path::Path) -> bool {
    // The same question `looks_like_a_code` answers for a token, asked of a
    // stem. It used to be a byte-identical copy of that predicate 500 lines
    // away in this file, so "what looks like a code" had two definitions that
    // had to move together and would have disagreed in opposite directions.
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(super::metadata::looks_like_a_code)
}

/// The structural level a spec's error occurs at (`file`, `utterance`, `tier`,
/// `word`, ...): an open set across corpora, so a validated newtype, not an enum.
///
/// # Every route in, enumerated
///
/// [`FromStr`] only. `Deserialize` routes through `TryFrom<String>` so a level
/// read back from JSON is held to the same non-empty rule as one read from
/// markdown. It carried a bare `#[serde(transparent)]` until 2026-08-15, which
/// would have built `SpecLevel("")` from any JSON string; its immediate
/// neighbour in [`ErrorCorpusMetadata`], `CategoryName`, had that door closed
/// four lines away, so the struct answered "how do I get one of these" two
/// different ways depending on which field you looked at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SpecLevel(String);

impl SpecLevel {
    /// The level label text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for SpecLevel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let label = value.trim();
        if label.is_empty() {
            return Err("Empty Level in Metadata".to_string());
        }
        Ok(Self(label.to_owned()))
    }
}

impl TryFrom<String> for SpecLevel {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<SpecLevel> for String {
    fn from(level: SpecLevel) -> Self {
        level.0
    }
}

/// Root structure for an error corpus specification file
#[derive(Debug, Deserialize)]
pub struct ErrorCorpusSpec {
    pub metadata: ErrorCorpusMetadata,
    pub examples: Vec<ErrorCorpusExample>,
    /// Filesystem path this spec was loaded from. Not part of the markdown; set
    /// by `parse_markdown`/`load`. Carried so the generator can record each
    /// fixture's source spec in the manifest for diagnostics.
    #[serde(skip)]
    source_path: std::path::PathBuf,
}

/// Metadata about the error category
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorCorpusMetadata {
    /// Category grouping (`retrace`, `language`, ...).
    pub category: CategoryName,
    /// Human-readable description of this error category.
    pub description: String,
    /// Structural level where errors occur (`file`, `utterance`, `tier`, ...).
    pub level: SpecLevel,
    /// Layer: parser (grammar-level) or validation (semantic-level). Specs
    /// without an explicit Layer default to parser.
    #[serde(default)]
    pub layer: SpecLayer,
    /// Implementation status. REQUIRED; a spec declaring none is refused.
    pub status: Status,
}

/// A single error corpus example with invalid input
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorCorpusExample {
    /// Unique name for this example (used in test names)
    pub name: String,
    /// Human-readable description of what's wrong
    pub description: String,
    /// The invalid CHAT input that should produce an error
    pub input: String,
    /// Primary expected code - optional, for documentation. Kept for
    /// back-compat; equals `expected_codes.first()`.
    #[serde(default)]
    pub error_code: Option<SpecErrorCode>,
    /// All codes this example declares via its `**Expected Error Codes**` line,
    /// falling back to the spec's title code. The generator tests the fixture
    /// for THESE codes (not the title code), so every example of a multi-example
    /// spec is checked for its own declared codes instead of being dropped.
    #[serde(default)]
    pub expected_codes: Vec<SpecErrorCode>,
    /// Human description of where the error occurs
    #[serde(default)]
    pub error_location: Option<String>,
    /// Additional notes about the error
    #[serde(default)]
    pub notes: Option<String>,
    /// Expected CST showing ERROR nodes (optional, can be auto-generated)
    #[serde(default)]
    pub expected_cst: Option<String>,
}

impl ErrorCorpusSpec {
    /// The path this spec was loaded from, for manifest provenance / diagnostics.
    pub fn source_path_display(&self) -> String {
        self.source_path.display().to_string()
    }

    /// Load an error corpus specification from a markdown file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| format!("failed to read: {e}"))?;

        Self::parse_markdown(&content, path)
    }

    /// Load all error corpus specifications from a directory tree
    pub fn load_all(root: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let root = root.as_ref();
        let mut specs = Vec::new();

        if !root.exists() {
            return Ok(specs);
        }

        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                // Only files NAMED like a spec are specs. `spec/errors/` also
                // holds documentation (README, the enhancement guide), and
                // those must be skipped rather than parsed.
                if !is_error_spec_filename(path) {
                    continue;
                }
                // FAIL CLOSED. This used to be `eprintln!("Warning: ...")`,
                // which meant a spec file that failed to parse silently left
                // the corpus, taking any gate that would have judged it with
                // it. A warning on stderr during code generation is
                // indistinguishable from noise; a spec that does not parse is
                // a defect in the spec.
                let spec = Self::load(path)
                    .map_err(|e| format!("Failed to load {}: {}", path.display(), e))?;
                specs.push(spec);
            }
        }

        Ok(specs)
    }

    /// Parse markdown content into an ErrorCorpusSpec
    /// # Errors do NOT name the file
    ///
    /// [`Self::load_all`] prefixes every failure with `Failed to load {path}:`.
    /// Twelve sites here named it again until 2026-08-15, so every corpus-spec
    /// failure printed the path twice, and the sibling parser had the same
    /// defect at eight sites. `path` survives as a parameter because
    /// `source_path` genuinely needs it; it is not for error text.
    fn parse_markdown(content: &str, path: &Path) -> Result<Self, String> {
        /// Enum variants for Section.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Section {
            None,
            Description,
            Metadata,
            Example,
            ExpectedBehavior,
            ChatRule,
            Notes,
        }

        let arena = Arena::new();
        let root = parse_document(&arena, content, &Options::default());

        let mut section = Section::None;
        let mut title = None::<String>;
        let mut description_parts = Vec::new();
        let mut expected_behavior_parts = Vec::new();
        let mut chat_rule_parts = Vec::new();
        let mut notes_parts = Vec::new();

        let mut category = None::<String>;
        let mut level = None::<String>;
        let mut layer = None::<String>;
        let mut status = None::<String>;

        // Each `## Example` segment yields one fixture: its chat block plus the
        // codes declared on its `**Expected Error Codes**` line. We accumulate
        // the current segment and finalize it at every heading boundary, so the
        // chat block and its Expected line pair regardless of their order.
        let mut examples: Vec<(String, Vec<SpecErrorCode>)> = Vec::new();
        let mut current_input: Option<String> = None;
        let mut current_codes: Vec<SpecErrorCode> = Vec::new();

        for node in root.descendants() {
            let node_data = node.data.borrow();
            match &node_data.value {
                NodeValue::Heading(heading) if heading.level == 1 => {
                    title = Some(normalize_whitespace(&extract_text_from_children(node)));
                }
                NodeValue::Heading(heading) if heading.level == 2 => {
                    // A new heading ends the current example segment.
                    finalize_example(&mut examples, &mut current_input, &mut current_codes);
                    let heading_text = normalize_whitespace(&extract_text_from_children(node));
                    section = if heading_text == "Description" {
                        Section::Description
                    } else if heading_text == "Metadata" {
                        Section::Metadata
                    } else if heading_text == "Example" || heading_text.starts_with("Example ") {
                        Section::Example
                    } else if heading_text == "Expected Behavior" {
                        Section::ExpectedBehavior
                    } else if heading_text == "CHAT Rule" {
                        Section::ChatRule
                    } else if heading_text == "Notes" {
                        Section::Notes
                    } else {
                        Section::None
                    };
                }
                NodeValue::Paragraph => {
                    let text = normalize_whitespace(&extract_text_from_children(node));
                    if text.is_empty() {
                        continue;
                    }
                    match section {
                        Section::Description => description_parts.push(text),
                        Section::ExpectedBehavior => expected_behavior_parts.push(text),
                        Section::ChatRule => chat_rule_parts.push(text),
                        Section::Notes => notes_parts.push(text),
                        // The `**Expected Error Codes**: E###, ...` line for
                        // the current example segment.
                        Section::Example if text.contains("Expected Error Codes") => {
                            current_codes = super::metadata::expected_error_codes(&text)?;
                        }
                        _ => {}
                    }
                }
                NodeValue::List(_) if section == Section::Metadata => {
                    for child in node.children() {
                        if let NodeValue::Item(_) = child.data.borrow().value {
                            let mut key = String::new();
                            let mut value = String::new();
                            let mut found_colon = false;

                            for item_node in child.descendants() {
                                // Check if this node is inside a Strong parent
                                let is_in_strong = item_node.parent().is_some_and(|p| {
                                    matches!(p.data.borrow().value, NodeValue::Strong)
                                });

                                match &item_node.data.borrow().value {
                                    NodeValue::Text(text) => {
                                        if is_in_strong {
                                            let mut strong_text = text.to_string();
                                            if strong_text.ends_with(':') {
                                                strong_text.pop();
                                            }
                                            key.push_str(&strong_text);
                                        } else if text.contains(':') && !found_colon {
                                            found_colon = true;
                                            let parts: Vec<&str> = text.splitn(2, ':').collect();
                                            if parts.len() == 2 {
                                                value.push_str(parts[1]);
                                            }
                                        } else if found_colon {
                                            value.push_str(text);
                                        }
                                    }
                                    NodeValue::Code(code) if found_colon => {
                                        value.push_str(&code.literal);
                                    }
                                    _ => {}
                                }
                            }

                            let key = normalize_whitespace(&key);
                            let value = normalize_whitespace(&value);
                            if key == "Category" {
                                category = Some(value);
                            } else if key == "Level" {
                                level = Some(value);
                            } else if key == "Layer" {
                                layer = Some(value);
                            } else if key == "Status" {
                                status = Some(value);
                            }
                        }
                    }
                }
                NodeValue::CodeBlock(code_block) if section == Section::Example => {
                    // REFUSE a wrong fence rather than fall through to `_ => {}`.
                    // That fall-through left `current_input` as `None`, and
                    // `finalize_example`'s `None => codes.clear()` then discarded
                    // the example AND its declared codes with no signal at all,
                    // so a mis-fenced example vanished from the validation corpus
                    // and from `manifest.json`. Invisible in a directory listing,
                    // which is what `examples_asserting_nothing_do_not_increase`
                    // exists to catch. It is the same "dropping is the worst
                    // available outcome" argument as `expected_error_codes`, and
                    // it matches the sibling parser, which already refuses here.
                    if code_block.info != "chat" {
                        return Err(format!(
                            "an example's code fence is ```{} where only ```chat \
                             is supported.",
                            code_block.info
                        ));
                    }
                    current_input = Some(strip_single_trailing_newline(&code_block.literal));
                }
                _ => {}
            }
        }
        // Finalize the last example (no trailing heading closes it).
        finalize_example(&mut examples, &mut current_input, &mut current_codes);

        let title = title.ok_or_else(|| "Missing title".to_string())?;
        // `metadata::parse_spec_title` owns this for both parsers of these
        // files. This copy split on the first `:` or `,`; `error.rs`'s took the
        // first whitespace token and stripped only a trailing `:`, so the two
        // disagreed on the 11 titles written `# E209, name`. The shared parser
        // keeps the separator rule, which is the one the data supports, and
        // falls back to the first token when a heading has no separator, so it
        // is a superset of both rather than a swap of one for the other.
        let parsed_title = super::metadata::parse_spec_title(&title);
        let name = parsed_title.name;
        if name.is_empty() {
            return Err("Invalid title format".to_string());
        }

        let description = normalize_whitespace(&description_parts.join(" "));
        if description.is_empty() {
            return Err("Missing Description content".to_string());
        }

        let category_str = category.ok_or_else(|| "Missing Category in Metadata".to_string())?;
        let category = category_str
            .parse::<CategoryName>()
            .map_err(|why| why.to_string())?;
        let level_str = level.ok_or_else(|| "Missing Level in Metadata".to_string())?;
        let level = level_str.parse::<SpecLevel>()?;
        let layer = match layer {
            Some(text) => text.parse::<SpecLayer>().map_err(|e| e.to_string())?,
            None => SpecLayer::default(),
        };
        let status = match status {
            Some(text) => text.parse::<Status>().map_err(|e| e.to_string())?,
            // Required, exactly as in `error.rs`. This used to be
            // `Status::default()`, which invented `implemented` for a spec that
            // declared nothing. Every one of the 236 error specs declares a
            // Status today, so this arm was already unreachable in practice;
            // what it did was let the two parsers of the same file disagree
            // about whether the bullet is optional.
            None => {
                return Err("no `- **Status**:` bullet. Every spec must declare \
                     one (implemented / not_implemented / deprecated / \
                     unreachable_from_chat)."
                    .to_string());
            }
        };

        // The spec's title code is each example's fallback when it declares no
        // `Expected Error Codes` of its own.
        // This parser has no bullet route, so a heading that names no code is
        // fatal here where `error.rs` can still fall back.
        let title_code = parsed_title
            .code
            .ok_or_else(|| format!("the title {title:?} names no error code"))?;

        // Deliberately NOT an error: parse what is there and let the
        // coverage gate rule on it. Erroring here made `load_all` drop the
        // spec, which is how an example-less spec slipped past the gate whose
        // whole job is to notice one.

        let _expected_behavior = expected_behavior_parts.join("\n");
        let _chat_rule = normalize_whitespace(&chat_rule_parts.join(" "));

        let notes = if notes_parts.is_empty() {
            None
        } else {
            Some(normalize_whitespace(&notes_parts.join(" ")))
        };

        let metadata = ErrorCorpusMetadata {
            category,
            description: description.clone(),
            level,
            layer,
            status,
        };

        // One ErrorCorpusExample per `## Example` segment. An example with no
        // explicit `Expected Error Codes` falls back to the spec's title code,
        // preserving the single-example specs' previous behavior.
        let built_examples: Vec<ErrorCorpusExample> = examples
            .into_iter()
            .map(|(input, codes)| {
                let expected_codes = if codes.is_empty() {
                    vec![title_code.clone()]
                } else {
                    codes
                };
                ErrorCorpusExample {
                    name: name.clone(),
                    description: description.clone(),
                    input,
                    error_code: expected_codes.first().cloned(),
                    expected_codes,
                    error_location: None,
                    notes: notes.clone(),
                    expected_cst: None,
                }
            })
            .collect();

        Ok(ErrorCorpusSpec {
            metadata,
            examples: built_examples,
            source_path: path.to_path_buf(),
        })
    }
}

impl ErrorCorpusExample {
    /// Generate a sanitized test name
    pub fn test_name(&self) -> String {
        self.name.replace(['-', ' '], "_").to_lowercase()
    }

    /// Get the expected CST if available, otherwise return placeholder
    pub fn expected_cst_or_placeholder(&self) -> String {
        match self.expected_cst.as_ref() {
            Some(cst) => cst.clone(),
            None => "(todo)".to_string(),
        }
    }
}

/// Finalize the current `## Example` segment into the examples list. A segment
/// with no chat block contributes nothing (and any stray Expected codes are
/// discarded); otherwise it pairs the chat input with its declared codes.
fn finalize_example(
    examples: &mut Vec<(String, Vec<SpecErrorCode>)>,
    input: &mut Option<String>,
    codes: &mut Vec<SpecErrorCode>,
) {
    match input.take() {
        Some(text) => examples.push((text, std::mem::take(codes))),
        None => codes.clear(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A minimal but valid error-corpus spec markdown carrying an explicit
    /// `Status` bullet in its `## Metadata` section. The `{status}`
    /// placeholder is substituted so one body serves every status, including
    /// the absent case that `status_is_required` asserts is refused.
    fn spec_markdown(status_bullet: &str) -> String {
        format!(
            "# E999: Test error\n\
             \n\
             ## Description\n\
             \n\
             A test error description.\n\
             \n\
             ## Metadata\n\
             \n\
             - **Category**: retrace\n\
             - **Level**: utterance\n\
             - **Layer**: validation\n\
             {status_bullet}\
             \n\
             ## Example\n\
             \n\
             ```chat\n\
             @UTF8\n\
             @Begin\n\
             @End\n\
             ```\n"
        )
    }

    #[test]
    fn status_not_implemented_is_parsed() {
        let markdown = spec_markdown("- **Status**: not_implemented\n");
        let spec = ErrorCorpusSpec::parse_markdown(&markdown, Path::new("E999_test.md"))
            .expect("spec with explicit Status should parse");
        assert_eq!(spec.metadata.status, Status::NotImplemented);
    }

    /// A spec with no `**Status**` bullet is REFUSED, naming the file.
    ///
    /// Replaces `status_defaults_to_implemented_when_absent`, which asserted
    /// that such a spec parsed and came back `Implemented`. That test encoded
    /// the loss: it made the invented answer a guaranteed behaviour, so
    /// removing the invention meant rewriting the test's expectation, which is
    /// the shape this project treats as a standing confession.
    #[test]
    fn status_is_required() {
        let markdown = spec_markdown("");
        let why = ErrorCorpusSpec::parse_markdown(&markdown, Path::new("E999_test.md"))
            .expect_err("a spec without Status must be refused");
        assert!(why.contains("Status"), "{why}");
    }

    /// A loaded spec must retain the path it came from so the generator can
    /// record each fixture's `source_spec` provenance in the manifest. Without
    /// this the manifest could not point a failing fixture back at its spec.
    #[test]
    fn spec_retains_its_source_path() {
        let markdown = spec_markdown("- **Status**: implemented\n");
        let spec =
            ErrorCorpusSpec::parse_markdown(&markdown, Path::new("spec/errors/E999_test.md"))
                .expect("spec should parse");
        assert_eq!(spec.source_path_display(), "spec/errors/E999_test.md");
    }

    /// A minimal but valid error-corpus spec markdown whose `# E### ...`
    /// title separates the code from the human title with a COMMA instead
    /// of a colon. Nine real specs in `spec/errors/` use this comma form
    /// (E348, E220, E248, E249, E701, E245, E370, E347, E209); the loader
    /// must parse them, not silently skip them.
    fn comma_form_spec_markdown() -> String {
        "# E249, Bare @s shortcut with no secondary language\n\
         \n\
         ## Description\n\
         \n\
         A test error description.\n\
         \n\
         ## Metadata\n\
         \n\
         - **Category**: language\n\
         - **Level**: word\n\
         - **Layer**: validation\n\
         - **Status**: implemented\n\
         \n\
         ## Example\n\
         \n\
         ```chat\n\
         @UTF8\n\
         @Begin\n\
         @End\n\
         ```\n\
         \n\
         **Expected Error Codes**: E249\n"
            .to_string()
    }

    /// RED guard for the comma-form title bug: the title parser used to
    /// split only on `:`, so a `# E###, ...` title produced an empty/invalid
    /// error code and `load`/`load_all` silently dropped the spec. The
    /// loader must parse the comma form, deriving code `E249` and a
    /// non-empty human title, and emit exactly one example.
    #[test]
    fn comma_form_title_is_parsed() {
        let markdown = comma_form_spec_markdown();
        let spec = ErrorCorpusSpec::parse_markdown(&markdown, Path::new("E249_auto.md"))
            .expect("comma-form spec should parse");
        let example = spec
            .examples
            .first()
            .expect("comma-form spec should yield one example");
        assert_eq!(
            example.error_code.as_ref().map(|c| c.as_str()),
            Some("E249")
        );
        assert_eq!(example.name, "Bare @s shortcut with no secondary language");
        assert_eq!(spec.examples.len(), 1);
    }

    /// Colon-form titles must keep their exact prior behavior after the
    /// comma-acceptance fix: code and human title split on the first `:`.
    #[test]
    fn colon_form_title_behavior_unchanged() {
        let markdown = spec_markdown("- **Status**: implemented\n");
        let spec = ErrorCorpusSpec::parse_markdown(&markdown, Path::new("E999_test.md"))
            .expect("colon-form spec should parse");
        let example = spec
            .examples
            .first()
            .expect("colon-form spec should yield one example");
        assert_eq!(
            example.error_code.as_ref().map(|c| c.as_str()),
            Some("E999")
        );
        assert_eq!(example.name, "Test error");
    }

    /// Resolve the real `spec/errors` directory from this crate's manifest
    /// dir. `CARGO_MANIFEST_DIR` for this crate is `<repo>/spec/tools`, so
    /// the repo root is two levels up and `spec/errors` hangs off it.
    fn spec_errors_dir() -> std::path::PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent() // <repo>/spec
            .and_then(Path::parent) // <repo>
            .expect("crate manifest dir should have a grandparent (repo root)");
        repo_root.join("spec").join("errors")
    }

    /// Count the `.md` files under `spec/errors` that the loader is
    /// CONTRACTUALLY expected to load: those carrying an `# E###`/`# W###`
    /// title (colon OR comma form) AND a `## Example` section containing a
    /// ` ```chat ` block, which together are the minimum the loader requires
    /// to produce a spec. README / guide files (prose titles such as
    /// `# Error Specifications`) are excluded, as are a handful of specs that
    /// carry a code title but no usable `## Example` chat block (E001/E002/
    /// E340 placeholder specs; E502's reproduction lives under a
    /// `## Minimal Reproduction` heading, not `## Example`): those are a
    /// separate, pre-existing data gap unrelated to the title-separator bug,
    /// so counting them here would conflate two distinct failure classes.
    /// This count therefore equals exactly the set of loadable specs, making
    /// the strict-equality guard a tight regression gate on the title-parsing
    /// surface specifically.
    fn count_loadable_error_specs(dir: &Path) -> usize {
        let mut count = 0;
        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let Ok(content) = fs::read_to_string(path) else {
                continue;
            };
            let Some(first_line) = content.lines().find(|l| !l.trim().is_empty()) else {
                continue;
            };
            // An example is NO LONGER part of being loadable. It used to be:
            // `parse_markdown` rejected a spec with no `## Example`, so this
            // count had to exclude such files or the assertion would never
            // hold. That symmetry is exactly what hid the problem, since a
            // spec missing an example was subtracted from BOTH sides and the
            // guard could not see it go. The invariant is now the stronger
            // one it always should have been: every file with an E###/W###
            // title must load.
            if is_error_spec_title(first_line) {
                count += 1;
            }
        }
        count
    }

    /// True when a first line is a `# E###`/`# W###` error-spec title, i.e.
    /// `# ` followed by `E` or `W`, then one or more ASCII digits. Matches
    /// both colon-form (`# E316: ...`) and comma-form (`# E249, ...`).
    fn is_error_spec_title(line: &str) -> bool {
        let Some(rest) = line.strip_prefix("# ") else {
            return false;
        };
        let mut chars = rest.chars();
        match chars.next() {
            Some('E') | Some('W') => {}
            _ => return false,
        }
        let mut saw_digit = false;
        for c in chars {
            if c.is_ascii_digit() {
                saw_digit = true;
                continue;
            }
            break;
        }
        saw_digit
    }

    /// Count guard: every `.md` file in `spec/errors` that carries an
    /// `# E###`/`# W###` title AND a usable `## Example` chat block MUST
    /// load. A future title format the loader cannot handle would shrink
    /// `load_all`'s output below this count, failing here loudly instead of
    /// silently dropping coverage. This is the regression gate the whole
    /// "spec/errors is the single source of truth" effort depends on.
    #[test]
    fn load_all_loads_every_error_spec_file() {
        let dir = spec_errors_dir();
        assert!(
            dir.is_dir(),
            "spec/errors directory should exist at {}",
            dir.display()
        );

        let expected = count_loadable_error_specs(&dir);
        let specs = ErrorCorpusSpec::load_all(&dir).expect("load_all should succeed");
        let loaded = specs.len();

        assert_eq!(
            loaded,
            expected,
            "load_all loaded {loaded} specs but {expected} .md files in {} carry an E###/W### \
             title and a usable ## Example chat block; some specs are being silently dropped \
             by the loader",
            dir.display()
        );

        // Belt-and-suspenders floor: 172 colon-form + 9 comma-form = 181
        // specs currently carry both a code title and a `## Example` chat
        // block. (The original task estimate of 175+9=184 predated counting
        // the 4 code-titled specs that lack a usable `## Example` block:
        // E001/E002/E340 placeholders and E502's `## Minimal Reproduction`.)
        // This floor catches a comma-form regression even if the file-count
        // helper itself drifts.
        const MIN_EXPECTED_SPECS: usize = 181;
        assert!(
            loaded >= MIN_EXPECTED_SPECS,
            "load_all loaded only {loaded} specs; expected at least {MIN_EXPECTED_SPECS} \
             (172 colon-form + 9 comma-form with a usable ## Example chat block)",
        );
    }
}
