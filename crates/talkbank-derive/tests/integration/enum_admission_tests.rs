//! Native macro policy/API boundaries, not canonical CHAT coverage.
#![allow(clippy::panic, clippy::unwrap_used)]

use talkbank_derive::{ValidationTagged, error_code_enum};
use talkbank_model::model::{ValidationTag, ValidationTagged as TagTrait};

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
