//! Source-admission and helper-call boundaries of the hygiene scanner.

use super::{BlankedShape, RelPath, Scanned, SourceFile, blank_literals, calls_helper};

#[test]
fn generic_helper_calls_are_not_function_items_or_declarations() {
    for call in [
        "rejects::<Semantic>(input)",
        "rejects :: < Vec<Option<Semantic>> > (input)",
        "module::rejects::<fn() -> bool>(input)",
        "() { for input in inputs { rejects::<Semantic>(input); } }",
    ] {
        assert!(
            calls_helper(&BlankedShape(call.into()), "rejects"),
            "{call}"
        );
    }
    for non_call in [
        "rejects::<Semantic>",
        "rejects_more::<Semantic>(input)",
        "not_rejects::<Semantic>(input)",
        "fn rejects<T>(input: T) {}",
        "rejects::<Semantic; unrelated(input)",
    ] {
        assert!(
            !calls_helper(&BlankedShape(non_call.into()), "rejects"),
            "{non_call}"
        );
    }
}

#[test]
fn helper_declaration_is_not_a_call() {
    let original = r#"
        fn report() { panic!("helper failed"); }
        #[test]
        fn compile_only() {
            struct Handler;
            impl Handler { fn report(&self) {} }
        }
        #[test]
        fn invokes_helper() { report (); }
    "#;
    let scan = Scanned {
        files: vec![SourceFile {
            path: RelPath::new("example.rs"),
            original: original.to_owned(),
            blanked: blank_literals(original),
        }],
        unreadable: vec![],
    };
    let found = scan.resolve().vacuous();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "compile_only");
}

#[test]
fn generic_admission_tests_resolve_through_their_real_source() {
    let original = include_str!("../../../talkbank-derive/src/field_plan.rs");
    let scan = Scanned {
        files: vec![SourceFile {
            path: RelPath::new("field_plan.rs"),
            original: original.to_owned(),
            blanked: blank_literals(original),
        }],
        unreadable: vec![],
    };
    let found = scan.resolve().vacuous();
    assert!(
        found.is_empty(),
        "admission helpers contain real assertions"
    );
}

#[test]
fn blanking_escaped_literals_preserves_length_and_token_admission() {
    let original = r#"rejects::<Kind>("escaped \"quote\" and \\ backslash");"#;
    let blanked = blank_literals(original);
    assert_eq!(blanked.len(), original.len());
    assert!(!blanked.contains('\\'));
    assert!(blanked.parse::<proc_macro2::TokenStream>().is_ok());
    assert!(calls_helper(&BlankedShape(blanked), "rejects"));
}
