//! Runtime configuration, privacy and idempotence boundaries, not CHAT goldens.

use super::*;

fn map_input(entries: &str) -> String {
    format!("version = 1\n[[transcripts]]\nkey = 'sample'\n{entries}")
}

fn entry(original: &str, replacement: &str) -> String {
    format!("[[transcripts.names]]\noriginal = '{original}'\nreplacement = '{replacement}'\n")
}

#[test]
fn exact_case_decisions_and_replacement_idempotence() {
    let parser = TreeSitterParser::new().unwrap();
    let input = map_input(&entry("Rose", "PersonA"));
    let map = NameMap::from_toml(&input, &parser).unwrap();
    let names = map.for_transcript("sample").unwrap();
    let NameDecision::Replace(replacement) = names.decide("Rose") else {
        panic!("exact match was not admitted");
    };
    assert_eq!(replacement.as_ref(), "PersonA");
    assert!(matches!(names.decide("rose"), NameDecision::CaseNearMiss));
    assert!(matches!(names.decide("ROSE"), NameDecision::CaseNearMiss));
    assert!(matches!(names.decide("roses"), NameDecision::Keep));
    assert!(matches!(
        names.decide(replacement.as_ref()),
        NameDecision::Keep
    ));
    assert!(map.for_transcript("other").is_none());
    assert!(map.for_transcript("sample.cha").is_none());
}

#[test]
fn unicode_names_and_shared_placeholders_keep_exact_identity() {
    let parser = TreeSitterParser::new().unwrap();
    let input = map_input(&(entry("Émile", "PersonA") + &entry("Summer", "PersonA")));
    let map = NameMap::from_toml(&input, &parser).unwrap();
    let names = map.for_transcript("sample").unwrap();
    for original in ["Émile", "Summer"] {
        let NameDecision::Replace(replacement) = names.decide(original) else {
            panic!("exact name must match");
        };
        assert_eq!(replacement.as_ref(), "PersonA");
    }
    assert!(matches!(names.decide("ÉMILE"), NameDecision::CaseNearMiss));
    assert!(matches!(names.decide("Emile"), NameDecision::Keep));
    assert!(matches!(names.decide("PersonA"), NameDecision::Keep));
}

#[test]
fn refuses_cycles_identity_duplicates_and_case_collisions() {
    let parser = TreeSitterParser::new().unwrap();
    for entries in [
        entry("Rose", "Rose"),
        entry("Rose", "rose"),
        entry("Rose", "Summer") + &entry("Summer", "PersonA"),
        entry("Rose", "Summer") + &entry("Summer", "Rose"),
    ] {
        assert_eq!(
            NameMap::from_toml(&map_input(&entries), &parser).unwrap_err(),
            NameMapError::ReplacementCollision
        );
    }
    assert_eq!(
        NameMap::from_toml(
            &map_input(&(entry("Rose", "PersonA") + &entry("Rose", "PersonB"))),
            &parser
        )
        .unwrap_err(),
        NameMapError::DuplicateName
    );
}

#[test]
fn refuses_markup_and_untranscribed_tokens_at_both_boundaries() {
    let parser = TreeSitterParser::new().unwrap();
    for token in [
        "",
        "two words",
        "Nam(e)",
        "Name+tag",
        "Rose@s",
        "Rose’s",
        "xxx",
        "yyy",
        "www",
    ] {
        for entries in [entry(token, "PersonA"), entry("Rose", token)] {
            assert_eq!(
                NameMap::from_toml(&map_input(&entries), &parser).unwrap_err(),
                NameMapError::InvalidToken
            );
        }
    }
}

#[test]
fn errors_and_debug_do_not_disclose_runtime_values() {
    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(&map_input(&entry("Rose", "PersonA")), &parser).unwrap();
    assert_eq!(format!("{map:?}"), "NameMap(<private>)");
    assert_eq!(
        format!("{:?}", map.for_transcript("sample").unwrap()),
        "TranscriptNames(<private>)"
    );
    for input in [
        "private_name = 'Rose'",
        "version = 'Rose'",
        "version = 1\ntranscripts = 'Rose'",
    ] {
        let error = NameMap::from_toml(input, &parser).unwrap_err();
        assert_eq!(error, NameMapError::InvalidDocument);
        assert!(!format!("{error:?} {error}").contains("Rose"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn schema_and_transcript_scoping_are_explicit() {
    let parser = TreeSitterParser::new().unwrap();
    for (input, expected) in [
        (
            "version = 2\ntranscripts = []".to_owned(),
            NameMapError::UnsupportedVersion,
        ),
        (
            "version = 1\ntranscripts = []".to_owned(),
            NameMapError::EmptyEntries,
        ),
        (map_input("names = []"), NameMapError::EmptyEntries),
        (
            map_input(&entry("Rose", "PersonA")).replace("key = 'sample'", "key = ''"),
            NameMapError::InvalidTranscriptKey,
        ),
        (
            map_input(&entry("Rose", "PersonA"))
                + "\n[[transcripts]]\nkey = 'sample'\n"
                + &entry("Summer", "PersonB"),
            NameMapError::DuplicateTranscript,
        ),
        (
            map_input(&entry("Rose", "PersonA")) + "unexpected = 'Rose'",
            NameMapError::InvalidDocument,
        ),
    ] {
        assert_eq!(NameMap::from_toml(&input, &parser).unwrap_err(), expected);
    }
    let input = map_input(&entry("Rose", "PersonA"))
        + "\n[[transcripts]]\nkey = 'other'\n"
        + &entry("PersonA", "PersonB");
    assert!(NameMap::from_toml(&input, &parser).is_ok());
}
