//! The per-CODE registry: every fact that is true of an error CODE, as
//! distinct from a fact about one document describing it.
//!
//! # The distinction this file exists to draw
//!
//! `spec/errors/E202_missing_form_type.md` is a DOCUMENT. Its name, its
//! description, its examples and its notes belong to it, and a code may have
//! several such documents: eleven codes do. But a code has facts of its own
//! that no document can own, because two documents about one code would each
//! have to state them and could then disagree:
//!
//! | fact | before this registry |
//! |---|---|
//! | the Rust variant it compiles to | only in `ErrorCode`, hand-written |
//! | its one-line rustdoc | only in `ErrorCode`, hand-written |
//! | its [`ErrorKind`] | in EVERY spec file for the code, held together by a runtime agreement check that bailed on disagreement |
//! | its [`Status`] | in EVERY spec file, mirrored onto the enum as `#[status(planned)]`, reconciled by a 180-line gate |
//! | that a retired number is never reused | a prose comment in the enum |
//!
//! Every row was a copy, and four of the five had a check standing where a
//! single owner belongs. `E241`'s two spec files disagree about `name`
//! (`Auto-generated from corpus` against `xx`) and `E519`'s three all differ,
//! which is fine for a per-document label and is exactly why per-CODE facts
//! could never be read off one.
//!
//! # What this deletes
//!
//! The `spec_status` gate, both directions of the `spec/errors <-> ErrorCode`
//! divergence check, the per-code `kind` agreement loop, and the enum's
//! retired-number comment. Each becomes a state that cannot be written rather
//! than a check that must be run.
//!
//! # The one thing it does NOT own
//!
//! Whether a code is DOCUMENTED. That was previously entangled with the
//! vocabulary question ("this variant has no spec file" read as a divergence);
//! it is really a coverage question, and it stays one, asked by a gate that
//! can now say plainly what it is asking.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

use crate::{SpecErrorCode, Status, UnknownMetadataValue};

/// The registry file's location under the repository root.
///
/// A constant, not a caller's argument, for the reason the artifact registry
/// states about destinations: a path that can be passed can be passed wrongly,
/// and there is exactly one of these files.
pub const REGISTRY_PATH: &str = "spec/codes/error-codes.toml";

/// The four `DiagnosticKind` axis values a code can be classified as.
///
/// Mirrors `talkbank_model::errors::DiagnosticKind` structurally by name.
/// This crate cannot depend on `talkbank-model` (that would be circular:
/// `talkbank-model`'s own diagnostic-kind registry is generated FROM this
/// registry, by a binary in `spec/runtime-tools`, which is the one place both
/// directions of the dependency meet). The generator maps each variant here to
/// the identically-named `DiagnosticKind` variant by name; a variant added to
/// one and not the other is caught at the generator's match, not silently
/// ignored.
///
/// # Why it moved here from the spec loader
///
/// It was the fourth closed vocabulary of the spec format and the only one not
/// living beside the other three, which forced `SpecFrontmatter` to carry a
/// `Kind` type parameter purely so the format could avoid naming a type it
/// could not reach. With `kind` a per-CODE fact, the parameter has nothing
/// left to abstract over and is gone.
///
/// # Deserialized THROUGH [`FromStr`], not by the derive
///
/// A plain `Deserialize` derive would spell the four variant names a second
/// time, in serde's generated code, where nothing holds them to
/// [`Self::as_str`]. That is the same duplication this type's own doc warns
/// about one paragraph up, so the read route goes through the table rather
/// than beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
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
    /// ONE table, the way [`Status`] already has one.
    ///
    /// This name is FOUR things at once: what the registry's `kind` value must
    /// say, what the generated `DiagnosticKind` registry emits as source text,
    /// what `docs/errors/*.md` publishes, and what the index table shows.
    /// Three of those were separate matches until 2026-08-15, and the
    /// published pair were `{:?}` on the derived `Debug`, so renaming a
    /// variant would have silently changed user-facing documentation while
    /// the generator kept emitting the old literal.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalidity => "Invalidity",
            Self::Unmodeled => "Unmodeled",
            Self::Deprecation => "Deprecation",
            Self::Style => "Style",
        }
    }

    /// The identically-named `talkbank_model::errors::DiagnosticKind` variant
    /// this value maps to, as source text for code generation.
    ///
    /// Identical to [`Self::as_str`] by construction rather than by
    /// coincidence, and named separately because the CALLER's intent differs:
    /// this one is Rust source text and must not drift if the published
    /// spelling ever gains a space.
    #[must_use]
    pub const fn diagnostic_kind_variant(self) -> &'static str {
        self.as_str()
    }
}

impl FromStr for ErrorKind {
    type Err = UnknownMetadataValue;

    /// Case-sensitive and exact: the four spelled-out variant names, nothing
    /// else. A plain match, the shape [`Status::from_str`] uses, and for the
    /// reason recorded there: an `ALL` array is a second hand-maintained list
    /// that nothing checks for completeness.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Invalidity" => Ok(Self::Invalidity),
            "Unmodeled" => Ok(Self::Unmodeled),
            "Deprecation" => Ok(Self::Deprecation),
            "Style" => Ok(Self::Style),
            other => Err(UnknownMetadataValue::new("Kind", other)),
        }
    }
}

impl TryFrom<String> for ErrorKind {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The Rust identifier an [`ErrorCode`](crate::SpecErrorCode) variant compiles
/// to.
///
/// # Every route in, enumerated
///
/// [`FromStr`], and `Deserialize` routed through `TryFrom<String>` so a value
/// read from the registry file is held to the same rule as one parsed from
/// text. There is no third route and no `pub` field: possession of a
/// `VariantName` PROVES the string is a legal Rust identifier in
/// `UpperCamelCase`, which is what lets the generator emit it into source
/// without a further check. A newtype whose invariant any caller could assert
/// from the raw parts would be a label rather than a proof.
///
/// The rule is deliberately narrower than Rust's: an ASCII uppercase letter
/// followed by ASCII alphanumerics. That admits no underscore, no keyword and
/// no raw identifier, so there is no case in which the emitted token needs
/// escaping. All 224 seeded variants satisfy it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct VariantName(String);

impl VariantName {
    /// The identifier, ready to emit as Rust source.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for VariantName {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let name = value.trim();
        let mut chars = name.chars();
        let leads = chars.next().is_some_and(|c| c.is_ascii_uppercase());
        if leads && chars.all(|c| c.is_ascii_alphanumeric()) {
            return Ok(Self(name.to_owned()));
        }
        Err(UnknownMetadataValue::new("variant", name))
    }
}

impl TryFrom<String> for VariantName {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl fmt::Display for VariantName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A code's rustdoc, as the generated enum will emit it.
///
/// # Why this is not a "one-line summary"
///
/// It was, for about an hour, and the JSON schema caught it. Several codes
/// carry genuinely multi-paragraph rustdoc (E311 explains why it stopped being
/// planned; E377 explains why it is named for a shape rather than a spelling),
/// and `schema/chat-file.schema.json` embeds every variant's doc verbatim. A
/// one-line rule flattened those paragraphs into run-on sentences, silently:
/// the text was still there, the structure was not.
///
/// That is the lossy-round-trip shape. The cure is not to widen the rule and
/// hope, it is to make the type hold what the source holds: arbitrary rustdoc,
/// normalized to `\n`, emitted one `///` line at a time by
/// [`crate::registry`]'s consumer. Emptiness stays refused, for the reason
/// [`crate::SpecDescription`] refuses it: an empty doc comment reads as a
/// documented variant.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Rustdoc(String);

impl Rustdoc {
    /// The doc text, non-empty, with `\n` line separators and no trailing
    /// newline.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The lines to emit, each of which becomes one `///` comment line.
    ///
    /// A blank line here is a paragraph break, and emits as a bare `///`.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.0.split('\n')
    }
}

impl FromStr for Rustdoc {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // CRLF is normalized rather than refused: a registry edited on Windows
        // is not a different document, and the emitted source must be LF
        // either way.
        let text = value.replace("\r\n", "\n");
        let text = text.trim();
        if text.is_empty() {
            return Err(UnknownMetadataValue::new("summary", value));
        }
        Ok(Self(text.to_owned()))
    }
}

impl TryFrom<String> for Rustdoc {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl fmt::Display for Rustdoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One code's per-code facts: the complete set, and the only place they live.
///
/// # Every route in, enumerated
///
/// `Deserialize`, reached only from [`CodeRegistry`]'s own private
/// `RegistryFile`, so [`CodeRegistry::parse`] is the only way one comes into
/// existence and [`CodeRegistry::resolve`] and [`CodeRegistry::entries`] are
/// the only ways to reach one.
///
/// The fields were `pub` for an afternoon, which made a struct literal a
/// sixth route and `toml::from_str::<CodeEntry>` a seventh. Neither could
/// reach a consumer, because the three holders are private fields; but two
/// docstrings already described possession of one as PROOF that a code is
/// registered, and that proof rested on module privacy elsewhere rather than
/// on this type. A constructor taking the type's own fields as arguments is
/// the repo's named tell for a forgeable proof, so the doors are closed and
/// the claim is true where it is made.
///
/// Contrast `form_markers::registry::MarkerRow`, which keeps nine `pub`
/// fields and is right to: nothing treats a `MarkerRow` as proof of anything,
/// it is data a renderer walks. That is the line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeEntry {
    code: SpecErrorCode,
    variant: VariantName,
    summary: Rustdoc,
    kind: ErrorKind,
    status: Status,
}

impl CodeEntry {
    /// The code as written everywhere else: `E202`, `W108`.
    #[must_use]
    pub fn code(&self) -> &SpecErrorCode {
        &self.code
    }

    /// The `ErrorCode` variant it compiles to.
    #[must_use]
    pub fn variant(&self) -> &VariantName {
        &self.variant
    }

    /// The variant's rustdoc, verbatim.
    #[must_use]
    pub fn rustdoc(&self) -> &Rustdoc {
        &self.summary
    }

    /// Which `DiagnosticKind` axis the code reports on.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Whether the validator actually enforces the rule.
    #[must_use]
    pub fn status(&self) -> Status {
        self.status
    }
}

/// A number that was used and must never be reused.
///
/// # This is the prose comment, promoted
///
/// `ErrorCode` carried a 20-line comment block recording that W210, W211,
/// W601, W602 and W999 are retired "and not reused". Prose is gated by
/// nothing: reusing W601 for a new check would have compiled, shipped, and
/// silently changed the meaning of a number in every archived transcript
/// report that mentions it. Here the same fact refuses the file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetiredCode {
    /// The retired number.
    pub code: SpecErrorCode,
    /// Why it was retired, and when. Prose, deliberately: the reasons do not
    /// form a closed set (removed as dead code, renumbered, ruled invalid).
    pub reason: String,
}

/// The registry file failed to load, or violated a rule spanning its entries.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// The file is not well-formed TOML, or violates the per-entry schema.
    #[error("{REGISTRY_PATH}: {0}")]
    Toml(#[from] toml::de::Error),
    /// Two `[[code]]` entries claim the same code.
    #[error("{REGISTRY_PATH}: {code} is registered twice")]
    DuplicateCode {
        /// The code claimed more than once.
        code: SpecErrorCode,
    },
    /// Two codes compile to the same Rust identifier, which would not compile.
    #[error("{REGISTRY_PATH}: {first} and {second} both compile to `{variant}`")]
    DuplicateVariant {
        /// The identifier claimed twice.
        variant: VariantName,
        /// The code that claimed it first, in file order.
        first: SpecErrorCode,
        /// The code that claimed it again.
        second: SpecErrorCode,
    },
    /// A retired number is registered as a live code.
    ///
    /// The check the enum's comment could only ask a reader to perform.
    #[error(
        "{REGISTRY_PATH}: {code} is retired and may never be reused. \
         Retired because: {reason}"
    )]
    ReusedRetiredCode {
        /// The number being reused.
        code: SpecErrorCode,
        /// The retirement's own recorded reason.
        reason: String,
    },
    /// The same number is retired twice.
    #[error("{REGISTRY_PATH}: {code} is retired twice")]
    DuplicateRetirement {
        /// The number listed more than once.
        code: SpecErrorCode,
    },
}

/// A code named by something outside the registry that the registry does not
/// declare.
///
/// Its own error type rather than a `None`, because every caller wants to name
/// the code and the file that named it, and an `Option` re-checked at each of
/// them is the missing type this whole redesign is about.
#[derive(Debug, thiserror::Error)]
#[error("{code} is not declared in {REGISTRY_PATH}")]
pub struct UnregisteredCode {
    /// The code nothing declares.
    pub code: SpecErrorCode,
}

/// The TOML file's shape. Private: it exists only to give serde something to
/// read, and [`CodeRegistry`] is what everything holds.
///
/// The cross-entry rules (uniqueness, retirement) cannot be expressed to
/// serde, so they live in the [`TryFrom`] below, which is the ONLY route from
/// this repr to a `CodeRegistry`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    #[serde(default, rename = "code")]
    codes: Vec<CodeEntry>,
    #[serde(default, rename = "retired")]
    retired: Vec<RetiredCode>,
}

/// Every error code chatter knows, and every number it must never reuse.
///
/// # Every route in, enumerated
///
/// [`CodeRegistry::parse`], which goes through `RegistryFile` and its
/// `TryFrom`. There is no constructor taking entries, so possession of a
/// `CodeRegistry` proves the cross-entry rules hold: no code twice, no
/// identifier twice, no retired number brought back. A constructor taking the
/// parts would let a caller assert exactly the invariants this type exists to
/// carry.
#[derive(Debug)]
pub struct CodeRegistry {
    /// File order, which is the order the generated enum is emitted in, so a
    /// contributor's grouping of the registry file survives into the source.
    ///
    /// There was a `by_code: BTreeMap<SpecErrorCode, usize>` beside this, an
    /// index into a sibling `Vec` that only the constructor kept valid:
    /// `entries[index]` type-checks whether the index belongs to this vector
    /// or not, which is the convention-held-relationship shape in miniature.
    /// [`Self::resolve`] runs once per spec file over ~224 entries, so the
    /// scan costs nothing measurable and one cross-field invariant is gone.
    /// The uniqueness CHECK still needs a set; it is a local in `TryFrom` and
    /// is dropped afterwards, exactly as `by_variant` already was.
    entries: Vec<CodeEntry>,
    retired: Vec<RetiredCode>,
}

impl TryFrom<RegistryFile> for CodeRegistry {
    type Error = RegistryError;

    fn try_from(file: RegistryFile) -> Result<Self, Self::Error> {
        let mut retired_reasons: BTreeMap<SpecErrorCode, String> = BTreeMap::new();
        for entry in &file.retired {
            if retired_reasons
                .insert(entry.code.clone(), entry.reason.clone())
                .is_some()
            {
                return Err(RegistryError::DuplicateRetirement {
                    code: entry.code.clone(),
                });
            }
        }

        let mut seen: BTreeMap<SpecErrorCode, ()> = BTreeMap::new();
        let mut by_variant: BTreeMap<VariantName, SpecErrorCode> = BTreeMap::new();
        for entry in &file.codes {
            if let Some(reason) = retired_reasons.get(&entry.code) {
                return Err(RegistryError::ReusedRetiredCode {
                    code: entry.code.clone(),
                    reason: reason.clone(),
                });
            }
            if seen.insert(entry.code.clone(), ()).is_some() {
                return Err(RegistryError::DuplicateCode {
                    code: entry.code.clone(),
                });
            }
            if let Some(first) = by_variant.insert(entry.variant.clone(), entry.code.clone()) {
                return Err(RegistryError::DuplicateVariant {
                    variant: entry.variant.clone(),
                    first,
                    second: entry.code.clone(),
                });
            }
        }

        Ok(Self {
            entries: file.codes,
            retired: file.retired,
        })
    }
}

/// The registry file could not be read from disk.
#[derive(Debug, thiserror::Error)]
pub enum RegistryLoadError {
    /// The file is absent or unreadable.
    ///
    /// Never treated as "no codes". An empty registry would generate an empty
    /// enum and every gate downstream would then agree with it, which is the
    /// shape of failure this whole subsystem exists to remove.
    #[error("cannot read {path}: {source}")]
    Unreadable {
        /// The path that was tried, resolved.
        path: String,
        /// The underlying IO failure.
        #[source]
        source: std::io::Error,
    },
    /// The file was read and violates the format or a cross-entry rule.
    #[error(transparent)]
    Invalid(#[from] RegistryError),
}

impl CodeRegistry {
    /// Read the registry from its one location under `repo_root`.
    ///
    /// # Errors
    ///
    /// When the file cannot be read, or fails [`Self::parse`].
    pub fn load(repo_root: &std::path::Path) -> Result<Self, RegistryLoadError> {
        let path = repo_root.join(REGISTRY_PATH);
        let text =
            std::fs::read_to_string(&path).map_err(|source| RegistryLoadError::Unreadable {
                path: path.display().to_string(),
                source,
            })?;
        Ok(Self::parse(&text)?)
    }

    /// Read the registry from the file's text.
    ///
    /// # Errors
    ///
    /// When the TOML is malformed, an entry violates its schema, or the file
    /// breaks a rule spanning entries: a code or identifier claimed twice, or
    /// a retired number brought back.
    pub fn parse(text: &str) -> Result<Self, RegistryError> {
        let file: RegistryFile = toml::from_str(text)?;
        Self::try_from(file)
    }

    /// The entry for a code, or an error naming the code.
    ///
    /// # Errors
    ///
    /// When nothing in the registry declares `code`.
    pub fn resolve(&self, code: &SpecErrorCode) -> Result<&CodeEntry, UnregisteredCode> {
        self.entries
            .iter()
            .find(|entry| entry.code == *code)
            .ok_or_else(|| UnregisteredCode { code: code.clone() })
    }

    /// Every registered code, in file order.
    ///
    /// File order rather than sorted, because the file's grouping is a
    /// contributor's editorial choice (the `E2xx` word codes together, and so
    /// on) and the generated enum should read the way the registry does.
    pub fn entries(&self) -> &[CodeEntry] {
        &self.entries
    }

    /// Every retired number, in file order.
    ///
    /// A slice rather than an `impl ExactSizeIterator`, because
    /// `ExactSizeIterator::is_empty` is unstable and the opaque return forced
    /// the caller to write `.len() > 0`. A slice hides the field just as well
    /// and is strictly more capable.
    pub fn retired(&self) -> &[RetiredCode] {
        &self.retired
    }
}

#[cfg(test)]
mod tests {
    use super::{CodeRegistry, ErrorKind, RegistryError, Rustdoc, VariantName};
    use crate::{SpecErrorCode, Status};

    /// A minimal well-formed registry, for the rules that need a baseline.
    fn two_codes() -> &'static str {
        "[[code]]\ncode = 'E001'\nvariant = 'InternalError'\n\
         summary = 'Internal error.'\nkind = 'Invalidity'\nstatus = 'implemented'\n\
         [[code]]\ncode = 'E003'\nvariant = 'EmptyString'\n\
         summary = 'Input string is empty.'\nkind = 'Invalidity'\n\
         status = 'not_implemented'\n"
    }

    #[test]
    fn entries_keep_file_order_and_resolve_by_code() {
        let registry = CodeRegistry::parse(two_codes()).expect("well-formed");
        let codes: Vec<&str> = registry
            .entries()
            .iter()
            .map(|e| e.code().as_str())
            .collect();
        assert_eq!(codes, ["E001", "E003"]);

        let empty = SpecErrorCode::parse("E003").expect("well-formed code");
        let entry = registry.resolve(&empty).expect("registered");
        assert_eq!(entry.variant().as_str(), "EmptyString");
        assert_eq!(entry.status(), Status::NotImplemented);
        assert_eq!(entry.kind(), ErrorKind::Invalidity);
    }

    /// A code nothing declares is an ERROR naming the code, not a `None` for
    /// every caller to re-decide.
    #[test]
    fn an_unregistered_code_names_itself() {
        let registry = CodeRegistry::parse(two_codes()).expect("well-formed");
        let missing = SpecErrorCode::parse("E999").expect("well-formed code");
        let why = registry.resolve(&missing).expect_err("not registered");
        assert!(why.to_string().contains("E999"), "{why}");
    }

    /// The rule the enum's prose comment could only ask a reader to keep.
    /// W601 was renumbered to E756 on 2026-07-16; bringing it back would
    /// silently change what the number means in every archived report.
    #[test]
    fn a_retired_number_cannot_be_reused() {
        let text = "[[retired]]\ncode = 'W601'\nreason = 'renumbered to E756'\n\
                    [[code]]\ncode = 'W601'\nvariant = 'EmptyDependentTier'\n\
                    summary = 'Empty dependent tier.'\nkind = 'Invalidity'\n\
                    status = 'implemented'\n";
        let why = CodeRegistry::parse(text).expect_err("W601 is retired");
        assert!(
            matches!(why, RegistryError::ReusedRetiredCode { .. }),
            "{why:?}"
        );
        assert!(why.to_string().contains("renumbered to E756"), "{why}");
    }

    /// Two variants with one identifier do not compile, so the registry
    /// refuses them where the message can name both codes rather than letting
    /// rustc report a duplicate in generated source.
    #[test]
    fn two_codes_cannot_share_one_identifier() {
        let text = "[[code]]\ncode = 'E001'\nvariant = 'Same'\nsummary = 'a.'\n\
                    kind = 'Invalidity'\nstatus = 'implemented'\n\
                    [[code]]\ncode = 'E002'\nvariant = 'Same'\nsummary = 'b.'\n\
                    kind = 'Invalidity'\nstatus = 'implemented'\n";
        let why = CodeRegistry::parse(text).expect_err("duplicate identifier");
        assert!(
            matches!(why, RegistryError::DuplicateVariant { .. }),
            "{why:?}"
        );
    }

    #[test]
    fn one_code_cannot_be_registered_twice() {
        let text = "[[code]]\ncode = 'E001'\nvariant = 'One'\nsummary = 'a.'\n\
                    kind = 'Invalidity'\nstatus = 'implemented'\n\
                    [[code]]\ncode = 'E001'\nvariant = 'Two'\nsummary = 'b.'\n\
                    kind = 'Invalidity'\nstatus = 'implemented'\n";
        assert!(matches!(
            CodeRegistry::parse(text),
            Err(RegistryError::DuplicateCode { .. })
        ));
    }

    /// An unknown key is a load error, the property the whole format moved to
    /// TOML for. A WIRE-FORMAT property serde owns, so no type of ours deletes
    /// this test.
    #[test]
    fn an_unknown_key_is_a_load_error() {
        let text = "[[code]]\ncode = 'E001'\nvariant = 'One'\nsummary = 'a.'\n\
                    kind = 'Invalidity'\nstatus = 'implemented'\nseverity = 'high'\n";
        let why = CodeRegistry::parse(text).expect_err("unknown key");
        assert!(why.to_string().contains("severity"), "{why}");
    }

    /// Rustdoc keeps its paragraph structure, which a one-line rule destroyed.
    ///
    /// The regression this pins is real and was caught by the JSON schema:
    /// E311's and E377's multi-paragraph docs were flattened into run-on
    /// sentences by a `Summary` type that refused newlines.
    #[test]
    fn rustdoc_keeps_its_paragraphs_and_refuses_emptiness() {
        let doc: Rustdoc = "First line.\n\nSecond paragraph.".parse().expect("valid");
        assert_eq!(
            doc.lines().collect::<Vec<_>>(),
            ["First line.", "", "Second paragraph."],
            "a blank line is a paragraph break, not something to collapse"
        );
        assert_eq!(
            "a\r\nb".parse::<Rustdoc>().expect("valid").as_str(),
            "a\nb",
            "CRLF is normalized, not refused"
        );
        assert!("   ".parse::<Rustdoc>().is_err());
        assert!("\n\n".parse::<Rustdoc>().is_err());
    }

    /// The identifier rule is narrower than Rust's on purpose: what it admits
    /// never needs escaping.
    ///
    /// It deliberately does NOT try to tell an identifier from a code: `E202`
    /// satisfies both rules and is a legal variant name. Stopping two codes
    /// from compiling to one identifier is
    /// [`RegistryError::DuplicateVariant`]'s job, and asking this type to
    /// guess intent from spelling would be a second, weaker answer to it.
    #[test]
    fn a_variant_name_is_an_upper_camel_ascii_identifier() {
        assert!("MissingFormType".parse::<VariantName>().is_ok());
        assert!("E202".parse::<VariantName>().is_ok(), "a legal identifier");
        assert!("2Missing".parse::<VariantName>().is_err(), "leading digit");
        assert!("missingFormType".parse::<VariantName>().is_err());
        assert!("Missing_Form".parse::<VariantName>().is_err());
        assert!("Missing Form".parse::<VariantName>().is_err());
        assert!("".parse::<VariantName>().is_err());
    }
}
