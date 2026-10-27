//! Wire admission must preserve the distinction between custom and built-in tiers.

use super::DependentTier;
use crate::generated::phon_tier_labels::RESERVED_CUSTOM_TIER_LABELS;
use crate::model::UserDefinedDependentTierLabel;

#[test]
fn structured_extension_labels_are_not_custom_labels() {
    for label in RESERVED_CUSTOM_TIER_LABELS {
        assert!(UserDefinedDependentTierLabel::new(label).is_err());
        let wire = serde_json::json!({
            "type": "UserDefined",
            "data": { "label": label, "content": "annotation" }
        });
        assert!(serde_json::from_value::<DependentTier>(wire).is_err());
    }
}

#[test]
fn unsupported_wire_recovery_remains_distinct() {
    let wire = serde_json::json!({
        "type": "Unsupported",
        "data": { "label": "bogus", "content": null }
    });
    let tier: DependentTier = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(&tier, DependentTier::Unsupported(_)));
    assert_eq!(serde_json::to_value(tier).unwrap(), wire);
}

#[test]
fn user_defined_wire_label_cannot_impersonate_a_structured_tier() {
    let wire = serde_json::json!({
        "type": "UserDefined",
        "data": { "label": "mor", "content": "arbitrary text" }
    });
    assert!(
        serde_json::from_value::<DependentTier>(wire).is_err(),
        "a custom-tier payload must not serialize with a built-in tier prefix"
    );
}

#[test]
fn user_defined_wire_label_requires_a_complete_custom_prefix() {
    for label in ["", "x", "x1", "xname:other", "xname\n", "%xname"] {
        assert!(UserDefinedDependentTierLabel::new(label).is_err());
        let wire = serde_json::json!({
            "type": "UserDefined",
            "data": { "label": label, "content": "annotation" }
        });
        assert!(
            serde_json::from_value::<DependentTier>(wire).is_err(),
            "invalid custom-tier label was admitted: {label:?}"
        );
    }
}

#[test]
fn user_defined_wire_label_preserves_valid_spelling() {
    for label in ["xtst", "xA1", "xmor"] {
        let wire = serde_json::json!({
            "type": "UserDefined",
            "data": { "label": label, "content": "annotation" }
        });
        let tier: DependentTier = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(tier).unwrap(), wire);
    }
}
