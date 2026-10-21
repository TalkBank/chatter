//! Native macro policy/API boundaries, not canonical CHAT coverage.
#![allow(clippy::panic, clippy::unwrap_used)]

use talkbank_derive::{ValidationTagged, error_code_enum};
use talkbank_model::model::{ValidationTag, ValidationTagged as TagTrait};

#[error_code_enum]
#[allow(clippy::non_minimal_cfg)] // Constant predicates exercise both gates on every target.
enum ConditionalRegistry {
    #[cfg(all())]
    #[code("E001")]
    Active,
    #[cfg(any())]
    #[code("E002")]
    #[status(planned)]
    Disabled,
    #[cfg_attr(all(), cfg(all()), doc = "A retained planned variant.")]
    #[code("E003")]
    #[status(planned)]
    Planned,
    #[cfg_attr(all(), cfg_attr(all(), cfg(all())))]
    #[code("E004")]
    #[status(opt_in)]
    OptIn,
    #[cfg_attr(all(), cfg(any()))]
    #[code("E005")]
    #[status(opt_in)]
    DisabledOptIn,
    #[cfg_attr(any(), cfg(any()))]
    #[code("E006")]
    Unconditional,
    #[cfg_attr(all(), cfg_attr(all(), cfg(any())))]
    #[code("E007")]
    DisabledNested,
    #[cfg(all())]
    #[cfg(any())]
    #[code("E008")]
    DisabledConjunction,
    #[cfg(all())]
    #[code("E999")]
    UnknownError,
}

#[error_code_enum]
#[cfg(any())]
enum DisabledRegistry {
    #[code("E999")]
    UnknownError,
}

#[error_code_enum]
#[cfg_attr(all(), cfg(any()))]
enum AlsoDisabledRegistry {
    #[code("E999")]
    UnknownError,
}

#[error_code_enum]
enum EmptyStatusRegistry {
    #[cfg(any())]
    #[code("E001")]
    #[status(planned)]
    DisabledPlanned,
    #[cfg(any())]
    #[code("E002")]
    #[status(opt_in)]
    DisabledOptIn,
    #[code("E999")]
    UnknownError,
}

#[test]
fn conditional_registry_keeps_api_counts_and_wire_values_in_sync() {
    use ConditionalRegistry::{Active, OptIn, Planned, Unconditional, UnknownError};
    let all: &[ConditionalRegistry; 5] = ConditionalRegistry::all();
    let planned: &[ConditionalRegistry; 1] = ConditionalRegistry::planned();
    let opt_in: &[ConditionalRegistry; 1] = ConditionalRegistry::opt_in();
    assert_eq!(all, &[Active, Planned, OptIn, Unconditional, UnknownError]);
    assert_eq!(planned, &[Planned]);
    assert_eq!(opt_in, &[OptIn]);
    assert_eq!(
        ConditionalRegistry::iter().copied().collect::<Vec<_>>(),
        all
    );
    for (value, code) in all.iter().zip(["E001", "E003", "E004", "E006", "E999"]) {
        assert_eq!(value.as_str(), code);
        assert_eq!(ConditionalRegistry::parse_exact(code), Some(*value));
        assert_eq!(serde_json::to_value(value).unwrap(), code);
        assert_eq!(
            serde_json::from_value::<ConditionalRegistry>(code.into()).unwrap(),
            *value
        );
    }
    for disabled in ["E002", "E005", "E007", "E008"] {
        assert_eq!(ConditionalRegistry::parse_exact(disabled), None);
        assert!(serde_json::from_value::<ConditionalRegistry>(disabled.into()).is_err());
    }
    let empty_planned: &[EmptyStatusRegistry; 0] = EmptyStatusRegistry::planned();
    let empty_opt_in: &[EmptyStatusRegistry; 0] = EmptyStatusRegistry::opt_in();
    assert!(empty_planned.is_empty());
    assert!(empty_opt_in.is_empty());
    assert_eq!(
        EmptyStatusRegistry::all(),
        &[EmptyStatusRegistry::UnknownError]
    );
    let schema = schemars::schema_for!(ConditionalRegistry);
    // Schemars groups plain unit variants into `enum` and emits the documented
    // variant as a separate `const` branch. Verify both membership and its docs.
    let branches = schema.as_value()["oneOf"].as_array().unwrap();
    let schema_codes: std::collections::BTreeSet<_> = branches
        .iter()
        .flat_map(|branch| {
            assert_eq!(branch["type"], "string");
            match branch.get("enum") {
                Some(values) => values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>(),
                None => {
                    assert_eq!(branch["description"], "A retained planned variant.");
                    vec![branch["const"].as_str().unwrap()]
                }
            }
        })
        .collect();
    assert_eq!(
        schema_codes,
        ["E001", "E003", "E004", "E006", "E999"]
            .into_iter()
            .collect()
    );
}

#[error_code_enum]
enum Registry {
    #[code("E001")]
    Active,
    #[code("E002")]
    #[status(planned)]
    Planned,
    #[code("E003")]
    #[status(opt_in)]
    OptIn,
    #[code("E999")]
    UnknownError,
    #[code("E1000")]
    FourDigit,
}

#[test]
fn generated_registry_api_preserves_spelling_order_and_status_policy() {
    let expected = [
        (Registry::Active, "E001"),
        (Registry::Planned, "E002"),
        (Registry::OptIn, "E003"),
        (Registry::UnknownError, "E999"),
        (Registry::FourDigit, "E1000"),
    ];
    for (value, code) in expected {
        assert_eq!(value.as_str(), code);
        assert_eq!(value.to_string(), code);
        assert_eq!(Registry::parse_exact(code), Some(value));
        assert_eq!(
            value.documentation_url(),
            format!("https://talkbank.org/errors/{code}")
        );
    }
    assert_eq!(Registry::all(), &expected.map(|(value, _)| value));
    assert_eq!(
        Registry::iter().copied().collect::<Vec<_>>(),
        expected.map(|(value, _)| value)
    );
    assert_eq!(Registry::planned(), &[Registry::Planned]);
    assert_eq!(Registry::opt_in(), &[Registry::OptIn]);
    assert!(Registry::UnknownError < Registry::FourDigit);
    for unknown in ["", "E1", "E0001", "E004", " E001", "E001 "] {
        assert_eq!(Registry::parse_exact(unknown), None);
    }
}

// Payload has no validation/equality/shift traits: tagging must not traverse it.
struct Payload;

#[derive(ValidationTagged)]
#[allow(dead_code)] // Payloads exist solely to exercise the declaration shapes.
enum GenericTags<'a, T, const N: usize = 2>
where
    T: 'a,
{
    Clean {
        value: &'a T,
        marker: std::marker::PhantomData<[u8; N]>,
    },
    #[validation_tag(clean)]
    ProblemError(T),
    DeferredWarning(T),
    #[validation_tag(error)]
    #[allow(non_camel_case_types)] // A real keyword needs a raw identifier.
    r#type,
}

#[test]
fn tagging_preserves_generic_shapes_raw_names_and_explicit_precedence() {
    let payload = Payload;
    let named: GenericTags<'_, Payload> = GenericTags::Clean {
        value: &payload,
        marker: std::marker::PhantomData,
    };
    let override_clean: GenericTags<'_, Payload> = GenericTags::ProblemError(Payload);
    let warning: GenericTags<'_, Payload> = GenericTags::DeferredWarning(Payload);
    let error: GenericTags<'_, Payload> = GenericTags::r#type;
    assert_eq!(named.validation_tag(), ValidationTag::Clean);
    assert_eq!(override_clean.validation_tag(), ValidationTag::Clean);
    assert_eq!(warning.validation_tag(), ValidationTag::Warning);
    assert_eq!(error.validation_tag(), ValidationTag::Error);
}
