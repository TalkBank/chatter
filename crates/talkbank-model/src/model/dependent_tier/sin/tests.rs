//! `%sin` token/group container tests.
//!
//! These cases cover token/group construction and canonical `%sin` formatting
//! so gesture-tier model behavior stays stable during parser changes.

use super::{SinGroupGestures, SinItem, SinTier, SinToken};

/// Builds a `%sin` tier from explicit `SinItem` values.
#[test]
fn test_sin_tier_new() {
    let items = vec![
        SinItem::Token(SinToken::new("g:toy:dpoint").expect("nonempty test token")),
        SinItem::Token(SinToken::new("0").expect("nonempty test token")),
    ];
    let tier = SinTier::new(items);
    assert_eq!(tier.items.len(), 2);
    assert_eq!(tier.len(), 2);
    assert!(!tier.is_empty());
}

/// Handles an empty `%sin` tier.
#[test]
fn test_sin_tier_empty() {
    let tier = SinTier::new(vec![]);
    assert_eq!(tier.len(), 0);
    assert!(tier.is_empty());
    assert!(SinTier::from_tokens(vec![String::new()]).is_err());
}

/// Preserves literal `0` gesture placeholders.
#[test]
fn test_sin_tier_with_zeros() -> Result<(), String> {
    let tier = SinTier::from_tokens(vec!["0".to_string(), "0".to_string(), "0".to_string()])
        .expect("nonempty test tokens");
    assert_eq!(tier.len(), 3);
    match &tier.items[0] {
        SinItem::Token(text) => assert_eq!(text.as_ref(), "0"),
        _ => return Err("Expected Token".to_string()),
    }
    Ok(())
}

/// Preserves explicit gesture tokens.
#[test]
fn test_sin_tier_with_gesture_codes() -> Result<(), String> {
    let tier = SinTier::from_tokens(vec![
        "g:toy:dpoint".to_string(),
        "gg:toyy:dpointt".to_string(),
    ])
    .expect("nonempty test tokens");
    assert_eq!(tier.len(), 2);
    match &tier.items[0] {
        SinItem::Token(text) => assert_eq!(text.as_ref(), "g:toy:dpoint"),
        _ => return Err("Expected Token".to_string()),
    }
    Ok(())
}

/// Preserves grouped `%sin` gestures.
#[test]
fn test_sin_tier_with_groups() -> Result<(), String> {
    let items = vec![
        SinItem::Token(SinToken::new("b").expect("nonempty test token")),
        SinItem::SinGroup(SinGroupGestures::new(vec![
            SinToken::new("c").expect("nonempty test token"),
            SinToken::new("d").expect("nonempty test token"),
        ])),
        SinItem::Token(SinToken::new("e").expect("nonempty test token")),
    ];
    let tier = SinTier::new(items);
    assert_eq!(tier.len(), 3);
    match &tier.items[1] {
        SinItem::SinGroup(gestures) => assert_eq!(gestures.len(), 2),
        _ => return Err("Expected SinGroup".to_string()),
    }
    Ok(())
}

/// Serializes `%sin` tiers to canonical CHAT text.
#[test]
fn test_sin_tier_chat_format() {
    use crate::model::WriteChat;
    let tier = SinTier::from_tokens(vec![
        "g:toy:dpoint".to_string(),
        "0".to_string(),
        "g:book:hold".to_string(),
    ])
    .expect("nonempty test tokens");
    let output = tier.to_chat_string();
    assert_eq!(output, "%sin:\tg:toy:dpoint 0 g:book:hold");
}

/// Lenient JSON admission must still reject empty tokens at utterance validation.
#[test]
fn deserialized_sin_tokens_are_validated_through_the_utterance() {
    use crate::model::{MainTier, Terminator, Utterance};
    use crate::validation::{Validate, ValidationContext};
    use crate::{ErrorCode, ErrorCollector, Span};

    let tier: SinTier = serde_json::from_value(serde_json::json!({
        "items": [
            {"type": "token", "content": ""},
            {"type": "sin_group", "content": ["g:toy:hold", ""]}
        ]
    }))
    .expect("editable JSON permits invalid text for contextual validation");
    let span = Span::from_usize(40, 70);
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
    let utterance = Utterance::new(main).with_sin(tier.with_span(span));
    let errors = ErrorCollector::new();
    utterance.validate(&ValidationContext::default(), &errors);
    let empty_tokens: Vec<_> = errors
        .into_vec()
        .into_iter()
        .filter(|error| error.code == ErrorCode::EmptyString)
        .collect();
    assert_eq!(
        empty_tokens.len(),
        2,
        "both bare and grouped empty tokens must be diagnosed"
    );
    assert!(empty_tokens.iter().all(|error| error.location.span == span));
}
