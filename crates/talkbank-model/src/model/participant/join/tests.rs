//! Unit tests for the participant join.
//!
//! These exercised a `&[Header]` twin of the join that no production code
//! called, while the production `&[Line]` version had no unit test at all.
//! They now drive the real thing, and they live beside it rather than in the
//! tree-sitter parser crate, which had stopped owning the code they test.
//!
//! `Line::header` wraps each fixture header with a dummy span, which is what
//! the assertions here already assumed: every diagnostic is reported at
//! offset 0.

use indexmap::IndexMap;

use super::build_participants_from_lines;
use crate::model::{
    ChatDate, Header, IDHeader, LanguageCode, Line, Participant, ParticipantEntry, ParticipantName,
    ParticipantRole, SpeakerCode,
};
use crate::{ErrorCode, ErrorCollector, ParseError};

/// Run the join over fixture headers, returning the map and the diagnostics it
/// reported.
///
/// The sink is the join's only route to the map, so this is also the shortest
/// honest way to read the diagnostics back.
fn join(headers: Vec<Header>) -> (IndexMap<SpeakerCode, Participant>, Vec<ParseError>) {
    let lines: Vec<Line> = headers
        .into_iter()
        .map(|header| Line::header(crate::Location::Unlocated, header))
        .collect();
    let errors = ErrorCollector::new();
    let participants = build_participants_from_lines(&lines).report_into(&errors);
    (participants, errors.into_vec())
}

/// An `@ID` for a language code that every fixture below shares.
fn id(speaker: &str, role: &str) -> IDHeader {
    IDHeader::new(
        LanguageCode::new("eng").expect("test literal is non-empty"),
        speaker,
        role,
    )
}

/// A single-speaker `@Participants` declaration.
fn declares(speaker: &str, name: &str, role: &str) -> Header {
    Header::Participants {
        entries: vec![ParticipantEntry::new(
            SpeakerCode::new(speaker),
            Some(ParticipantName::new(name)),
            ParticipantRole::new(role),
        )]
        .into(),
    }
}

/// A declaration plus a matching `@ID` yields one enriched participant.
#[test]
fn test_build_participants_basic() -> Result<(), String> {
    let (participants, errors) = join(vec![
        declares("CHI", "Ruth", "Target_Child"),
        Header::ID(id("CHI", "Target_Child")),
    ]);

    assert_eq!(participants.len(), 1);
    assert!(errors.is_empty());

    let chi = participants
        .get("CHI")
        .ok_or_else(|| "CHI participant should exist".to_string())?;
    assert_eq!(chi.code.as_str(), "CHI");
    assert_eq!(chi.name().map(|n| n.as_str()), Some("Ruth"));
    assert_eq!(chi.role().map(|role| role.as_str()), Some("Target_Child"));
    Ok(())
}

/// An `@Birth of` naming a declared speaker enriches that participant.
#[test]
fn test_build_participants_with_birth() -> Result<(), String> {
    let (participants, errors) = join(vec![
        declares("CHI", "Ruth", "Target_Child"),
        Header::ID(id("CHI", "Target_Child")),
        Header::Birth {
            participant: SpeakerCode::new("CHI"),
            date: ChatDate::new("28-JUN-2001"),
        },
    ]);

    assert_eq!(participants.len(), 1);
    assert!(errors.is_empty());

    let chi = participants
        .get("CHI")
        .ok_or_else(|| "CHI participant should exist".to_string())?;
    assert_eq!(
        chi.birth_date.as_ref().map(|d| d.as_str()),
        Some("28-JUN-2001")
    );
    Ok(())
}

/// A declaration with no `@ID` raises E522 and contributes no participant.
#[test]
fn test_e522_missing_id() {
    let (participants, errors) = join(vec![declares("CHI", "Ruth", "Target_Child")]);

    assert_eq!(participants.len(), 0);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.to_string(), "E522");
    assert!(errors[0].message.contains("CHI"));
    assert!(errors[0].message.contains("no matching @ID header"));
}

/// An `@ID` with no declaration raises E523.
#[test]
fn test_e523_orphan_id() {
    let (participants, errors) = join(vec![
        declares("CHI", "Ruth", "Target_Child"),
        Header::ID(id("CHI", "Target_Child")),
        Header::ID(id("MOT", "Mother")),
    ]);

    assert_eq!(participants.len(), 1);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.to_string(), "E523");
    assert!(errors[0].message.contains("MOT"));
    assert!(errors[0].message.contains("not in @Participants"));
}

/// An `@Birth of` naming an undeclared speaker raises E524.
#[test]
fn test_e524_orphan_birth() {
    let (participants, errors) = join(vec![
        declares("CHI", "Ruth", "Target_Child"),
        Header::ID(id("CHI", "Target_Child")),
        Header::Birth {
            participant: SpeakerCode::new("MOT"),
            date: ChatDate::new("01-JAN-2000"),
        },
    ]);

    assert_eq!(participants.len(), 1);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.to_string(), "E524");
    assert!(errors[0].message.contains("MOT"));
    assert!(errors[0].message.contains("not a declared participant"));
}

/// Metadata absence does not erase declaration evidence for a birth header.
#[test]
fn birth_for_declared_speaker_without_id_is_not_orphaned() {
    let (participants, errors) = join(vec![
        declares("CHI", "Ruth", "Target_Child"),
        Header::Birth {
            participant: SpeakerCode::new("CHI"),
            date: ChatDate::new("01-JAN-2000"),
        },
    ]);
    assert!(participants.is_empty(), "metadata is genuinely missing");
    assert_eq!(errors.len(), 1, "only missing ID, no unknown birth speaker");
    assert_eq!(errors[0].code, crate::ErrorCode::SpeakerNotDefined);
}

/// Several speakers keep declaration order, and each takes only its own
/// `@ID` and `@Birth`.
#[test]
fn test_multiple_participants() -> Result<(), String> {
    let headers = vec![
        Header::Participants {
            entries: vec![
                ParticipantEntry::new(
                    SpeakerCode::new("CHI"),
                    Some(ParticipantName::new("Ruth")),
                    ParticipantRole::new("Target_Child"),
                ),
                ParticipantEntry::new(
                    SpeakerCode::new("INV"),
                    Some(ParticipantName::new("Chiat")),
                    ParticipantRole::new("Investigator"),
                ),
            ]
            .into(),
        },
        Header::ID(id("CHI", "Target_Child").with_age("10;03.")),
        Header::ID(id("INV", "Investigator")),
        Header::Birth {
            participant: SpeakerCode::new("CHI"),
            date: ChatDate::new("28-JUN-2001"),
        },
    ];

    let (participants, errors) = join(headers);

    assert_eq!(participants.len(), 2);
    assert!(errors.is_empty());

    let chi = participants
        .get("CHI")
        .ok_or_else(|| "CHI participant should exist".to_string())?;
    assert_eq!(chi.code.as_str(), "CHI");
    assert_eq!(
        chi.birth_date.as_ref().map(|d| d.as_str()),
        Some("28-JUN-2001")
    );

    let inv = participants
        .get("INV")
        .ok_or_else(|| "INV participant should exist".to_string())?;
    assert_eq!(inv.code.as_str(), "INV");
    assert_eq!(inv.birth_date, None);
    Ok(())
}

/// A rejected ID cannot establish absence, even after the wire loses locations.
#[test]
fn rejected_id_preserves_uncertainty_without_inventing_metadata() {
    let rejected = Header::UnparsedID(crate::model::UnparsedIDHeader::new(
        crate::Location::Unlocated,
        "@ID:\teng,,ara|corpus|CHI|||||Target_Child|||",
    ));
    for header in [
        rejected.clone(),
        serde_json::from_value(serde_json::to_value(rejected).unwrap()).unwrap(),
    ] {
        let (participants, errors) = join(vec![
            declares("CHI", "Ruth", "Target_Child"),
            header,
            Header::ID(id("INV", "Investigator")),
        ]);
        assert!(participants.is_empty());
        assert!(
            errors
                .iter()
                .all(|error| error.code != ErrorCode::SpeakerNotDefined)
        );
        // Readable orphan metadata still proves its own inconsistency.
        assert!(
            errors
                .iter()
                .any(|error| error.code == ErrorCode::OrphanIDHeader)
        );
    }
}
