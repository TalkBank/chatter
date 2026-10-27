//! Interned grammatical-relation labels used by `%gra`.
//!
//! Relation labels are interned because `%gra` tiers repeat a small vocabulary
//! (`NSUBJ`, `OBJ`, `ROOT`, etc.) across large corpora.
//! Interning keeps these comparisons pointer-equality cheap during alignment and
//! serialization passes.
//!
//! CHAT reference anchor:
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use talkbank_derive::{SemanticEq, SpanShift};

/// Grammatical relation type (Universal Dependencies).
///
/// Relation labels from the Universal Dependencies framework describing syntactic
/// relationships between words in a sentence.
///
/// A label starts with an ASCII letter and continues with ASCII letters,
/// digits or hyphens. Case and subtype spelling are preserved. Vocabulary
/// policy remains a separate validation rule, so unknown heads are retained.
/// Raw text must pass lexical admission before construction or JSON import.
///
/// ```compile_fail
/// use talkbank_model::model::GrammaticalRelationType;
/// let label: GrammaticalRelationType = "ROOT".into();
/// ```
/// ```compile_fail
/// use talkbank_model::model::GrammaticalRelationType;
/// let label = GrammaticalRelationType::new("ROOT").unwrap();
/// let raw = label.0;
/// ```
///
/// # Common Universal Dependencies Relations
///
/// **Core arguments:**
/// - **ROOT**: Root of the sentence (head = 0)
/// - **NSUBJ**: Subject (nominal subject)
/// - **OBJ**: Direct object
/// - **IOBJ**: Indirect object
/// - **CSUBJ**: Clausal subject
/// - **CCOMP**: Clausal complement
///
/// **Nominal dependents:**
/// - **DET**: Determiner (the, a, this)
/// - **AMOD**: Adjectival modifier
/// - **NMOD**: Nominal modifier
/// - **NUM**: Numeric modifier
/// - **POSS**: Possessive modifier
///
/// **Function words:**
/// - **AUX**: Auxiliary verb
/// - **CASE**: Case marking (prepositions, postpositions)
/// - **MARK**: Marker (subordinating conjunction)
/// - **PUNCT**: Punctuation
///
/// **Other:**
/// - **ADV**: Adverbial modifier
/// - **CONJ**: Conjunct
/// - **CC**: Coordinating conjunction
/// - **NEG**: Negation
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I eat cookies .
/// %mor: pro:sub|I v|eat det:art|the n|cookie-PL .
/// %gra: 1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
/// ```
///
/// # References
///
/// - [Universal Dependencies Relations](https://universaldependencies.org/u/dep/)
/// - [CHAT Manual: Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct GrammaticalRelationType(Arc<str>);

/// Why text cannot be represented by a single grammatical relation label.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidGrammaticalRelationType {
    /// Every relation has a label.
    #[error("grammatical relation label is empty")]
    Empty,
    /// The first character must be an ASCII letter.
    #[error("grammatical relation label starts with non-letter {character:?}")]
    InvalidStart {
        /// The first observed character.
        character: char,
    },
    /// The rest of the label admits only ASCII letters, digits and hyphens.
    #[error("invalid grammatical relation label character {character:?} at byte {byte_offset}")]
    InvalidCharacter {
        /// The refused character.
        character: char,
        /// Its UTF-8 byte offset in the label.
        byte_offset: usize,
    },
}

impl GrammaticalRelationType {
    /// Admit and intern one label without imposing the UD head vocabulary.
    pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidGrammaticalRelationType> {
        let text = text.as_ref();
        let first = text
            .chars()
            .next()
            .ok_or(InvalidGrammaticalRelationType::Empty)?;
        if !first.is_ascii_alphabetic() {
            return Err(InvalidGrammaticalRelationType::InvalidStart { character: first });
        }
        if let Some((byte_offset, character)) = text
            .char_indices()
            .find(|(_, ch)| !ch.is_ascii_alphanumeric() && *ch != '-')
        {
            return Err(InvalidGrammaticalRelationType::InvalidCharacter {
                character,
                byte_offset,
            });
        }
        Ok(Self(crate::model::pos_interner().intern(text)))
    }

    /// The canonical label for an explicitly constructed utterance root.
    pub fn root() -> Self {
        Self(crate::model::pos_interner().intern("ROOT"))
    }

    /// Borrow the admitted label with its original case and subtype intact.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GrammaticalRelationType {
    type Error = InvalidGrammaticalRelationType;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl TryFrom<&str> for GrammaticalRelationType {
    type Error = InvalidGrammaticalRelationType;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl JsonSchema for GrammaticalRelationType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "GrammaticalRelationType".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string", "minLength": 1, "pattern": "^[A-Za-z]",
            "not": { "pattern": "[^A-Za-z0-9-]" }
        })
    }
}

impl crate::model::WriteChat for GrammaticalRelationType {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.as_str())
    }
}

impl std::fmt::Display for GrammaticalRelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::ops::Deref for GrammaticalRelationType {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for GrammaticalRelationType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for GrammaticalRelationType {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn relation_label_wire_refuses_non_tokens() {
        for text in [
            "",
            "ROOT-X Y",
            "ROOT-X|1|0|ROOT",
            "ROOT-X\n@End",
            "ROOT-\0",
            "1ROOT",
            "-ROOT",
            "RÖÖT",
            "ROOT_",
        ] {
            assert!(
                GrammaticalRelationType::new(text).is_err(),
                "constructor accepted {text:?}"
            );
            assert!(
                serde_json::from_value::<GrammaticalRelationType>(serde_json::json!(text)).is_err(),
                "accepted non-token {text:?}"
            );
        }
    }

    #[test]
    fn lexical_admission_preserves_case_custom_subtypes_and_unknown_heads() {
        for text in [
            "ROOT",
            "root",
            "ROOT-x",
            "NSUBJ-CUSTOM-PART",
            "NSUBJJ",
            "A--B",
            "CUSTOM42-",
        ] {
            let label = GrammaticalRelationType::new(text).unwrap();
            assert_eq!(label.as_str(), text);
            assert_eq!(
                serde_json::to_value(&label).unwrap(),
                serde_json::json!(text)
            );
            assert_eq!(
                serde_json::from_value::<GrammaticalRelationType>(serde_json::json!(text)).unwrap(),
                label
            );
        }
        assert_eq!(
            GrammaticalRelationType::root(),
            GrammaticalRelationType::new("ROOT").unwrap()
        );
    }

    /// Identical labels are interned to the same backing allocation.
    ///
    /// Pointer equality keeps hot-path comparisons cheap in parser/model code.
    #[test]
    fn test_gra_type_interning() {
        let root1 = GrammaticalRelationType::new("ROOT").unwrap();
        let root2 = GrammaticalRelationType::new("ROOT").unwrap();

        // Same Arc (pointer equality) - strings should be interned
        assert!(Arc::ptr_eq(&root1.0, &root2.0));
        assert_eq!(root1.as_str(), "ROOT");
        assert_eq!(root2.as_str(), "ROOT");
    }

    /// Distinct labels keep distinct interned allocations.
    ///
    /// This prevents accidental aliasing across unrelated dependency labels.
    #[test]
    fn test_gra_type_different_values() {
        let root = GrammaticalRelationType::new("ROOT").unwrap();
        let subj = GrammaticalRelationType::new("NSUBJ").unwrap();

        // Different values - different Arcs
        assert!(!Arc::ptr_eq(&root.0, &subj.0));
        assert_eq!(root.as_str(), "ROOT");
        assert_eq!(subj.as_str(), "NSUBJ");
    }

    /// Display output preserves the original relation label text.
    ///
    /// This is required for lossless `%gra` roundtripping.
    #[test]
    fn test_gra_type_display() {
        let rel = GrammaticalRelationType::new("OBJ").unwrap();
        assert_eq!(rel.to_string(), "OBJ");
        assert_eq!(format!("{}", rel), "OBJ");
    }
}
