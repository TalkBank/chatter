//! The typed model of `spec/form_markers/form_marker_registry.json`.
//!
//! # Why this is types rather than checks
//!
//! Nearly everything this module used to have to verify is now impossible to
//! express. A marker code is a [`MarkerCode`], which cannot hold `@k` or an
//! empty string, because its only constructor is a `TryFrom` that refuses
//! them. Whether a marker takes a `:label` is a [`LabelPolicy`], so a
//! label-free marker has nowhere to put an example label. And the example is
//! DERIVED from the stem and the marker rather than stored, so the commonest
//! mistake in a table like this, an example showing the neighbouring row's
//! marker, has no way to happen.
//!
//! What is left is the handful of facts that relate rows to each other:
//! uniqueness of the marker, the variant name and the manual anchor. Those are
//! checked once, at load, and a [`FormMarkerRegistry`] is the evidence that
//! they hold.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

/// Anything that can go wrong turning the registry file into a
/// [`FormMarkerRegistry`].
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("cannot read the form-marker registry at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the form-marker registry is not valid JSON for this model: {source}")]
    Parse {
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "marker code {value:?} must be one or more lowercase ASCII letters, with no `@` and no label"
    )]
    MarkerCode { value: String },

    #[error("variant name {value:?} must be a Rust enum variant name (upper camel, ASCII)")]
    VariantName { value: String },

    #[error(
        "example stem {value:?} must not be empty or contain `@`: the example is built as `<stem>@<marker>`, so a stem carrying its own `@` would produce two markers"
    )]
    ExampleStem { value: String },

    #[error(
        "example label {value:?} must be one or more of [A-Za-z0-9_], which is what the grammar accepts after `:`"
    )]
    ExampleLabel { value: String },

    #[error("{field} must not be empty")]
    Empty { field: &'static str },

    #[error("two rows share the {field} {value:?}")]
    Duplicate { field: &'static str, value: String },

    #[error("the registry declares no markers")]
    NoMarkers,

    #[error(
        "registry version {found} is not supported; this loader understands version {supported}"
    )]
    UnsupportedVersion { found: u32, supported: u32 },
}

/// A non-empty string newtype whose only invariant is that it is non-empty.
///
/// Declared by macro because writing five of these out is five chances to get
/// one wrong, and the point of the distinct types is that a gloss cannot be
/// passed where an anchor is wanted, not that each has bespoke parsing.
macro_rules! non_empty_text {
    ($(#[$meta:meta])* $name:ident, $field:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);

        impl TryFrom<String> for $name {
            type Error = RegistryError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                if value.trim().is_empty() {
                    return Err(RegistryError::Empty { field: $field });
                }
                Ok(Self(value))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl $name {
            /// Borrow the text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

non_empty_text!(
    /// What the marker means, as the CHAT manual's "Categories" column gives it.
    Gloss,
    "gloss"
);
non_empty_text!(
    /// The `id` of the marker's own anchor in the CHAT manual, e.g.
    /// `Kana_Marker`. Every site that links to the manual links to the
    /// per-marker anchor rather than the section, so a reader lands on the row
    /// rather than the table.
    ManualAnchor,
    "manual_anchor"
);
non_empty_text!(
    /// A clause that reads on after the example, e.g. "meaning sticky", so
    /// every site can render "(`gumma@c`, meaning sticky)" from one field.
    ExampleNote,
    "example_note"
);
non_empty_text!(
    /// What to use instead of a deprecated marker, e.g. `&-um`.
    Replacement,
    "deprecated.use_instead"
);
non_empty_text!(
    /// Why a marker is deprecated.
    DeprecationReason,
    "deprecated.reason"
);
non_empty_text!(
    /// A note recording where the CHAT manual disagrees with what chatter
    /// accepts, and which authority settled it.
    ManualDisagreement,
    "manual_disagreement"
);

/// A marker code as written after `@`, without the `@` and without any label:
/// `b`, `fp`, `sas`.
///
/// Every marker in the corpus authority's sanctioned list is lowercase ASCII,
/// so the type says so, which is why no generator has to ask whether a code
/// needs escaping for Rust, for re2c or for Markdown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct MarkerCode(String);

impl TryFrom<String> for MarkerCode {
    type Error = RegistryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_lowercase()) {
            return Err(RegistryError::MarkerCode { value });
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for MarkerCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl MarkerCode {
    /// Borrow the code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name of the Rust enum variant this marker becomes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct VariantName(String);

impl TryFrom<String> for VariantName {
    type Error = RegistryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut chars = value.chars();
        let shaped = matches!(chars.next(), Some(first) if first.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_alphanumeric());
        if !shaped {
            return Err(RegistryError::VariantName { value });
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for VariantName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl VariantName {
    /// Borrow the name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The word an example is built on: the `gumma` of `gumma@c`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct ExampleStem(String);

impl TryFrom<String> for ExampleStem {
    type Error = RegistryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.contains('@') {
            return Err(RegistryError::ExampleStem { value });
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for ExampleStem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The label an example puts after the colon: the `rtfd` of `word@z:rtfd`.
///
/// Constrained to what the grammar accepts after `:`, so an example cannot
/// show a label the parser would reject.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct ExampleLabel(String);

impl TryFrom<String> for ExampleLabel {
    type Error = RegistryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(RegistryError::ExampleLabel { value });
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for ExampleLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a marker takes a `:label`, and if it does, the label its example
/// shows.
///
/// `depfile.cut` encodes this distinction structurally, listing bare `*@x`
/// beside `*@s:*` and `*@z:*`, and it is the fact that decides the shape of
/// the generated Rust variant: [`LabelPolicy::Required`] becomes a variant
/// carrying a `String`, [`LabelPolicy::Forbidden`] a unit variant.
///
/// The example label lives INSIDE the `Required` variant rather than beside
/// the policy as its own field. A field that is meaningful only when another
/// field has a particular value is the shape this whole registry exists to
/// remove; here it would have allowed a label-free marker to carry an example
/// label that nothing would ever render.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelPolicy {
    /// The marker is written bare: `stuff@x`. Writing `@x:foo` is E203.
    Forbidden,
    /// The marker requires a label: `word@z:rtfd`.
    Required {
        /// The label the generated example shows.
        example_label: ExampleLabel,
    },
}

/// A marker that should no longer be used, and what replaces it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deprecation {
    /// What to write instead.
    pub use_instead: Replacement,
    /// Why, in one clause.
    pub reason: DeprecationReason,
}

/// One marker.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkerRow {
    /// The code written after `@`.
    pub marker: MarkerCode,
    /// The Rust enum variant it becomes.
    pub variant: VariantName,
    /// What it means.
    pub gloss: Gloss,
    /// Its own anchor in the CHAT manual.
    pub manual_anchor: ManualAnchor,
    /// The word the example is built on.
    pub example_stem: ExampleStem,
    /// A clause that reads on after the example, when the manual gives one.
    pub example_note: Option<ExampleNote>,
    /// Whether it takes a `:label`.
    pub label: LabelPolicy,
    /// Set when the marker is deprecated.
    #[serde(default)]
    pub deprecated: Option<Deprecation>,
    /// Set where the CHAT manual disagrees with what chatter accepts.
    #[serde(default)]
    pub manual_disagreement: Option<ManualDisagreement>,
}

impl MarkerRow {
    /// The example, built from the stem and this row's own marker and label
    /// policy.
    ///
    /// Derived rather than stored: a stored example is free to name a
    /// different marker, and in a table of twenty-two near-identical rows that
    /// is the mistake to expect. There is nothing here for a reviewer to check.
    pub fn example(&self) -> String {
        match &self.label {
            LabelPolicy::Forbidden => format!("{}@{}", self.example_stem, self.marker),
            LabelPolicy::Required { example_label } => {
                format!("{}@{}:{}", self.example_stem, self.marker, example_label)
            }
        }
    }

    /// How the marker is written when naming it rather than using it: `@k`, or
    /// `@z:<label>` for one that requires a label.
    pub fn marker_display(&self) -> String {
        match &self.label {
            LabelPolicy::Forbidden => format!("@{}", self.marker),
            LabelPolicy::Required { .. } => format!("@{}:<label>", self.marker),
        }
    }
}

/// The registry file's shape, before cross-row checks.
///
/// `description` and `authorities` are prose for whoever opens the JSON. They
/// are declared so `deny_unknown_fields` still catches a typo, read with
/// `IgnoredAny` so nothing is built from them, and named with a leading
/// underscore so the compiler knows that is deliberate. They were `String` and
/// `serde_json::Value` behind three `#[allow(dead_code)]` attributes, which
/// allocated a whole subtree nothing reads and needed prose to explain itself.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    /// Checked, not ignored: a field that looks like a compatibility guard and
    /// is not one is worse than no field. A future shape change bumps this and
    /// old readers refuse it instead of silently misreading.
    version: u32,
    #[serde(rename = "description")]
    _description: serde::de::IgnoredAny,
    #[serde(rename = "authorities")]
    _authorities: serde::de::IgnoredAny,
    markers: Vec<MarkerRow>,
}

/// The only registry shape this loader understands.
const SUPPORTED_VERSION: u32 = 1;

/// The marker inventory, with its cross-row invariants established.
///
/// # Constructing one
///
/// [`FormMarkerRegistry::load`] and [`FormMarkerRegistry::from_json`] are the
/// only ways to obtain one, and both check uniqueness before returning. The
/// rows are private, so no caller can assemble a registry from parts and skip
/// the check: holding one IS the evidence.
#[derive(Debug)]
pub struct FormMarkerRegistry {
    markers: Vec<MarkerRow>,
}

impl FormMarkerRegistry {
    /// The registry's path relative to the repository root.
    pub const RELATIVE_PATH: &'static str = "spec/form_markers/form_marker_registry.json";

    /// Read and check the registry under `repo_root`.
    pub fn load(repo_root: &Path) -> Result<Self, RegistryError> {
        let path = repo_root.join(Self::RELATIVE_PATH);
        let text = std::fs::read_to_string(&path)
            .map_err(|source| RegistryError::Read { path, source })?;
        Self::from_json(&text)
    }

    /// Check registry JSON. Separate from [`Self::load`] so tests can feed it
    /// a seeded defect without touching the repository's own file.
    pub fn from_json(text: &str) -> Result<Self, RegistryError> {
        let file: RegistryFile =
            serde_json::from_str(text).map_err(|source| RegistryError::Parse { source })?;

        if file.version != SUPPORTED_VERSION {
            return Err(RegistryError::UnsupportedVersion {
                found: file.version,
                supported: SUPPORTED_VERSION,
            });
        }

        if file.markers.is_empty() {
            return Err(RegistryError::NoMarkers);
        }

        let mut markers = file.markers;
        // Sorted here rather than checked, so the order rows are written in is
        // not a rule anyone has to remember. Every generated site lists markers
        // in this order, so sorting once keeps a new row from landing wherever
        // it was typed and reshuffling four generated files.
        markers.sort_by(|left, right| left.marker.cmp(&right.marker));

        check_unique("marker", markers.iter().map(|row| row.marker.as_str()))?;
        check_unique("variant", markers.iter().map(|row| row.variant.as_str()))?;
        check_unique(
            "manual_anchor",
            markers.iter().map(|row| row.manual_anchor.as_str()),
        )?;

        Ok(Self { markers })
    }

    /// The markers, in code order.
    pub fn markers(&self) -> &[MarkerRow] {
        &self.markers
    }
}

/// Reject two rows claiming the same value for a field that identifies a row.
///
/// Borrows rather than taking `String`, so the happy path allocates nothing and
/// only the error path owns its value.
fn check_unique<'a>(
    field: &'static str,
    values: impl Iterator<Item = &'a str>,
) -> Result<(), RegistryError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(RegistryError::Duplicate {
                field,
                value: value.to_owned(),
            });
        }
    }
    Ok(())
}
