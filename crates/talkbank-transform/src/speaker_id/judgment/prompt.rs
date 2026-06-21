//! Renders a [`JudgmentRequest`] into transport-neutral chat messages for the
//! holistic LLM judgment call. No network, no model -- just deterministic
//! string construction. The `talkbank-llm` crate maps [`ChatMessage`] values
//! to its endpoint's wire format.

use std::fmt::Write;

use crate::speaker_id::provenance::PromptVersion;

use super::request::JudgmentRequest;
use super::session_context::AgeMonths;

// ---------------------------------------------------------------------------
// Named age-band threshold constants
// ---------------------------------------------------------------------------

/// Upper bound (exclusive) for the babbling / pre-linguistic stage, in months.
/// Children below this age are typically at the babbling or single-word level
/// and produce little interpretable multi-word speech.
const BABBLING_MAX_MONTHS: u32 = 18;

/// Upper bound (exclusive) for the early-multiword stage, in months.
/// Children in this range are typically putting two to three words together
/// and have a rapidly expanding vocabulary.
const EARLY_MULTIWORD_MAX_MONTHS: u32 = 36;

/// Upper bound (exclusive) for the preschool stage, in months.
/// Children in this range use grammatically more complete sentences and are
/// refining phonology and morphology.
const PRESCHOOL_MAX_MONTHS: u32 = 72;

// School-age is everything at or above PRESCHOOL_MAX_MONTHS.

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/// Prompt-template version tag. Bump when wording changes so older override
/// entries can be marked stale (see [`PromptVersion`]).
///
/// v2 (2026-06-11): the declared sample type became a free-vocabulary
/// label from the corpus-agnostic session-context file, so the system
/// prompt's `corrected:<type>` instruction no longer names a closed,
/// corpus-specific set of sample types.
pub const CURRENT_PROMPT_VERSION: &str = "v2";

/// Label used in the user message when no sample type is known.
const UNKNOWN: &str = "unknown";

/// Label used in the user message when no declared adult roles are present.
const NONE: &str = "none";

// ---------------------------------------------------------------------------
// System prompt (static, transport-neutral)
// ---------------------------------------------------------------------------

/// The static system prompt sent as the first message in every holistic call.
///
/// It describes the task (map donor / ASR speaker codes to CHAT roles for a
/// child-language session) and specifies the EXACT JSON output schema the
/// model must return. The schema keys and enum values here match
/// `output.rs` byte-for-byte so that the JSON parser in that module never
/// encounters unexpected field names.
const SYSTEM_PROMPT: &str = "\
You are an expert child-language-data curator helping to assign CHAT-format \
speaker codes to anonymized ASR speakers in a child-language transcript.

CONTEXT
=======
In a TalkBank donor session the child's utterances are already hand-coded with \
the CHI speaker code. The ASR pipeline captures all speakers as numbered \
codes (PAR0, PAR1, ...). Your job is to decide, for each donor speaker code, \
whether it is the child (CHI), an adult to merge into the transcript, or \
noise to drop.

You will also:
  - Assign a CHAT adult-role code to every speaker you rule as adult.
  - Verify whether the declared sample type is correct.
  - Decide whether an adult merge is applicable at all (false for child-only \
monologues or reading samples with no interactive adult).

OUTPUT FORMAT
=============
Return ONLY a single JSON object. No prose before or after. No markdown code \
fence. No trailing commas. No comments.

The object must have exactly these six top-level keys:

  \"speaker_mapping\"
      Object mapping each donor speaker code (e.g. \"PAR0\") to one of:
        \"CHI\"    -- this donor IS the child; its utterances are dropped
        \"adult\"  -- this donor is the adult to merge in
        \"drop\"   -- this donor is noise or a third party; drop it

  \"adult_roles\"
      Object mapping each donor code you ruled \"adult\" to its CHAT role:
        \"INV\"  -- investigator
        \"SLP\"  -- speech-language pathologist / clinician
        \"MOT\"  -- mother
        \"FAT\"  -- father
      Leave the object empty ({}) when merge_applicable is false.

  \"sample_type\"
      Your verdict on the declared sample type:
        \"confirmed\"           -- the declared type is correct
        \"corrected:<type>\"    -- wrong; <type> is a short free-text label \
for the correct sample type
        \"uncertain\"           -- cannot determine

  \"merge_applicable\"
      Boolean (true/false). False for child-only or reading-aloud sessions \
with no interactive adult present.

  \"confidence\"
      Object with exactly three keys, each a float in 0.0..=1.0:
        \"mapping\"          -- confidence in the speaker_mapping decisions
        \"roles\"            -- confidence in the adult_roles assignments
        \"merge_applicable\" -- confidence in the merge_applicable decision

  \"reasoning\"
      One or two sentences of plain-text rationale. No JSON inside this value.

Example of a valid response (do not copy the content, only the shape):
{
  \"speaker_mapping\": {\"PAR0\": \"CHI\", \"PAR1\": \"adult\"},
  \"adult_roles\": {\"PAR1\": \"INV\"},
  \"sample_type\": \"confirmed\",
  \"merge_applicable\": true,
  \"confidence\": {\"mapping\": 0.9, \"roles\": 0.8, \"merge_applicable\": 0.95},
  \"reasoning\": \"PAR0 produces short one-word turns consistent with the child; \
PAR1 prompts and scaffolds.\"
}";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The role of a chat message: system context or user turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A system-context message that sets the model's behavior.
    System,
    /// A user-turn message that carries the per-session data.
    User,
}

/// A transport-neutral chat message. The `talkbank-llm` crate maps this to
/// its endpoint's wire format (OpenAI-compatible JSON, Anthropic API, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Whether this is a system or user message.
    pub role: Role,
    /// The message content.
    pub content: String,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Render the system + user messages for `request`. Returns the messages and
/// the prompt-template version used.
///
/// The caller passes the pair to a [`JudgmentProvider`] implementation which
/// maps it to the endpoint's wire format. The returned [`PromptVersion`]
/// is stored in the provenance block of every resulting override entry so
/// stale entries can be detected after a prompt update.
///
/// [`JudgmentProvider`]: super::provider::JudgmentProvider
pub fn render_messages(request: &JudgmentRequest) -> (Vec<ChatMessage>, PromptVersion) {
    let system = ChatMessage {
        role: Role::System,
        content: SYSTEM_PROMPT.to_string(),
    };
    let user = ChatMessage {
        role: Role::User,
        content: build_user_message(request),
    };
    let version = PromptVersion(CURRENT_PROMPT_VERSION.to_string());
    (vec![system, user], version)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build the per-session user message from `request`.
///
/// Uses `writeln!` into a `String`; `write!` / `writeln!` into a `String` is
/// infallible, so `Result`s are discarded with `let _ =` as required by the
/// style guide (no `.unwrap()` in non-test code).
fn build_user_message(request: &JudgmentRequest) -> String {
    let mut out = String::new();

    // Session identifier.
    let _ = writeln!(out, "session_id: {}", request.session_id.0);

    // Declared sample type (free vocabulary, rendered verbatim).
    let sample_label = request
        .sample_type
        .as_ref()
        .map(|st| st.as_str())
        .unwrap_or(UNKNOWN);
    let _ = writeln!(out, "sample_type: {sample_label}");

    // Declared adult roles (free vocabulary, rendered verbatim).
    if request.declared_roles.is_empty() {
        let _ = writeln!(out, "declared_adult_roles: {NONE}");
    } else {
        let labels: Vec<&str> = request
            .declared_roles
            .iter()
            .map(|role| role.as_str())
            .collect();
        let _ = writeln!(out, "declared_adult_roles: {}", labels.join(", "));
    }

    // Consent tier (free vocabulary, rendered verbatim).
    let tier_label = request
        .consent_tier
        .as_ref()
        .map(|tier| tier.as_str())
        .unwrap_or(UNKNOWN);
    let _ = writeln!(out, "consent_tier: {tier_label}");

    // Developmental stage (only when age is known).
    if let Some(age) = request.age_months {
        let hint = stage_hint(age);
        let _ = writeln!(out, "child_age_months: {age} ({hint})");
    }

    // Per-speaker samples.
    let _ = writeln!(out);
    for speaker in &request.samples {
        let _ = writeln!(out, "--- speaker: {} ---", speaker.code);
        for (i, utt) in speaker.utterances.iter().enumerate() {
            let _ = writeln!(out, "{}: {}", i + 1, utt.0);
        }
    }

    out
}

/// Return a coarse developmental-stage hint for the given age.
///
/// The hint is approximate guidance for the model, NOT a clinical assertion.
/// The raw month count is always included in the prompt as well so the model
/// can reason at full resolution. Band boundaries are defined by the named
/// constants above.
fn stage_hint(age: AgeMonths) -> &'static str {
    if age.0 < BABBLING_MAX_MONTHS {
        "babbling / pre-linguistic"
    } else if age.0 < EARLY_MULTIWORD_MAX_MONTHS {
        "early multiword"
    } else if age.0 < PRESCHOOL_MAX_MONTHS {
        "preschool"
    } else {
        "school-age"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use talkbank_model::SpeakerCode;

    use super::*;
    use crate::speaker_id::judgment::request::{
        JudgmentRequest, SampledUtterance, SessionId, SpeakerSamples,
    };
    use crate::speaker_id::judgment::session_context::{
        AgeMonths, ConsentTierLabel, RoleLabel, SampleTypeLabel,
    };

    // -----------------------------------------------------------------------
    // Test fixture helpers
    // -----------------------------------------------------------------------

    /// Build a minimal [`JudgmentRequest`] with two speakers: the CHI anchor
    /// and a PAR1 donor with a known sample utterance.
    fn minimal_request() -> JudgmentRequest {
        JudgmentRequest {
            session_id: SessionId("test_session_001".into()),
            sample_type: Some(SampleTypeLabel("clinician interview".into())),
            declared_roles: vec![RoleLabel("Investigator".into())],
            consent_tier: Some(ConsentTierLabel("audio only".into())),
            age_months: Some(AgeMonths(24)),
            anchor: SpeakerCode::new("CHI"),
            samples: vec![
                SpeakerSamples {
                    code: SpeakerCode::new("CHI"),
                    utterances: vec![
                        SampledUtterance("doggie run".into()),
                        SampledUtterance("more juice".into()),
                    ],
                },
                SpeakerSamples {
                    code: SpeakerCode::new("PAR1"),
                    utterances: vec![
                        SampledUtterance("can you say dog".into()),
                        SampledUtterance("good job".into()),
                    ],
                },
            ],
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    /// The system message must name every required JSON output key so the
    /// model knows exactly what to produce. These names match `output.rs`
    /// byte-for-byte.
    #[test]
    fn system_message_names_all_required_json_keys() {
        let request = minimal_request();
        let (messages, _) = render_messages(&request);
        let system_content = &messages[0].content;
        assert_eq!(
            messages[0].role,
            Role::System,
            "first message must be system"
        );
        for key in &[
            "speaker_mapping",
            "adult_roles",
            "sample_type",
            "merge_applicable",
            "confidence",
            "reasoning",
        ] {
            assert!(
                system_content.contains(key),
                "system prompt must name the JSON key {key:?}; got:\n{system_content}"
            );
        }
    }

    /// The user message must include the donor speaker code and at least one
    /// sampled utterance token so the model has real transcript evidence.
    #[test]
    fn user_message_contains_speaker_codes_and_sampled_text() {
        let request = minimal_request();
        let (messages, _) = render_messages(&request);
        let user_content = &messages[1].content;
        assert_eq!(messages[1].role, Role::User, "second message must be user");
        assert!(
            user_content.contains("PAR1"),
            "user message must contain donor speaker code PAR1; got:\n{user_content}"
        );
        // At least one sampled utterance token must appear.
        assert!(
            user_content.contains("dog"),
            "user message must contain at least one utterance token; got:\n{user_content}"
        );
    }

    /// When age_months is Some the user message must contain the
    /// "child_age_months:" line.
    #[test]
    fn developmental_stage_line_present_when_age_known() {
        let request = minimal_request(); // has age_months: Some(24)
        let (messages, _) = render_messages(&request);
        let user_content = &messages[1].content;
        assert!(
            user_content.contains("child_age_months:"),
            "user message must include child_age_months when age is known; got:\n{user_content}"
        );
        // The stage hint for 24 months is "early multiword".
        assert!(
            user_content.contains("early multiword"),
            "user message must include the 'early multiword' stage hint for 24 months; got:\n{user_content}"
        );
    }

    /// When age_months is None the "child_age_months:" line must be OMITTED.
    #[test]
    fn developmental_stage_line_absent_when_age_unknown() {
        let mut request = minimal_request();
        request.age_months = None;
        let (messages, _) = render_messages(&request);
        let user_content = &messages[1].content;
        assert!(
            !user_content.contains("child_age_months"),
            "user message must NOT include child_age_months when age is unknown; got:\n{user_content}"
        );
    }

    /// When no sample type is declared the rendered value must be the literal
    /// string "unknown".
    #[test]
    fn sample_type_renders_unknown_when_absent() {
        let mut request = minimal_request();
        request.sample_type = None;
        let (messages, _) = render_messages(&request);
        let user_content = &messages[1].content;
        assert!(
            user_content.contains("sample_type: unknown"),
            "user message must render 'unknown' when sample_type is None; got:\n{user_content}"
        );
    }

    /// render_messages must return the current prompt version tag. The
    /// literal is asserted (not `CURRENT_PROMPT_VERSION`, which would be a
    /// tautology) so that bumping the prompt version requires a deliberate
    /// matching edit here, keeping provenance-staleness reasoning honest.
    #[test]
    fn returns_current_prompt_version() {
        let request = minimal_request();
        let (_, version) = render_messages(&request);
        assert_eq!(
            version.0, "v2",
            "returned PromptVersion must equal the current prompt-template version literal"
        );
    }

    /// The free-vocabulary context labels must reach the user message
    /// verbatim: no chatter-side vocabulary mapping may rewrite them.
    #[test]
    fn context_labels_render_verbatim() {
        let request = minimal_request();
        let (messages, _) = render_messages(&request);
        let user_content = &messages[1].content;
        for expected in [
            "sample_type: clinician interview",
            "declared_adult_roles: Investigator",
            "consent_tier: audio only",
        ] {
            assert!(
                user_content.contains(expected),
                "user message must contain {expected:?} verbatim; got:\n{user_content}"
            );
        }
    }
}
