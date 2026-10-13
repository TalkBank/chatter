//! Native admission and ordering contracts; compiler fixtures own diagnostic locality.

use super::*;

fn admit(source: &str) -> syn::Result<CompleteRegistry> {
    RegistrySyntax::parse(TokenStream::new(), source.parse().expect("Rust tokens"))?
        .admit()?
        .order()?
        .complete()
}

#[test]
fn code_metadata_refuses_missing_malformed_duplicate_and_misplaced_attributes() {
    for attr in [
        "",
        "#[code]",
        "#[code()]",
        "#[code = \"E001\"]",
        "#[code(42)]",
        "#[code(\"E001\",)]",
        "#[code(\"E001\", \"E002\")]",
        "#[code(\"E001\")] #[code(\"E001\")]",
        "#[code(42)] #[code(\"E001\")]",
    ] {
        assert!(
            admit(&format!("enum Codes {{ {attr} UnknownError }}")).is_err(),
            "{attr}"
        );
    }
    assert!(admit("#[code(\"E001\")] enum Codes { #[code(\"E001\")] UnknownError }").is_err());
}

#[test]
fn status_metadata_refuses_malformed_duplicates_conflicts_and_misplacement() {
    for attr in [
        "#[status]",
        "#[status()]",
        "#[status = \"planned\"]",
        "#[status(typo)]",
        "#[status(planned,)]",
        "#[status(planned, opt_in)]",
        "#[status(planned = true)]",
        "#[status(planned)] #[status(planned)]",
        "#[status(opt_in)] #[status(opt_in)]",
        "#[status(planned)] #[status(opt_in)]",
    ] {
        assert!(
            admit(&format!(
                "enum Codes {{ #[code(\"E001\")] {attr} UnknownError }}"
            ))
            .is_err(),
            "{attr}"
        );
    }
    assert!(admit("#[status(planned)] enum Codes { #[code(\"E001\")] UnknownError }").is_err());
}

#[test]
fn shape_admission_never_erases_arguments_generics_where_clauses_or_discriminants() {
    assert!(
        RegistrySyntax::parse(
            "unexpected".parse().unwrap(),
            "enum Codes {}".parse().unwrap()
        )
        .is_err()
    );
    for source in [
        "struct Codes;",
        "union Codes { value: u32 }",
        "enum Codes<const N: usize> { #[code(\"E001\")] UnknownError }",
        "enum Codes where (): Clone { #[code(\"E001\")] UnknownError }",
        "enum Codes { #[code(\"E001\")] UnknownError = 7 }",
        "enum Codes { #[code(\"E001\")] UnknownError(u32) }",
        "enum Codes { #[code(\"E001\")] UnknownError { value: u32 } }",
    ] {
        assert!(admit(source).is_err(), "{source}");
    }
}

#[test]
fn order_and_completion_refuse_duplicate_keys_descending_values_and_missing_unknown() {
    for source in [
        "enum Codes {}",
        "enum Codes { #[code(\"E001\")] Known }",
        "enum Codes { #[code(\"E001\")] First, #[code(\"E1\")] UnknownError }",
        "enum Codes { #[code(\"E1000\")] First, #[code(\"E999\")] UnknownError }",
    ] {
        assert!(admit(source).is_err(), "{source}");
    }
}

#[test]
fn key_policy_keeps_existing_prefix_and_number_acceptance_without_normalizing_literals() {
    for code in ["001", "E001", "-1", "+1", "é1", "E4294967295"] {
        let literal = LitStr::new(code, proc_macro2::Span::call_site());
        let admitted = CodeLiteral::admit(literal).expect("existing accepted sort key");
        assert_eq!(admitted.literal.value(), code);
    }
    for code in ["", "E", "E1x", "E1_0", "E4294967296", "E1 "] {
        assert!(CodeLiteral::admit(LitStr::new(code, proc_macro2::Span::call_site())).is_err());
    }
}

#[test]
fn admitted_statuses_are_exclusive_and_absence_is_the_active_policy() {
    let registry = admit("pub enum Codes { #[code(\"E001\")] Active, #[code(\"E002\")] #[status(planned)] Planned, #[code(\"E003\")] #[status(opt_in)] OptIn, #[code(\"E999\")] UnknownError }").unwrap();
    for (status, names) in [
        (Enforcement::Active, vec!["Active", "UnknownError"]),
        (Enforcement::Planned, vec!["Planned"]),
        (Enforcement::OptIn, vec!["OptIn"]),
    ] {
        assert_eq!(
            registry
                .variants_in(status)
                .map(|variant| variant.name().to_string())
                .collect::<Vec<_>>(),
            names
        );
    }
}
