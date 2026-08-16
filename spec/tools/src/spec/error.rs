//! # Error Specification Types
//!
//! Structured representation of the error spec files in `spec/errors/`.
//!
//! Each Markdown file defines one error code with its metadata (kind,
//! category, layer), a human-readable description, and one or more bad-input
//! examples that should trigger the error. Generators consume these types to
//! emit Rust validation tests and error documentation pages.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

use super::metadata::{CategoryName, SpecErrorCode, SpecLayer, Status};

use super::comrak_text::{
    extract_text_from_children, normalize_whitespace, strip_single_trailing_newline,
};

/// Root structure for an error specification file.
///
/// Typically loaded from a single `spec/errors/E###_*.md` Markdown file.
/// Contains category-level metadata plus one or more error definitions (in
/// practice, one per file).
#[derive(Debug, Deserialize)]
pub struct ErrorSpec {
    /// Category-level metadata (layer, status, kind).
    pub metadata: ErrorMetadata,
    /// Error definitions contained in this spec (usually exactly one).
    pub errors: Vec<ErrorDefinition>,
    /// Basename of the source Markdown file (e.g. `"E304_MissingUtteranceTerminator.md"`),
    /// populated after loading -- not present in the Markdown itself.
    #[serde(skip)]
    pub source_file: String,
}

/// Metadata about the error category
#[derive(Debug, Deserialize)]
pub struct ErrorMetadata {
    /// Error category: "validation", "header_validation", etc.
    pub category: CategoryName,
    /// Which layer of the pipeline catches this rule.
    ///
    /// Named `error_type` and typed `String` until 2026-08-15, though it is
    /// parsed from the `**Layer**` bullet and the sibling parser of the same
    /// files already had it as [`SpecLayer`]. Two string comparisons decided
    /// what got GENERATED from it: `artifacts.rs` selected the tree-sitter
    /// corpus with `== "parser"`, and `output/rust_test.rs` skipped emitting a
    /// parser test with `== "validation"`. A misspelled bullet therefore
    /// dropped a spec out of the corpus, or emitted a parser test for a
    /// validation-layer rule -- which is precisely the E342/E390 failure the
    /// book documents as a warning. It is now a load error.
    ///
    /// The markdown loader resolves this explicitly: an absent bullet means
    /// parser, a present-but-unknown value is an error. That absent-means-parser
    /// rule is a real CHAT-spec convention both parsers have always followed,
    /// which is why [`SpecLayer`] keeps a `Default` where [`Status`] does not.
    #[serde(rename = "type")]
    pub layer: SpecLayer,
    /// Human-readable description
    pub description: String,
    /// Implementation status, parsed at load into the closed set.
    ///
    /// Was a `String` on this struct until 2026-08-15, while the sibling parser
    /// of the same files had it typed. Why that mattered, and what it cost, is
    /// recorded once on [`Status`].
    ///
    /// No `#[serde(default)]`: [`Status`] has no `Default`, deliberately, so a
    /// spec that declares nothing is refused on BOTH construction paths rather
    /// than on the markdown one only.
    pub status: Status,
    /// What this diagnostic intrinsically IS, per the code's own spec.
    ///
    /// Deliberately REQUIRED, not `Option` with a default: a spec file
    /// carrying no `Kind` metadata bullet fails to load (see
    /// [`ErrorSpec::load`]) rather than silently falling back to a guess.
    /// The talkbank-model `DiagnosticKind` registry
    /// (`crates/talkbank-model/src/errors/generated_diagnostic_kind.rs`) is
    /// generated from this field across every spec file, so an unclassified
    /// code is a build-time failure here, not a silent gap in that registry.
    pub kind: ErrorKind,
}

/// The four `DiagnosticKind` axis values a spec file's `## Metadata` block
/// can declare via its `- **Kind**:` bullet.
///
/// Mirrors `talkbank_model::errors::DiagnosticKind` structurally by name.
/// This crate cannot depend on `talkbank-model` (that would be circular:
/// `talkbank-model`'s own diagnostic-kind registry is generated FROM this
/// crate's spec loader, by a binary in the sibling `spec/runtime-tools`
/// crate, which is the one place both directions of the dependency meet).
/// The generator maps each variant here to the identically-named
/// `DiagnosticKind` variant by name; a variant added to one and not the
/// other is caught at the generator's match, not silently ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    /// Violates the spec, or the construct does not make sense.
    Invalidity,
    /// Preserved but not interpreted: a chatter coverage gap, never a fault
    /// in the file itself.
    Unmodeled,
    /// Valid now, discouraged, on a sunset path toward `Invalidity`.
    Deprecation,
    /// Valid, purely stylistic.
    Style,
}

impl ErrorKind {
    /// ONE table, the way [`Status`] and [`SpecLayer`] already have one.
    ///
    /// This name is FOUR things at once: what a `- **Kind**:` bullet must say,
    /// what the generated `DiagnosticKind` registry emits as source text, what
    /// `docs/errors/*.md` publishes, and what the index table shows. Three of
    /// those were separate matches until 2026-08-15, and the published pair
    /// were `{:?}` on the derived `Debug`, so renaming a variant would have
    /// silently changed user-facing documentation while
    /// `diagnostic_kind_variant` kept emitting the old literal and only
    /// `parse` failed loudly.
    fn as_str(self) -> &'static str {
        match self {
            Self::Invalidity => "Invalidity",
            Self::Unmodeled => "Unmodeled",
            Self::Deprecation => "Deprecation",
            Self::Style => "Style",
        }
    }

    /// Parse a `- **Kind**:` bullet value. Case-sensitive and exact: the
    /// four spelled-out variant names, nothing else (no abbreviations, no
    /// synonyms), so a typo in a spec file fails loudly at load time
    /// instead of silently defaulting.
    fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        [
            Self::Invalidity,
            Self::Unmodeled,
            Self::Deprecation,
            Self::Style,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == value)
        .ok_or_else(|| {
            format!(
                "unrecognized Kind value {value:?}: expected one of \
                 Invalidity, Unmodeled, Deprecation, Style"
            )
        })
    }

    /// The identically-named `talkbank_model::errors::DiagnosticKind`
    /// variant this value maps to, as source text for code generation.
    ///
    /// Identical to [`Self::as_str`] by construction rather than by
    /// coincidence, and named separately because the CALLER's intent differs:
    /// this one is Rust source text and must not drift if the published
    /// spelling ever gains a space.
    pub fn diagnostic_kind_variant(self) -> &'static str {
        self.as_str()
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single error definition
#[derive(Debug, Deserialize)]
pub struct ErrorDefinition {
    /// Error code: "E241", "E520", etc.
    pub code: SpecErrorCode,
    /// Short name: "IllegalUntranscribed", "SpeakerNotInParticipants", etc.
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// How to fix the error
    pub suggestion: String,
    /// URL to documentation
    #[serde(default)]
    pub help_url: Option<String>,
    /// References this error needs for message generation
    pub references: ErrorReference,
    /// Bad examples that trigger this error
    pub examples: Vec<ErrorExample>,
}

/// Declares which source spans and contextual data an error message needs.
///
/// Generators use these flags to emit the correct `ErrorReference` construction
/// in Rust code. The `additional` map provides forward-compatible extensibility
/// for new reference kinds without changing the struct.
#[derive(Debug, Deserialize, Default)]
pub struct ErrorReference {
    // -- Common references --
    /// The span of the offending word.
    #[serde(default)]
    pub word_span: bool,
    /// The textual content of the offending word.
    #[serde(default)]
    pub word_text: bool,
    /// The span of the containing dependent tier line.
    #[serde(default)]
    pub tier_span: bool,
    /// The span of the containing utterance (main tier + dependents).
    #[serde(default)]
    pub utterance_span: bool,

    // -- Type-specific references --
    /// The specific illegal character that caused the error.
    #[serde(default)]
    pub illegal_char: bool,
    /// Byte offset of the illegal character within the word.
    #[serde(default)]
    pub char_position: bool,
    /// Three-letter speaker code (e.g. `CHI`, `MOT`).
    #[serde(default)]
    pub speaker_code: bool,
    /// The span of the `@Participants` header (for cross-reference labels).
    #[serde(default)]
    pub participants_span: bool,

    /// Catch-all for reference kinds not yet promoted to named fields.
    #[serde(flatten)]
    pub additional: HashMap<String, bool>,
}

/// A bad example that triggers an error
#[derive(Debug, Deserialize)]
pub struct ErrorExample {
    /// The input that triggers the error
    pub input: String,
    /// Expected error codes
    #[serde(default)]
    pub expected_codes: Vec<SpecErrorCode>,
    /// The fixture path the example was taken from, as its `**Source**` line
    /// gives it, when it has one.
    ///
    /// Its STEM is the transcript's name, which decides whether rules about
    /// the file's own name run (E531). An example with no `**Source**` is
    /// genuinely anonymous.
    #[serde(default)]
    pub source: Option<String>,
    /// Expected error message (or substring)
    pub expected_message: String,
    /// Optional labels for multi-span errors
    #[serde(default)]
    pub expected_labels: Vec<ErrorLabel>,
}

/// A label for multi-span errors
#[derive(Debug, Deserialize)]
pub struct ErrorLabel {
    /// Which span: "utterance", "participants", etc.
    pub span: String,
    /// Label text: "speaker used here", "@Participants declared here", etc.
    pub text: String,
}

/// Does a following sibling of this code block declare expected codes?
///
/// The loader reads `**Expected Error Codes**` from the siblings BEFORE a code
/// fence. A spec that puts the line after the fence therefore declares nothing,
/// while reading, to a human, as fully specified. This detects that so the
/// loader can refuse it instead of silently accepting an example that cannot
/// fail.
fn raw_after_fence_declares_codes(node: &comrak::nodes::AstNode<'_>) -> bool {
    let mut next = node.next_sibling();
    while let Some(sibling) = next {
        if let comrak::nodes::NodeValue::Heading(heading) = sibling.data.borrow().value
            && heading.level <= 2
        {
            return false;
        }
        if extract_text_from_children(sibling).contains("Expected Error Codes:") {
            return true;
        }
        next = sibling.next_sibling();
    }
    false
}

impl ErrorSpec {
    /// Load an error specification from a Markdown file
    /// # Errors do NOT name the file
    ///
    /// [`Self::load_all`] prefixes every failure with `Failed to load {path}:`,
    /// so naming the path here would print it twice. That was got wrong eight
    /// times in this function before 2026-08-15, including twice by the commits
    /// that documented the rule, which is why `load` is private: `load_all` is
    /// the only route in, so there is no caller that could need the path and
    /// not get it.
    fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| format!("failed to read: {e}"))?;

        // Parse to AST
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, &content, &comrak::Options::default());

        let mut name = String::new();
        let mut description = String::new();
        let mut examples = Vec::new();
        let mut metadata = std::collections::HashMap::new();

        let mut found_h1 = false;
        let mut current_h2 = String::new();

        // Walk the AST
        for node in root.descendants() {
            let node_data = node.data.borrow();

            match &node_data.value {
                // H1 heading - extract code and name
                comrak::nodes::NodeValue::Heading(heading) if heading.level == 1 && !found_h1 => {
                    name = extract_text_from_children(node);
                    found_h1 = true;
                }

                // H2 heading
                comrak::nodes::NodeValue::Heading(heading) if heading.level == 2 => {
                    current_h2 = normalize_whitespace(&extract_text_from_children(node));
                }

                // Description paragraph
                comrak::nodes::NodeValue::Paragraph
                    if current_h2 == "Description" && description.is_empty() =>
                {
                    description = normalize_whitespace(&extract_text_from_children(node));
                }

                // Metadata list
                comrak::nodes::NodeValue::List(_) if current_h2 == "Metadata" => {
                    for child in node.children() {
                        if let comrak::nodes::NodeValue::Item(_) = child.data.borrow().value {
                            extract_metadata_from_list_item(child, &mut metadata);
                        }
                    }
                }

                // Example code block
                comrak::nodes::NodeValue::CodeBlock(code_block)
                    if current_h2.starts_with("Example") =>
                {
                    let input = strip_single_trailing_newline(&code_block.literal);

                    // AN ERROR EXAMPLE IS A WHOLE CHAT FILE, AND THE FENCE SAYS
                    // SO. This was a `context: String` carrying the fence info,
                    // which the generator interpolated into
                    // `parser.parse_{context}` with no mapping step. So every
                    // value but `chat` named a `TreeSitterParser` method that does
                    // not exist, including the empty fence (defaulted to
                    // `utterance`, a free function, not a method) and all five
                    // alternatives the format reference documented.
                    //
                    // Measured over all 236 loaded spec files, 2026-08-15: 330 of 330
                    // example fences are `chat`, and all 214 generated tests call
                    // `parse_chat_file`. One reachable value is an invariant, not
                    // data, so it is checked here rather than stored.
                    if code_block.info != "chat" {
                        // `load_all` prefixes the file name, so this must not.
                        return Err(format!(
                            "an example's code fence is ```{} where only ```chat \
                             is supported. An error example is parsed as a whole \
                             CHAT file; there is no other parse entry point a \
                             generated test can call.",
                            code_block.info,
                        ));
                    }

                    // Try to find "Expected Error Codes" and "Source" in the
                    // preceding siblings. `Source` names the fixture the example
                    // came from, and its stem is what the transcript is CALLED:
                    // the rules that compare a transcript against its own file
                    // name (E531) need it, and the runner used to invent a stem
                    // because this line was parsed by nobody.
                    let mut expected_codes = Vec::new();
                    let mut source = None;
                    let mut prev = node.previous_sibling();
                    while let Some(sibling) = prev {
                        let text = extract_text_from_children(sibling);
                        if let Some(pos) = text.find("Source:") {
                            // The FIRST WHITESPACE-DELIMITED TOKEN, not the rest
                            // of the line. Comrak collapses a paragraph's soft
                            // line breaks, so `**Source**`, `**Trigger**` and
                            // `**Expected Error Codes**` arrive as one line and
                            // `lines().next()` swallowed all three. A source is
                            // a path, which never contains a space.
                            let rest = &text[pos + "Source:".len()..];
                            source = rest.split_whitespace().next().map(str::to_string);
                        }
                        if text.contains("Expected Error Codes") {
                            // ONE owner for this line, shared with the sibling
                            // parser of the same files. This used to split on
                            // `,` and require every token to parse, while
                            // `error_corpus.rs` split on non-alphanumerics and
                            // tolerated prose, so `E301 and E305` loaded in one
                            // reader and hard-failed in the other.
                            expected_codes = super::metadata::expected_error_codes(&text)?;
                            break;
                        }
                        // Stop if we hit another H2
                        if let comrak::nodes::NodeValue::Heading(h) = sibling.data.borrow().value
                            && h.level == 2
                        {
                            break;
                        }
                        prev = sibling.previous_sibling();
                    }

                    // A field placed AFTER the code fence is invisible: this
                    // loop reads PRECEDING siblings only. E757 declared its
                    // codes below the fence in two examples, so they were
                    // silently ignored and the examples asserted nothing while
                    // looking fully specified. Refuse that rather than drop it.
                    if expected_codes.is_empty() && raw_after_fence_declares_codes(node) {
                        return Err("an example declares `**Expected Error Codes**` AFTER \
                             its ```chat fence, where the loader cannot see it, so the \
                             example would assert nothing. Move the line above the fence."
                            .to_string());
                    }

                    examples.push(ErrorExample {
                        input,
                        expected_codes,
                        source,
                        expected_message: String::new(),
                        expected_labels: Vec::new(),
                    });
                }

                _ => {}
            }
        }

        // The code comes from the `- **Error Code**:` bullet, or from the
        // heading, which `metadata::parse_spec_title` owns for both parsers of
        // these files. Neither route may FABRICATE one.
        //
        // This chain used to end `.unwrap_or_default()`, so a spec with no
        // bullet and an empty H1 was given the EMPTY STRING as its error code,
        // which reached `ErrorCode::new("")` inside a generated test and became
        // the `Unknown` sentinel. Measured 2026-08-15 over all 236 loaded
        // specs: 233 use the bullet, 3 use the heading, none is malformed. So
        // this is PREVENTION, like `Span`'s `Default` removal.
        //
        // The heading route used to be weaker than the sibling parser's; see
        // `metadata::parse_spec_title`, which now owns it for both.
        let title = super::metadata::parse_spec_title(&name);
        // The bullet wins where both are present: 233 of 236 specs declare it,
        // and it is the field an author edits deliberately. A tuple match over
        // (bullet, heading) was tried and reverted: its first arm discarded the
        // second component with `_`, so it read as a cross-product while
        // deciding on one value, and bought no exhaustiveness.
        let code: SpecErrorCode = match metadata.get("Error Code") {
            Some(declared) => declared
                .parse()
                .map_err(|why| format!("`Error Code` {why}"))?,
            None => title.code.clone().ok_or_else(|| {
                format!(
                    "no `- **Error Code**:` bullet, and the heading {name:?} \
                     names no code either, so this file declares none at all."
                )
            })?,
        };

        // Required: a spec file with no `- **Kind**:` bullet, or an
        // unrecognized value, fails to load rather than silently defaulting.
        // See `ErrorMetadata::kind` for why this must never become optional.
        let kind_str = metadata.get("Kind").ok_or_else(|| {
            "missing required Kind metadata (must be one of: Invalidity, \
             Unmodeled, Deprecation, Style)"
                .to_string()
        })?;
        let kind = ErrorKind::parse(kind_str).map_err(|e| e.to_string())?;

        let error_def = ErrorDefinition {
            code: code.clone(),
            name: title.name.clone(),
            description: description.clone(),
            suggestion: String::new(), // TODO
            help_url: None,
            references: ErrorReference::default(),
            examples,
        };

        // Both metadata vocabularies are resolved BEFORE the literal, matching
        // `kind` twenty lines above and matching `error_corpus.rs`, the sibling
        // parser of these same files. They were briefly inline block expressions
        // inside the literal, which meant reading a nested closure and a
        // multi-line format string while still tracking which field you were in.
        //
        // An absent `Layer` bullet means parser, which is what the sibling parser
        // has always defaulted to. A PRESENT but misspelled value is an error,
        // which it was not before; those were one case and are now two.
        let layer = match metadata.get("Layer") {
            Some(raw) => raw.parse::<SpecLayer>().map_err(|why| why.to_string())?,
            None => SpecLayer::Parser,
        };
        let status = metadata
            .get("Status")
            .ok_or_else(|| {
                "no `- **Status**:` bullet. Every spec must declare one \
                 (implemented / not_implemented / deprecated / \
                 unreachable_from_chat). This used to default to `implemented`, \
                 so a spec that said nothing had an answer invented for it, and \
                 104 of the 238 files then in spec/errors declared nothing."
                    .to_string()
            })?
            .parse::<Status>()
            .map_err(|why| why.to_string())?;

        Ok(ErrorSpec {
            metadata: ErrorMetadata {
                // REQUIRED, like `Kind` and `Status`. This was
                // `.unwrap_or_default()`, so a spec declaring no category was
                // given the empty string and published as a `## ` heading with
                // no name. All 236 loaded specs declare one, so this is
                // prevention; the sibling parser has always required it.
                category: metadata
                    .get("Category")
                    .ok_or_else(|| {
                        "no `- **Category**:` bullet. Every spec must declare one.".to_string()
                    })?
                    .parse()
                    .map_err(|why| format!("`Category` {why}"))?,
                layer,
                description: description.clone(),
                status,
                kind,
            },
            errors: vec![error_def],
            source_file: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// Load all error specifications from a directory
    pub fn load_all(root: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let root = root.as_ref();
        let mut specs = Vec::new();
        let mut issues = Vec::new();

        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut paths: Vec<_> = walkdir::WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(e) => Some(e),
                Err(err) => {
                    issues.push(format!("WalkDir error: {}", err));
                    None
                }
            })
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
            .filter(|entry| {
                let file_name = entry.file_name().to_str().unwrap_or_default();
                !file_name.starts_with('_')
                    && file_name != "README.md"
                    && file_name != "SPEC_ENHANCEMENT_GUIDE.md"
            })
            .map(|entry| entry.into_path())
            .collect();
        paths.sort();

        for path in &paths {
            match Self::load(path) {
                Ok(spec) => specs.push(spec),
                Err(err) => issues.push(format!("Failed to load {}: {}", path.display(), err)),
            }
        }

        // A load failure (missing/invalid Kind, malformed metadata, a WalkDir
        // error) must actually fail the whole load: this used to be collected
        // into `issues` and then silently discarded (the `println!` below was
        // commented out), which meant a spec file that failed to parse was
        // just dropped from the result with NO signal to the caller. Every
        // caller of `load_all` already propagates a `Result` with `?`, so
        // surfacing failures here costs nothing and closes that hole. This is
        // also what makes `ErrorMetadata::kind` genuinely REQUIRED rather
        // than "required unless the loader swallows the error."
        if issues.is_empty() {
            Ok(specs)
        } else {
            Err(issues.join("\n"))
        }
    }
}

/// Extract metadata key-value pairs from a list item
fn extract_metadata_from_list_item<'a>(
    list_item: &'a comrak::nodes::AstNode<'a>,
    metadata: &mut std::collections::HashMap<String, String>,
) {
    let text = extract_text_from_children(list_item);
    if let Some((key, value)) = text.split_once(':') {
        metadata.insert(normalize_whitespace(key), normalize_whitespace(value));
    }
}

impl ErrorExample {
    /// The transcript's name, taken from the `**Source**` line's stem.
    ///
    /// `None` when the example declares no source, which is the honest answer:
    /// an example that names no file has no file name, and rules about the
    /// file's name do not apply to it.
    pub fn source_stem(&self) -> Option<&str> {
        self.source
            .as_deref()
            .and_then(|source| source.rsplit('/').next())
            .map(|file| file.strip_suffix(".cha").unwrap_or(file))
            .filter(|stem| !stem.is_empty())
    }

    /// Generate a sanitized name for this example.
    ///
    /// Uses NFKC normalization to convert uncommon codepoints, then
    /// collapses consecutive underscores to avoid non_snake_case warnings.
    pub fn sanitized_name(&self) -> String {
        // Use first few words of input or expected message
        let name = self
            .input
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join("_");
        let filtered: String = name
            .nfkc()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase();
        // Collapse consecutive underscores and trim leading/trailing underscores
        let mut result = String::with_capacity(filtered.len());
        let mut prev_underscore = false;
        for c in filtered.chars() {
            if c == '_' {
                if !prev_underscore && !result.is_empty() {
                    result.push('_');
                }
                prev_underscore = true;
            } else {
                result.push(c);
                prev_underscore = false;
            }
        }
        // Trim trailing underscore
        if result.ends_with('_') {
            result.pop();
        }
        result
    }

    /// Extract expected error message substring for assertion
    pub fn expected_substring(&self) -> &str {
        &self.expected_message
    }
}
