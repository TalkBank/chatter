//! # Error Corpus Specification Types
//!
//! Types for error corpus specifications - invalid CHAT examples
//! that should produce parse errors.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// Raised when a closed-set metadata value (Status, Layer) is unrecognized.
#[derive(Debug, thiserror::Error)]
#[error("unknown {field} value {value:?}")]
pub struct UnknownMetadataValue {
    field: &'static str,
    value: String,
}

/// Implementation status of an error spec: whether the validator actually
/// checks its rule. The runner asserts `Implemented` examples fire and skips
/// the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Rule implemented; each example must produce its declared codes.
    #[default]
    Implemented,
    /// Rule not implemented yet; examples are skipped by the runner.
    NotImplemented,
    /// Code deprecated/replaced by another; examples are skipped.
    Deprecated,
}

impl FromStr for Status {
    type Err = UnknownMetadataValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "implemented" => Ok(Self::Implemented),
            "not_implemented" => Ok(Self::NotImplemented),
            "deprecated" => Ok(Self::Deprecated),
            other => Err(UnknownMetadataValue {
                field: "Status",
                value: other.to_owned(),
            }),
        }
    }
}

/// The layer a spec's rule lives at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecLayer {
    /// Grammar/parser-level error.
    #[default]
    Parser,
    /// Semantic validation-level error.
    Validation,
}

impl SpecLayer {
    /// Whether this spec contributes a validation fixture.
    pub fn is_validation(self) -> bool {
        matches!(self, Self::Validation)
    }
}

impl FromStr for SpecLayer {
    type Err = UnknownMetadataValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "parser" => Ok(Self::Parser),
            // A spec flagged as both is treated as validation for generation.
            "validation" | "parser|validation" => Ok(Self::Validation),
            other => Err(UnknownMetadataValue {
                field: "Layer",
                value: other.to_owned(),
            }),
        }
    }
}

/// A CHAT error/warning code as written in specs: `E` or `W` followed by three
/// digits. Construction validates the shape so a malformed code never reaches
/// the manifest or the runner.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpecErrorCode(String);

impl SpecErrorCode {
    /// Parse an `E###`/`W###` token; `None` if it is not well formed.
    pub fn parse(token: &str) -> Option<Self> {
        let token = token.trim();
        let bytes = token.as_bytes();
        let well_formed = bytes.len() == 4
            && matches!(bytes[0], b'E' | b'W')
            && bytes[1..].iter().all(u8::is_ascii_digit);
        well_formed.then(|| Self(token.to_owned()))
    }

    /// The underlying `E###`/`W###` text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpecErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The structural level a spec's error occurs at (`file`, `utterance`, `tier`,
/// `word`, ...): an open set across corpora, so a validated newtype, not an enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpecLevel(String);

impl SpecLevel {
    /// Wrap a non-empty level label.
    pub fn new(label: &str) -> Option<Self> {
        let label = label.trim();
        (!label.is_empty()).then(|| Self(label.to_owned()))
    }
    /// The level label text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The category grouping for a spec (`retrace`, `language`, ...): an open set,
/// so a validated newtype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CategoryName(String);

impl CategoryName {
    /// Wrap a non-empty category name.
    pub fn new(name: &str) -> Option<Self> {
        let name = name.trim();
        (!name.is_empty()).then(|| Self(name.to_owned()))
    }
    /// The category name text.
    pub fn as_str(&self) -> &str {
        &self.0
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
    /// Implementation status. Specs without an explicit Status default to
    /// implemented.
    #[serde(default)]
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
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

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
                match Self::load(path) {
                    Ok(spec) => specs.push(spec),
                    Err(e) => eprintln!("Warning: Failed to load {}: {}", path.display(), e),
                }
            }
        }

        Ok(specs)
    }

    /// Parse markdown content into an ErrorCorpusSpec
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
                            current_codes = extract_error_codes(&text);
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
                NodeValue::CodeBlock(code_block)
                    if section == Section::Example && code_block.info == "chat" =>
                {
                    current_input = Some(strip_single_trailing_newline(&code_block.literal));
                }
                _ => {}
            }
        }
        // Finalize the last example (no trailing heading closes it).
        finalize_example(&mut examples, &mut current_input, &mut current_codes);

        let title = title.ok_or_else(|| format!("Missing title in {}", path.display()))?;
        // The `# E### ...` / `# W### ...` title separates the error code from
        // the human title with either a colon (`# E316: ...`, the majority
        // form) or a comma (`# E249, ...`, used by nine specs). Split on
        // whichever separator appears FIRST so both forms parse. For a
        // colon-first title this is byte-identical to the previous
        // `splitn(2, ':')` behavior; a comma-first title that previously
        // produced no usable code (and was silently dropped by `load_all`)
        // now parses correctly.
        let separator_index = title.find([':', ',']).ok_or_else(|| {
            format!(
                "Missing error code separator in title for {}",
                path.display()
            )
        })?;
        let (code_part, rest_with_separator) = title.split_at(separator_index);
        // `rest_with_separator` still leads with the separator char; drop it.
        let name_part = &rest_with_separator[rest_with_separator
            .char_indices()
            .nth(1)
            .map_or(rest_with_separator.len(), |(byte_index, _)| byte_index)..];
        let error_code = normalize_whitespace(code_part);
        let name = normalize_whitespace(name_part);

        if error_code.is_empty() || name.is_empty() {
            return Err(format!("Invalid title format in {}", path.display()));
        }

        let description = normalize_whitespace(&description_parts.join(" "));
        if description.is_empty() {
            return Err(format!("Missing Description content in {}", path.display()));
        }

        let category_str = category
            .ok_or_else(|| format!("Missing Category in Metadata in {}", path.display()))?;
        let category = CategoryName::new(&category_str)
            .ok_or_else(|| format!("Empty Category in Metadata in {}", path.display()))?;
        let level_str =
            level.ok_or_else(|| format!("Missing Level in Metadata in {}", path.display()))?;
        let level = SpecLevel::new(&level_str)
            .ok_or_else(|| format!("Empty Level in Metadata in {}", path.display()))?;
        let layer = match layer {
            Some(text) => text
                .parse::<SpecLayer>()
                .map_err(|e| format!("{e} in {}", path.display()))?,
            None => SpecLayer::default(),
        };
        let status = match status {
            Some(text) => text
                .parse::<Status>()
                .map_err(|e| format!("{e} in {}", path.display()))?,
            None => Status::default(),
        };

        // The spec's title code is each example's fallback when it declares no
        // `Expected Error Codes` of its own.
        let title_code = SpecErrorCode::parse(&error_code).ok_or_else(|| {
            format!(
                "Malformed title error code {error_code:?} in {}",
                path.display()
            )
        })?;

        if examples.is_empty() {
            return Err(format!(
                "Missing Example chat code block in {}",
                path.display()
            ));
        }

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

/// Extracts text from children.
fn extract_text_from_children<'a>(node: &'a AstNode<'a>) -> String {
    let mut result = String::new();
    for child in node.descendants() {
        if let NodeValue::Text(ref text) = child.data.borrow().value {
            result.push_str(text);
        }
    }
    result
}

/// Runs normalize whitespace.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// Extract the `E###` / `W###` codes from a `**Expected Error Codes**: ...`
/// paragraph. Scans only the portion after the label so a code mentioned
/// elsewhere is not picked up. Tokenizes on non-alphanumerics and keeps the
/// well-formed codes; `SpecErrorCode::parse` is the single source of truth for
/// code shape, so a malformed or over-long token is simply dropped.
fn extract_error_codes(text: &str) -> Vec<SpecErrorCode> {
    let after = match text.find("Expected Error Codes") {
        Some(idx) => &text[idx..],
        None => return Vec::new(),
    };
    after
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(SpecErrorCode::parse)
        .collect()
}

/// Runs strip single trailing newline.
fn strip_single_trailing_newline(text: &str) -> String {
    if let Some(stripped) = text.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = text.strip_suffix('\n') {
        stripped.to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A minimal but valid error-corpus spec markdown carrying an explicit
    /// `Status` bullet in its `## Metadata` section. The `{status}`
    /// placeholder is substituted so the same body exercises both the
    /// explicit-status and absent-status cases.
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

    #[test]
    fn status_defaults_to_implemented_when_absent() {
        let markdown = spec_markdown("");
        let spec = ErrorCorpusSpec::parse_markdown(&markdown, Path::new("E999_test.md"))
            .expect("spec without Status should parse");
        assert_eq!(spec.metadata.status, Status::Implemented);
    }

    /// A loaded spec must retain the path it came from so the generator can
    /// record each fixture's `source_spec` provenance in the manifest. Without
    /// this the manifest could not point a failing fixture back at its spec.
    #[test]
    fn spec_retains_its_source_path() {
        let markdown = spec_markdown("");
        let spec = ErrorCorpusSpec::parse_markdown(
            &markdown,
            Path::new("spec/errors/E999_test.md"),
        )
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
        assert_eq!(example.error_code.as_ref().map(|c| c.as_str()), Some("E249"));
        assert_eq!(example.name, "Bare @s shortcut with no secondary language");
        assert_eq!(spec.examples.len(), 1);
    }

    /// Colon-form titles must keep their exact prior behavior after the
    /// comma-acceptance fix: code and human title split on the first `:`.
    #[test]
    fn colon_form_title_behavior_unchanged() {
        let markdown = spec_markdown("");
        let spec = ErrorCorpusSpec::parse_markdown(&markdown, Path::new("E999_test.md"))
            .expect("colon-form spec should parse");
        let example = spec
            .examples
            .first()
            .expect("colon-form spec should yield one example");
        assert_eq!(example.error_code.as_ref().map(|c| c.as_str()), Some("E999"));
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
            if is_error_spec_title(first_line) && has_example_chat_block(&content) {
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

    /// True when the markdown has at least one ` ```chat ` fence that sits
    /// under an `## Example` (or `## Example ...`) section, mirroring the
    /// loader's own rule that only chat code blocks inside `Section::Example`
    /// become the spec's `input`. A chat block under any other heading
    /// (e.g. `## Minimal Reproduction`) does not count, exactly as the
    /// loader ignores it.
    fn has_example_chat_block(content: &str) -> bool {
        let mut in_example_section = false;
        for line in content.lines() {
            if let Some(heading) = line.strip_prefix("## ") {
                in_example_section = heading == "Example" || heading.starts_with("Example ");
                continue;
            }
            if in_example_section && line.trim_start().starts_with("```chat") {
                return true;
            }
        }
        false
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
