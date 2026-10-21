//! Tests for this subsystem.
//!

use talkbank_model::model::{
    AddTier, ComTier, ExpTier, GpxTier, IntTier, SitTier, SpaTier, WriteChat,
};

/// Tests com tier construction.
#[test]
fn test_com_tier_construction() {
    let tier = ComTier::from_text(talkbank_model::Location::Unlocated, "This is a comment");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%com:\tThis is a comment");
}

/// Tests exp tier construction.
#[test]
fn test_exp_tier_construction() {
    let tier = ExpTier::from_text(talkbank_model::Location::Unlocated, "Explanation text");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%exp:\tExplanation text");
}

/// Tests add tier construction.
#[test]
fn test_add_tier_construction() {
    let tier = AddTier::from_text(talkbank_model::Location::Unlocated, "MOT");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%add:\tMOT");
}

/// Tests spa tier construction.
#[test]
fn test_spa_tier_construction() {
    let tier = SpaTier::from_text(talkbank_model::Location::Unlocated, "$DIS");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%spa:\t$DIS");
}

/// Tests sit tier construction.
#[test]
fn test_sit_tier_construction() {
    let tier = SitTier::from_text(talkbank_model::Location::Unlocated, "at home");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%sit:\tat home");
}

/// Tests gpx tier construction.
#[test]
fn test_gpx_tier_construction() {
    let tier = GpxTier::from_text(talkbank_model::Location::Unlocated, "1.5");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%gpx:\t1.5");
}

/// Tests int tier construction.
#[test]
fn test_int_tier_construction() {
    let tier = IntTier::from_text(talkbank_model::Location::Unlocated, "rising");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%int:\trising");
}
