//! Identify explicit test ranges and source-omission candidates for coverage attribution.
//!
//! This parses Rust syntax, not names or mangled symbols. Unrecognized cfg
//! expressions are retained conservatively: this inventory does not claim to
//! reproduce rustc's conditional compilation or macro expansion.

use sha2::{Digest, Sha256};
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Source identity plus syntax-backed byte ranges, with exclusive upper bounds.
#[derive(Debug, serde::Serialize)]
pub struct TestSourceRanges {
    /// SHA-256 of the exact UTF-8 source whose offsets were parsed.
    pub source_sha256: String,
    /// Explicit `#[test]` functions and `#[cfg(test)]` modules/functions/impls.
    pub test_byte_ranges: Vec<std::ops::Range<usize>>,
    /// Syntactic candidates outside explicit test-only items. These do not prove
    /// compilation, execution, or macro expansion under any feature selection.
    pub non_test_items: Vec<SourceItem>,
}

/// Distinct obligations in a source-file omission audit, not executable counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceItemKind {
    /// A free function, impl method, or trait default with a written body.
    FunctionBody,
    /// Unexpanded macro input; its generated items remain unknown.
    MacroTokens,
    /// Non-doc attribute input, including cfg, derive and procedural attributes.
    AttributeInput,
    /// An out-of-line module whose parent configuration is not resolved here.
    ExternalModule,
}

/// One syntax-backed item with offsets into the inventory's hashed source.
#[derive(Debug, serde::Serialize)]
pub struct SourceItem {
    /// What this range establishes, without promoting unknown expansion to code.
    pub kind: SourceItemKind,
    /// UTF-8 source byte range, exclusive at the upper bound.
    pub byte_range: std::ops::Range<usize>,
}

/// Parse one complete Rust file, distinguishing explicit tests from candidates.
pub fn test_source_ranges(source: &str) -> Result<TestSourceRanges, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut visitor = TestItems {
        ranges: Vec::new(),
        items: Vec::new(),
    };
    visitor.visit_file(&syntax);
    Ok(TestSourceRanges {
        source_sha256: Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        test_byte_ranges: visitor.ranges,
        non_test_items: visitor.items,
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
    items: Vec<SourceItem>,
}

impl TestItems {
    fn record(&mut self, span: proc_macro2::Span) {
        self.ranges.push(span.byte_range());
    }

    fn item(&mut self, kind: SourceItemKind, span: proc_macro2::Span) {
        self.items.push(SourceItem {
            kind,
            byte_range: span.byte_range(),
        });
    }
}

impl<'ast> Visit<'ast> for TestItems {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if test_only(&item.attrs) && item.content.is_some() {
            self.record(item.span());
        } else {
            if item.content.is_none() {
                self.item(SourceItemKind::ExternalModule, item.span());
            }
            syn::visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if test_only(&item.attrs) {
            self.record(item.span());
        } else {
            self.item(SourceItemKind::FunctionBody, item.block.span());
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

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if test_only(&item.attrs) {
            self.record(item.span());
        } else {
            self.item(SourceItemKind::FunctionBody, item.block.span());
            syn::visit::visit_impl_item_fn(self, item);
        }
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if test_only(&item.attrs) {
            self.record(item.span());
        } else {
            if let Some(body) = &item.default {
                self.item(SourceItemKind::FunctionBody, body.span());
            }
            syn::visit::visit_trait_item_fn(self, item);
        }
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.item(SourceItemKind::MacroTokens, item.span());
        // Macro token trees are deliberately not interpreted as Rust items.
    }

    fn visit_attribute(&mut self, item: &'ast syn::Attribute) {
        if !item.path().is_ident("doc") {
            self.item(SourceItemKind::AttributeInput, item.span());
        }
    }
}
