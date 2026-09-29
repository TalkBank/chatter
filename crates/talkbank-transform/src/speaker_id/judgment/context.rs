//! Build the holistic judgment's per-session context from the optional
//! session-context file record plus the donor's `@ID` age.
//! `talkbank-transform` stays model-free; this is pure deterministic
//! mapping. Absent inputs map to `None` (the spec permits "unknown");
//! nothing is guessed.
//!
//! Resolution order per session:
//! 1. the explicit [`SessionContextRecord`] from the file, if present;
//! 2. for the age only, the donor's CHAT `@ID` age header (pure CHAT,
//!    no external metadata needed);
//! 3. otherwise unknown.
//!
//! [`SessionContextRecord`]: super::session_context::SessionContextRecord

use talkbank_model::model::{AgeValue, ChatFile};

use super::session_context::{
    AgeMonths, ConsentTierLabel, RoleLabel, SampleTypeLabel, SessionContextFile,
};

/// The four `JudgmentRequest` context fields, resolved for one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentContext {
    /// Declared sample-type label for this session, or `None` if unknown.
    pub sample_type: Option<SampleTypeLabel>,
    /// Declared adult role(s); empty when none are declared (the LLM
    /// then infers roles, and `sample_type` conveys the setting).
    pub declared_roles: Vec<RoleLabel>,
    /// Declared media-consent tier, or `None` if unknown.
    pub consent_tier: Option<ConsentTierLabel>,
    /// Child age in months: the session record's value if present, else
    /// the donor's `@ID` age, else `None`.
    pub age_months: Option<AgeMonths>,
}

/// Resolve the judgment context for `session_id` (the donor basename stem).
/// `context_file` is `None` when no `--session-context` was supplied.
/// `chat` is the parsed donor, used only for the `@ID` age fallback.
pub fn session_context(
    context_file: Option<&SessionContextFile>,
    session_id: &str,
    chat: &ChatFile,
) -> JudgmentContext {
    match context_file.and_then(|file| file.get(session_id)) {
        Some(record) => JudgmentContext {
            sample_type: record.sample_type.clone(),
            declared_roles: record.declared_roles.clone(),
            consent_tier: record.consent_tier.clone(),
            age_months: record.age_months.or_else(|| id_header_age_months(chat)),
        },
        None => JudgmentContext {
            sample_type: None,
            declared_roles: Vec::new(),
            consent_tier: None,
            age_months: id_header_age_months(chat),
        },
    }
}

/// Project the first supported `@ID` age with explicit months into total months.
/// Unsupported ages and unspecified months remain unknown. This consumes the
/// model's numeric representation rather than reinterpreting its retained text;
/// it is not a substitute for age syntax/range validation or anchor selection.
fn id_header_age_months(chat: &ChatFile) -> Option<AgeMonths> {
    for participant in chat.all_participants() {
        let Some(AgeValue::Valid {
            years,
            months: Some(months),
            ..
        }) = participant.id.age.as_ref()
        else {
            continue;
        };
        // u16 years and u8 months cannot overflow their widened u32 total.
        return Some(AgeMonths(u32::from(*years) * 12 + u32::from(*months)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::parse_and_validate;
    use crate::speaker_id::judgment::request::SessionId;
    use crate::speaker_id::judgment::session_context::SessionContextRecord;
    use std::collections::BTreeMap;
    use talkbank_model::ParseValidateOptions;

    /// Build a one-record context file keyed by `session_id`.
    fn file_with(session_id: &str, record: SessionContextRecord) -> SessionContextFile {
        SessionContextFile(BTreeMap::from([(
            SessionId(session_id.to_string()),
            record,
        )]))
    }

    fn chat_with_id_age(age_field: &str) -> talkbank_model::model::ChatFile {
        let src = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|c|CHI|{age_field}||||Target_Child|||\n*CHI:\thi . \u{15}0_1\u{15}\n@End\n"
        );
        parse_and_validate(&src, ParseValidateOptions::default()).expect("parse")
    }

    #[test]
    fn record_fills_labels_and_age() {
        let file = file_with(
            "NF201-3",
            SessionContextRecord {
                sample_type: Some(
                    SampleTypeLabel::try_from("clinician interview".to_string())
                        .expect("nonblank label"),
                ),
                declared_roles: vec![
                    RoleLabel::try_from("Investigator".to_string()).expect("nonblank label"),
                ],
                consent_tier: Some(
                    ConsentTierLabel::try_from("video+audio".to_string()).expect("nonblank label"),
                ),
                age_months: Some(AgeMonths(101)),
            },
        );
        let chat = chat_with_id_age("3;06.");
        let ctx = session_context(Some(&file), "NF201-3", &chat);
        assert_eq!(
            ctx.sample_type,
            Some(
                SampleTypeLabel::try_from("clinician interview".to_string())
                    .expect("nonblank label")
            )
        );
        assert_eq!(
            ctx.declared_roles,
            vec![RoleLabel::try_from("Investigator".to_string()).expect("nonblank label")]
        );
        assert_eq!(
            ctx.consent_tier,
            Some(ConsentTierLabel::try_from("video+audio".to_string()).expect("nonblank label"))
        );
        // The record's age wins over the @ID age (42 months here).
        assert_eq!(ctx.age_months, Some(AgeMonths(101)));
    }

    #[test]
    fn falls_back_to_id_age_when_record_age_absent() {
        let file = file_with(
            "NF201-3",
            SessionContextRecord {
                sample_type: None,
                declared_roles: Vec::new(),
                consent_tier: Some(
                    ConsentTierLabel::try_from("audio only".to_string()).expect("nonblank label"),
                ),
                age_months: None,
            },
        );
        let chat = chat_with_id_age("8;05."); // 8*12 + 5 = 101 months
        let ctx = session_context(Some(&file), "NF201-3", &chat);
        assert_eq!(
            ctx.consent_tier,
            Some(ConsentTierLabel::try_from("audio only".to_string()).expect("nonblank label"))
        );
        assert_eq!(ctx.age_months, Some(AgeMonths(101)));
    }

    #[test]
    fn absent_session_yields_unknown_but_keeps_id_age() {
        let file = file_with("OTHER", SessionContextRecord::default());
        let chat = chat_with_id_age("3;06.");
        let ctx = session_context(Some(&file), "NF201-3", &chat);
        assert_eq!(ctx.sample_type, None);
        assert!(ctx.declared_roles.is_empty());
        assert_eq!(ctx.consent_tier, None);
        assert_eq!(ctx.age_months, Some(AgeMonths(42))); // 3*12 + 6
    }

    #[test]
    fn no_context_file_uses_id_age_only() {
        let chat = chat_with_id_age("3;06.");
        let ctx = session_context(None, "NF201-3", &chat);
        assert_eq!(ctx.sample_type, None);
        assert!(ctx.declared_roles.is_empty());
        assert_eq!(ctx.consent_tier, None);
        assert_eq!(ctx.age_months, Some(AgeMonths(42)));
    }
}
