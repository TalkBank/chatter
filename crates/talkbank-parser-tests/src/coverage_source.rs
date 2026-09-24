//! Identify explicit inline test-only Rust ranges for coverage attribution.
//!
//! This parses Rust syntax, not names or mangled symbols. Unrecognized cfg
//! expressions are retained conservatively: this inventory does not claim to
//! reproduce rustc's conditional compilation or macro expansion.

use sha2::{Digest, Sha256};
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Source identity plus test-only byte ranges, with exclusive upper bounds.
#[derive(Debug, serde::Serialize)]
pub struct TestSourceRanges {
    /// SHA-256 of the exact UTF-8 source whose offsets were parsed.
    pub source_sha256: String,
    /// Explicit `#[test]` functions and `#[cfg(test)]` modules/functions/impls.
    pub test_byte_ranges: Vec<std::ops::Range<usize>>,
}

/// Parse one complete Rust file and retain only syntactically explicit tests.
pub fn test_source_ranges(source: &str) -> Result<TestSourceRanges, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut visitor = TestItems { ranges: Vec::new() };
    visitor.visit_file(&syntax);
    Ok(TestSourceRanges {
        source_sha256: Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        test_byte_ranges: visitor.ranges,
    })
}

fn test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Path>()
                    .is_ok_and(|path| path.is_ident("test")))
    })
}

struct TestItems {
    ranges: Vec<std::ops::Range<usize>>,
}

impl TestItems {
    fn record(&mut self, span: proc_macro2::Span) {
        self.ranges.push(span.byte_range());
    }
}

impl<'ast> Visit<'ast> for TestItems {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if test_only(&item.attrs) && item.content.is_some() {
            self.record(item.span());
        } else {
            syn::visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if test_only(&item.attrs) {
            self.record(item.span());
        } else {
            syn::visit::visit_item_fn(self, item);
        }
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if test_only(&item.attrs) {
            self.record(item.span());
        } else {
            syn::visit::visit_item_impl(self, item);
        }
    }
}
