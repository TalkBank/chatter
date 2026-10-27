//! Admission of the current file's language declaration, without a stale cache.

use super::ChatFile;
use crate::model::{Header, LanguageCode};

/// A nonempty ordered declaration, distinct from an observed header's raw list.
/// Individual code validity remains the language-code validator's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredLanguages {
    primary: LanguageCode,
    additional: Vec<LanguageCode>,
}

/// Borrowed evidence that an ordered language declaration has a primary code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredLanguagesRef<'a> {
    primary: &'a LanguageCode,
    additional: &'a [LanguageCode],
}

impl DeclaredLanguages {
    /// Admit an owned declaration without inventing a primary code for emptiness.
    pub fn new(codes: Vec<LanguageCode>) -> Result<Self, LanguageDeclarationError> {
        let mut codes = codes.into_iter();
        let primary = codes.next().ok_or(LanguageDeclarationError::Empty)?;
        Ok(Self {
            primary,
            additional: codes.collect(),
        })
    }

    /// Borrow the already admitted declaration without repeating admission.
    pub fn as_ref(&self) -> DeclaredLanguagesRef<'_> {
        DeclaredLanguagesRef {
            primary: &self.primary,
            additional: &self.additional,
        }
    }

    /// Read the admitted ordered codes.
    pub fn iter(&self) -> impl Iterator<Item = &LanguageCode> + Clone {
        self.as_ref().iter()
    }

    /// Append declarations without removing or replacing the required primary.
    pub fn extend(&mut self, codes: impl IntoIterator<Item = LanguageCode>) {
        self.additional.extend(codes);
    }
}

impl<'a> DeclaredLanguagesRef<'a> {
    /// Admit a raw list at a public or mutable-header boundary.
    pub fn new(codes: &'a [LanguageCode]) -> Result<Self, LanguageDeclarationError> {
        let (primary, additional) = codes.split_first().ok_or(LanguageDeclarationError::Empty)?;
        Ok(Self {
            primary,
            additional,
        })
    }

    /// The first declared language always exists.
    pub fn primary(self) -> &'a LanguageCode {
        self.primary
    }

    /// Remaining declarations, in source order; the first is secondary.
    pub fn additional(self) -> &'a [LanguageCode] {
        self.additional
    }

    /// Read every declared code, preserving order and repeated values.
    pub fn iter(self) -> impl Iterator<Item = &'a LanguageCode> + Clone {
        std::iter::once(self.primary).chain(self.additional.iter())
    }

    /// Test membership without changing declaration order.
    pub fn contains(self, code: &LanguageCode) -> bool {
        self.iter().any(|declared| declared == code)
    }

    /// Copy the codes for an explicitly mutable transformation.
    pub fn to_vec(self) -> Vec<LanguageCode> {
        self.iter().cloned().collect()
    }

    /// Own the admitted declaration without checking its cardinality again.
    pub fn to_owned(self) -> DeclaredLanguages {
        DeclaredLanguages {
            primary: self.primary.clone(),
            additional: self.additional.to_vec(),
        }
    }
}

impl serde::Serialize for DeclaredLanguagesRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl serde::Serialize for DeclaredLanguages {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.as_ref(), serializer)
    }
}

/// Why a transcript cannot supply an ordered language declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LanguageDeclarationError {
    /// No declaration was observed.
    #[error("missing @Languages declaration")]
    Absent,
    /// A declaration was observed but its syntax could not be admitted.
    #[error("malformed @Languages declaration")]
    Rejected,
    /// A structured declaration has no codes (including imported JSON).
    #[error("empty @Languages declaration")]
    Empty,
    /// More than one declaration was observed; no one of them is authoritative.
    #[error("repeated @Languages declarations")]
    Repeated,
}

impl ChatFile {
    /// Admit exactly one nonempty, structurally parsed language declaration.
    ///
    /// Read the current headers, including after JSON import or mutation.
    /// Rejected and repeated declarations never supply a default or membership
    /// set. Individual code validity remains the language-code validator's job.
    pub fn language_declaration(
        &self,
    ) -> Result<DeclaredLanguagesRef<'_>, LanguageDeclarationError> {
        let mut declaration = Err(LanguageDeclarationError::Absent);
        for header in self.headers() {
            let next = match header {
                Header::Languages { codes } => DeclaredLanguagesRef::new(codes),
                Header::UnparsedLanguages(_) => Err(LanguageDeclarationError::Rejected),
                _ => continue,
            };
            declaration = match declaration {
                Err(LanguageDeclarationError::Absent) => next,
                Ok(_)
                | Err(
                    LanguageDeclarationError::Rejected
                    | LanguageDeclarationError::Empty
                    | LanguageDeclarationError::Repeated,
                ) => Err(LanguageDeclarationError::Repeated),
            };
        }
        declaration
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[test]
    fn declaration_admission_preserves_order_and_refuses_empty_lists() {
        assert_eq!(
            DeclaredLanguages::new(Vec::new()),
            Err(LanguageDeclarationError::Empty)
        );
        assert_eq!(
            DeclaredLanguagesRef::new(&[]),
            Err(LanguageDeclarationError::Empty)
        );
        let codes = ["eng", "spa", "eng"].map(|code| LanguageCode::new(code).unwrap());
        let borrowed = DeclaredLanguagesRef::new(&codes).unwrap();
        assert_eq!(borrowed.primary(), &codes[0]);
        assert_eq!(borrowed.additional(), &codes[1..]);
        assert_eq!(borrowed.to_vec(), codes);
        let owned = DeclaredLanguages::new(codes.to_vec()).unwrap();
        assert_eq!(owned.as_ref(), borrowed);
        assert_eq!(borrowed.to_owned(), owned);
        assert_eq!(
            serde_json::to_value(borrowed).unwrap(),
            serde_json::json!(["eng", "spa", "eng"])
        );
        assert_eq!(
            serde_json::to_value(&owned).unwrap(),
            serde_json::to_value(borrowed).unwrap()
        );
        let mut extended = owned;
        extended.extend([LanguageCode::new("fra").unwrap()]);
        assert_eq!(extended.as_ref().primary(), borrowed.primary());
        assert_eq!(
            serde_json::to_value(extended).unwrap(),
            serde_json::json!(["eng", "spa", "eng", "fra"])
        );
    }

    #[test]
    fn declaration_cardinality_does_not_replace_individual_code_validation() {
        let empty_code: LanguageCode = serde_json::from_str("\"\"").unwrap();
        let declaration = DeclaredLanguages::new(vec![empty_code]).unwrap();
        assert_eq!(declaration.as_ref().primary().as_str(), "");
    }
}
