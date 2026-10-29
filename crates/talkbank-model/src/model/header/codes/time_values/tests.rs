use super::*;

#[test]
fn clock_range_accepts_valid_and_rejects_out_of_range() {
    let out_of_range = |s: &str| TimeDurationValue::from_text(s).has_out_of_range_component();
    // Valid clock values (CLAN accepts these).
    assert!(!out_of_range("23:59:59"));
    assert!(!out_of_range("12:30:45"));
    assert!(!out_of_range("00:00:00"));
    assert!(!out_of_range("00:00-23:59")); // HH:MM-HH:MM range, both endpoints valid
    // Out-of-range components (CLAN error 35), grounded empirically.
    assert!(out_of_range("99:99:99"));
    assert!(out_of_range("25:00:00")); // hours > 23
    assert!(out_of_range("12:60:00")); // minutes > 59
    assert!(out_of_range("12:00:60")); // seconds > 59
    assert!(out_of_range("00:00:00-25:00:00")); // a range endpoint out of range
}

#[test]
fn time_value_parse_hms() {
    let tv = parse_time_value("01:23:45").unwrap();
    assert_eq!(
        (tv.hours, tv.minutes, tv.seconds, tv.millis),
        (1, 23, 45, None)
    );
}

#[test]
fn time_value_parse_ms() {
    let tv = parse_time_value("23:45").unwrap();
    assert_eq!(
        (tv.hours, tv.minutes, tv.seconds, tv.millis),
        (0, 23, 45, None)
    );
}

#[test]
fn time_value_parse_bare_seconds() {
    let tv = parse_time_value("45").unwrap();
    assert_eq!(
        (tv.hours, tv.minutes, tv.seconds, tv.millis),
        (0, 0, 45, None)
    );
}

#[test]
fn time_value_parse_bare_seconds_with_millis() {
    let tv = parse_time_value("45.678").unwrap();
    assert_eq!(
        (tv.hours, tv.minutes, tv.seconds, tv.millis),
        (0, 0, 45, Some(678))
    );
}

#[test]
fn time_value_parse_with_millis() {
    let tv = parse_time_value("01:23:45.678").unwrap();
    assert_eq!(
        (tv.hours, tv.minutes, tv.seconds, tv.millis),
        (1, 23, 45, Some(678))
    );
}

#[test]
fn duration_single() {
    let d = TimeDurationValue::from_text("01:23:45");
    assert!(matches!(d.state, TimeDurationState::Parsed { .. }));
    assert_eq!(d.segments().len(), 1);
    assert!(matches!(d.segments()[0], TimeSegment::Single(_)));
}

#[test]
fn duration_range_hyphen() {
    let d = TimeDurationValue::from_text("00:00:00-01:30:00");
    assert_eq!(d.segments().len(), 1);
    assert!(matches!(d.segments()[0], TimeSegment::Range { .. }));
}

#[test]
fn duration_range_semicolon() {
    let d = TimeDurationValue::from_text("00:00:00;01:30:00");
    assert_eq!(d.segments().len(), 1);
    assert!(matches!(d.segments()[0], TimeSegment::Range { .. }));
}

#[test]
fn duration_comma_separated() {
    let d = TimeDurationValue::from_text("01:00:00, 02:00:00-03:00:00");
    assert_eq!(d.segments().len(), 2);
    assert!(matches!(d.segments()[0], TimeSegment::Single(_)));
    assert!(matches!(d.segments()[1], TimeSegment::Range { .. }));
}

#[test]
fn duration_unsupported() {
    let d = TimeDurationValue::from_text("foobar");
    assert!(matches!(d.state, TimeDurationState::Unsupported(_)));
    assert_eq!(d.segments().len(), 0);
}

#[test]
fn duration_admission_requires_every_segment_and_preserves_rejected_text() {
    use crate::model::{SemanticEq, WriteChat};

    let clock = TimeDurationValue::from_text("00:00:00");
    for text in [
        " ",
        ",",
        " , ",
        ",00:00:00",
        "00:00:00,",
        "00:00:00, ,01:00:00",
        "00:00:00,broken",
        "broken,00:00:00",
    ] {
        let value = TimeDurationValue::from_text(text);
        assert!(value.segments().is_empty(), "partial admission: {text:?}");
        assert!(!value.semantic_eq(&clock), "recovery became a clock");
        assert_eq!(value.invalidity().unwrap().into_text(), text);
        assert_eq!(value.to_chat_string(), text);
        let wire = serde_json::to_string(&value).unwrap();
        assert_eq!(wire, serde_json::to_string(text).unwrap());
        let restored: TimeDurationValue = serde_json::from_str(&wire).unwrap();
        assert_eq!(restored, value);
        assert!(restored.segments().is_empty());
    }
    // Complete permissive shapes remain inspectable; admission is not validity.
    for text in [
        "00:00:00,01:00:00",
        " 00:00:00 , 01:00:00 ",
        "00:00-01:00,02:00;03:00",
    ] {
        let value = TimeDurationValue::from_text(text);
        assert_eq!(value.segments().len(), 2);
        assert_eq!(value.to_chat_string(), text);
        assert!(value.invalidity().is_some());
    }
    assert!(TimeDurationValue::from_text("").invalidity().is_none());
}

#[test]
fn duration_roundtrip() {
    for input in &["01:23:45", "00:00:00-01:30:00", "01:00:00, 02:00:00"] {
        let d = TimeDurationValue::from_text(input);
        assert_eq!(d.as_str(), *input);
    }
}

#[test]
fn start_hms() {
    let s = TimeStartValue::from_text("01:23:45");
    assert!(matches!(
        s.time(),
        Some(TimeValue {
            hours: 1,
            minutes: 23,
            seconds: 45,
            ..
        })
    ));
}

#[test]
fn start_ms() {
    let s = TimeStartValue::from_text("23:45");
    assert!(matches!(
        s.time(),
        Some(TimeValue {
            hours: 0,
            minutes: 23,
            seconds: 45,
            ..
        })
    ));
}

#[test]
fn start_with_millis() {
    let s = TimeStartValue::from_text("01:23:45.678");
    assert!(matches!(
        s.time(),
        Some(TimeValue {
            hours: 1,
            minutes: 23,
            seconds: 45,
            millis: Some(678),
            ..
        })
    ));
}

#[test]
fn wire_admission_preserves_clock_assessment_and_absence() {
    use crate::model::{SemanticEq, ValidationTagged, WriteChat};
    for (text, duration_invalid, start_invalid) in [
        ("", false, false),
        ("00:00:00", false, false),
        ("25:00:00", true, true),
        ("12:60:00", true, true),
        ("12:00:60", true, true),
        ("17:30-18:00", false, true),
        ("59:59", true, false),
        ("01:23:45.678", true, true),
        ("afternoon", true, true),
        (" 01:23:45 ", true, true),
    ] {
        let wire = serde_json::to_string(text).unwrap();
        let duration = TimeDurationValue::from_text(text);
        let restored: TimeDurationValue = serde_json::from_str(&wire).unwrap();
        assert_eq!(restored, duration);
        assert!(restored.semantic_eq(&duration));
        assert_eq!(
            restored.invalidity().map(InvalidTimeDuration::into_text),
            duration_invalid.then_some(text)
        );
        assert_eq!(serde_json::to_string(&restored).unwrap(), wire);
        assert_eq!(restored.to_chat_string(), text);
        let start = TimeStartValue::from_text(text);
        let restored: TimeStartValue = serde_json::from_str(&wire).unwrap();
        assert_eq!(restored, start);
        assert!(restored.semantic_eq(&start));
        assert_eq!(
            restored.invalidity().map(InvalidTimeStart::into_text),
            start_invalid.then_some(text)
        );
        assert_eq!(serde_json::to_string(&restored).unwrap(), wire);
        assert_eq!(restored.to_chat_string(), text);
    }
    let absent = TimeStartValue::from_text("");
    let midnight = TimeStartValue::from_text("00:00:00");
    assert_eq!(absent.time(), None);
    assert!(midnight.time().is_some());
    assert!(!absent.semantic_eq(&midnight));
    assert!(!absent.has_validation_issue());
    assert!(!absent.violates_depfile_pattern());
    let absent_duration = TimeDurationValue::from_text("");
    assert!(!absent_duration.has_validation_issue());
    assert!(!absent_duration.violates_depfile_pattern());
    assert!(absent_duration.segments().is_empty());
    let mut copy = midnight.time().unwrap();
    copy.hours = 25;
    assert!(!copy.is_in_clock_range());
    assert!(midnight.invalidity().is_none());
    assert_eq!(midnight.as_str(), "00:00:00");
}
