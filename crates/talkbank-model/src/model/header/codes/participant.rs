//! Participant-related code types (`@Participants` records).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

use crate::{interned_newtype, string_newtype};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

interned_newtype! {
    /// Participant role in conversation
    ///
    /// Roles describe the participant's function in the conversation.
    /// Common examples: "Target_Child", "Mother", "Father", "Investigator"
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
    pub struct ParticipantRole,
    interner: crate::model::participant_interner()
}

string_newtype!(
    /// Display name recorded in `@Participants`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
    pub struct ParticipantName;
);

/// The observed name and role of a participant, or an explicit missing role.
///
/// The positional syntax cannot identify a name without a final role token.
/// Keeping the optional name inside Complete prevents an omitted role from
/// turning an alleged name into a different role during serialization.
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParticipantDetails {
    /// Observed role token and optional preceding name; validity is separate.
    Complete {
        /// Optional name preceding the role.
        name: Option<ParticipantName>,
        /// Observed role token, not an inferred value from ID metadata.
        role: ParticipantRole,
    },
    /// Only a speaker code was observed.
    MissingRole {},
}

impl ParticipantDetails {
    /// Borrow an observed name, if the positional syntax identifies one.
    pub fn name(&self) -> Option<&ParticipantName> {
        match self {
            Self::Complete { name, .. } => name.as_ref(),
            Self::MissingRole {} => None,
        }
    }

    /// Borrow the observed role without inventing text for an omission.
    pub fn role(&self) -> Option<&ParticipantRole> {
        match self {
            Self::Complete { role, .. } => Some(role),
            Self::MissingRole {} => None,
        }
    }

    /// Remove a name without altering role evidence.
    pub fn clear_name(&mut self) {
        match self {
            Self::Complete { name, .. } => *name = None,
            Self::MissingRole {} => {}
        }
    }
}

/// Participant entry in @Participants header
///
/// Format: `SPEAKER_CODE [Optional Name] Role`
/// Example: `CHI Alex Target_Child` or `MOT Mother`
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(deny_unknown_fields)]
pub struct ParticipantEntry {
    /// Observed speaker code.
    pub speaker_code: super::SpeakerCode,
    /// Complete name/role syntax or explicit role omission.
    pub details: ParticipantDetails,
}

impl ParticipantEntry {
    /// Construct observed name/role syntax; vocabulary validation is separate.
    pub fn new(
        speaker_code: super::SpeakerCode,
        name: Option<ParticipantName>,
        role: ParticipantRole,
    ) -> Self {
        Self {
            speaker_code,
            details: ParticipantDetails::Complete { name, role },
        }
    }

    /// Preserve an observed speaker code without inventing a name or role.
    pub fn missing_role(speaker_code: super::SpeakerCode) -> Self {
        Self {
            speaker_code,
            details: ParticipantDetails::MissingRole {},
        }
    }

    /// The name established by the positional syntax.
    pub fn name(&self) -> Option<&ParticipantName> {
        self.details.name()
    }

    /// The role token, if one was observed.
    pub fn role(&self) -> Option<&ParticipantRole> {
        self.details.role()
    }
}

impl crate::model::WriteChat for ParticipantEntry {
    /// Writes one entry, without the enclosing header label or list separator.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        crate::model::WriteChat::write_chat(&self.speaker_code, w)?;
        if let Some(name) = self.name() {
            {
                w.write_str(" ")?;
                crate::model::WriteChat::write_chat(&name, w)
            }?;
        }
        if let Some(role) = self.role() {
            w.write_str(" ")?;
            crate::model::WriteChat::write_chat(role, w)?;
        }
        Ok(())
    }
}

impl crate::validation::Validate for ParticipantEntry {
    /// Validates speaker-code formatting and role membership against CHAT role vocabulary.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        // The entry has no location of its own; the header validator narrows
        // the context to its line.
        let location = context.field_diagnostic_location();
        if self.speaker_code.as_str().is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyParticipantCode,
                crate::Severity::Error,
                location,
                crate::ErrorContext::new(
                    self.speaker_code.as_str(),
                    crate::Span::new(0, 0),
                    "speaker_code",
                ),
                "Participant speaker code cannot be empty",
            ));
        }

        crate::validation::header::check_speaker_id(
            self.speaker_code.as_str(),
            context.field_location,
            errors,
        );

        let Some(role) = self.role() else {
            errors.report(crate::ParseError::at_location(
                crate::ErrorCode::EmptyParticipantRole,
                crate::Severity::Error,
                context.field_location,
                "Participant declaration has no role",
            ));
            return;
        };
        if role.as_str().is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyParticipantRole,
                crate::Severity::Error,
                location,
                crate::ErrorContext::new(role.as_str(), crate::Span::new(0, 0), "role"),
                "Participant role cannot be empty",
            ));
        } else if !crate::validation::header::participant::is_allowed_participant_role(
            role.as_str(),
        ) {
            let suggested =
                crate::validation::header::participant::suggest_similar_role(role.as_str());
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::InvalidParticipantRole,
                    crate::Severity::Error,
                    location,
                    crate::ErrorContext::new(role.as_str(), crate::Span::new(0, 0), "role"),
                    format!("Invalid participant role: '{}'", role.as_str()),
                )
                .with_suggestion(format!("Use a valid CHAT role such as: {}", suggested)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn omitted_role_requires_explicit_wire_evidence_and_remains_invalid() {
        use crate::validation::Validate;
        let wire = serde_json::json!({"speaker_code":"CHI", "details":{"state":"missing_role"}});
        let entry: ParticipantEntry =
            serde_json::from_value(wire.clone()).expect("explicit omission");
        assert_eq!(entry.details, ParticipantDetails::MissingRole {});
        assert_eq!(serde_json::to_value(&entry).expect("wire"), wire);
        for field in ["name", "role"] {
            let mut contradictory = wire.clone();
            contradictory["details"][field] = serde_json::json!("Mother");
            assert!(
                serde_json::from_value::<ParticipantEntry>(contradictory).is_err(),
                "missing-role syntax cannot carry a {field}"
            );
        }
        let errors = crate::ErrorCollector::new();
        entry.validate(&crate::validation::ValidationContext::new(), &errors);
        assert_eq!(
            errors
                .into_vec()
                .iter()
                .map(|error| error.code)
                .collect::<Vec<_>>(),
            vec![crate::ErrorCode::EmptyParticipantRole]
        );
        let mut absent = wire;
        absent.as_object_mut().expect("object").remove("details");
        assert!(
            serde_json::from_value::<ParticipantEntry>(absent).is_err(),
            "missing wire data must not default"
        );
    }

    /// Repeated participant-role labels reuse the same interned allocation.
    ///
    /// This test guards the memory-sharing contract for frequent role strings.
    #[test]
    fn test_participant_role_interning() {
        let mother1 = ParticipantRole::new("Mother");
        let mother2 = ParticipantRole::new("Mother");

        // Same Arc (pointer equality) - strings should be interned
        assert!(Arc::ptr_eq(&mother1.0, &mother2.0));
        assert_eq!(mother1.as_str(), "Mother");
        assert_eq!(mother2.as_str(), "Mother");
    }

    /// Distinct participant roles must not alias to the same interned pointer.
    ///
    /// Avoiding aliasing keeps equality and diagnostics semantically correct.
    #[test]
    fn test_participant_role_different_values() {
        let mother = ParticipantRole::new("Mother");
        let father = ParticipantRole::new("Father");

        // Different values - different Arcs
        assert!(!Arc::ptr_eq(&mother.0, &father.0));
        assert_eq!(mother.as_str(), "Mother");
        assert_eq!(father.as_str(), "Father");
    }

    /// Display output preserves the exact role label text.
    ///
    /// This ensures header serialization remains lossless for participant roles.
    #[test]
    fn test_participant_role_display() {
        let role = ParticipantRole::new("Target_Child");
        assert_eq!(role.to_string(), "Target_Child");
        assert_eq!(format!("{}", role), "Target_Child");
    }
}
