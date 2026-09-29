//! Coverage-tool boundary tests: Rust syntax and UTF-8 byte offsets.
#![allow(clippy::expect_used)]

use talkbank_parser_tests::coverage_source::{SourceItemKind, test_source_ranges};

#[test]
fn only_explicit_test_items_are_classified() {
    let source = r##"
// Unicode before the ranges: λ
fn production() { let _ = "#[test] fn not_a_test() {}"; }
#[cfg(test)] mod cases { fn helper() {} #[test] fn example() {} }
#[test] fn standalone() {}
#[cfg(any(test, feature = "runtime"))] fn also_production() {}
mod tests { fn production_despite_name() {} }
"##;
    let result = test_source_ranges(source).expect("Rust syntax");
    assert_eq!(result.test_byte_ranges.len(), 2);
    let slices: Vec<_> = result
        .test_byte_ranges
        .iter()
        .map(|range| &source[range.clone()])
        .collect();
    assert!(slices[0].contains("fn helper()"));
    assert!(slices[1].contains("fn standalone()"));
    assert!(!slices.iter().any(|text| text.contains("also_production")));
    assert_ne!(
        result.source_sha256,
        test_source_ranges("").expect("empty Rust").source_sha256
    );
    assert_eq!(
        test_source_ranges("").expect("empty Rust").source_sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn invalid_rust_cannot_produce_an_empty_success_inventory() {
    assert!(test_source_ranges("fn {").is_err());
}

#[test]
fn omission_inventory_distinguishes_bodies_from_unexpanded_inputs() {
    let source = r#"
#[derive(Clone)] struct Value;
trait Contract { fn required(); fn provided() {} }
impl Value { fn method() { emit!(); } #[test] fn check() {} }
#[cfg(feature = "optional")] fn conditional() {}
#[cfg(test)] mod tests { fn helper() { hidden!(); } }
mod external;
make_functions!();
"#;
    let inventory = test_source_ranges(source).expect("Rust syntax");
    let of_kind = |kind| {
        inventory
            .non_test_items
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| &source[item.byte_range.clone()])
            .collect::<Vec<_>>()
    };
    assert_eq!(of_kind(SourceItemKind::FunctionBody).len(), 3);
    assert_eq!(
        of_kind(SourceItemKind::MacroTokens),
        ["emit!()", "make_functions!()"]
    );
    assert_eq!(of_kind(SourceItemKind::AttributeInput).len(), 2);
    assert_eq!(of_kind(SourceItemKind::ExternalModule), ["mod external;"]);
    assert_eq!(inventory.test_byte_ranges.len(), 2);
    let wire = serde_json::to_value(&inventory).expect("inventory wire representation");
    assert_eq!(wire["non_test_items"][0]["kind"], "attribute_input");
}
