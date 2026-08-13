//! Attribute macro implementation for canonical error-code enums.
//!
//! Generates:
//! - Serde rename attributes for each variant
//! - `as_str()` for enum-to-code conversion
//! - `new()` for code-to-enum conversion with `UnknownError` fallback
//! - `parse_exact()` for a fallible code-to-enum conversion that returns
//!   `None` for any code that names no declared variant, instead of
//!   silently falling back
//! - `Display` implementation
//! - `documentation_url()` helper
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Lit, Meta};

/// Split `"E1000"` into `("E", 1000)`; `None` when the code is not a letter
/// prefix followed by digits.
fn code_sort_key(code: &str) -> Option<(&str, u32)> {
    let split = code.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = code.split_at(split);
    digits.parse().ok().map(|number| (prefix, number))
}

/// Reject a declaration order that does not ascend by (prefix, number).
///
/// Returns the compile error to emit, or `None` when the order is sound. The
/// error names both offending codes, because "some variant is out of order" is
/// not something a reader of a 225-variant enum can act on.
fn ascending_violation(variants: &[(&syn::Ident, String, Vec<&Attribute>)]) -> Option<TokenStream> {
    let mut previous: Option<(&str, u32)> = None;
    for (ident, code, _) in variants {
        let Some(key) = code_sort_key(code) else {
            return Some(
                syn::Error::new_spanned(
                    ident,
                    format!("code {code:?} is not a letter prefix followed by digits"),
                )
                .to_compile_error(),
            );
        };
        if let Some(before) = previous
            && key <= before
        {
            return Some(
                syn::Error::new_spanned(
                    ident,
                    format!(
                        "error codes must be declared in ascending order: {}{} follows {}{}. \
                         Declaration order IS the sort order (see the `Ord` derive), so move \
                         this variant rather than relaxing the rule.",
                        key.0, key.1, before.0, before.1
                    ),
                )
                .to_compile_error(),
            );
        }
        previous = Some(key);
    }
    None
}

/// Expand the `#[error_code_enum]` attribute into the generated enum API.
pub fn impl_error_code_enum(input: TokenStream) -> TokenStream {
    let input: DeriveInput = match syn::parse2(input) {
        Ok(input) => input,
        Err(err) => {
            return syn::Error::new(err.span(), "failed to parse enum input").to_compile_error();
        }
    };

    let enum_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;

    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return syn::Error::new_spanned(&input, "error_code_enum can only be used on enums")
                .to_compile_error();
        }
    };

    let mut variants_with_codes = Vec::new();
    let mut planned_variants: Vec<&syn::Ident> = Vec::new();
    let mut unknown_variant = None;

    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                &variant.fields,
                "error_code_enum only supports unit variants",
            )
            .to_compile_error();
        }

        let variant_name = &variant.ident;

        // Find #[code("E001")] attribute
        let code = match variant.attrs.iter().find_map(|attr| {
            if attr.path().is_ident("code")
                && let Meta::List(meta_list) = &attr.meta
                && let Ok(Lit::Str(lit_str)) = syn::parse2(meta_list.tokens.clone())
            {
                return Some(lit_str.value());
            }
            None
        }) {
            Some(code) => code,
            None => {
                return syn::Error::new_spanned(
                    variant,
                    format!(
                        "Variant {} missing #[code(\"...\")] attribute",
                        variant_name
                    ),
                )
                .to_compile_error();
            }
        };

        // `#[status(planned)]` marks a code whose spec is documented but not
        // yet enforced. Absence means enforced, which is a default that would
        // normally be a hazard (wrong invisibly); it is safe here only because
        // `SpecStatusGate` compares every variant against `spec/errors/*.md`
        // and fails on any disagreement in either direction.
        let planned = variant.attrs.iter().any(|attr| {
            attr.path().is_ident("status")
                && matches!(&attr.meta, Meta::List(list) if list.tokens.to_string() == "planned")
        });
        if planned {
            planned_variants.push(variant_name);
        }

        if variant_name == "UnknownError" {
            unknown_variant = Some(variant_name.clone());
        }

        // Keep non-code attributes
        let other_attrs: Vec<&Attribute> = variant
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("code") && !attr.path().is_ident("status"))
            .collect();

        variants_with_codes.push((variant_name, code, other_attrs));
    }

    // Declaration order must ascend by (letter prefix, number), so that the
    // DERIVED `Ord`, `all()`, `iter()` and any `BTreeSet<ErrorCode>` all agree
    // by construction instead of by three separate conventions.
    //
    // Checked here rather than trusted: the enum had two descending adjacencies
    // (`E391` then `E202`, `W108` then `E999`) while a comment two files away
    // asserted it was ascending. A hand-written `Ord` comparing the code STRING
    // was the first fix, and it was wrong in its own way, because a string
    // compare is lexicographic and would sort a future `E1000` before `E202`.
    // Ordering the declarations makes the derive correct and free, and makes
    // this the only place the rule can be broken.
    if let Some(err) = ascending_violation(&variants_with_codes) {
        return err;
    }

    let unknown_ident = match unknown_variant {
        Some(ident) => ident,
        None => {
            return syn::Error::new_spanned(
                enum_name,
                "ErrorCode enum must have UnknownError variant",
            )
            .to_compile_error();
        }
    };

    // Generate enum with serde rename attributes.
    let enum_variants = variants_with_codes
        .iter()
        .map(|(variant_name, code, other_attrs)| {
            quote! {
                #(#other_attrs)*
                #[serde(rename = #code)]
                #variant_name
            }
        });

    // Generate as_str() match arms
    let as_str_arms = variants_with_codes.iter().map(|(variant_name, code, _)| {
        quote! {
            #enum_name::#variant_name => #code
        }
    });

    // Generate new() match arms
    let new_arms = variants_with_codes.iter().map(|(variant_name, code, _)| {
        quote! {
            #code => #enum_name::#variant_name
        }
    });

    // Generate parse_exact() match arms: same code-to-variant mapping as
    // new(), but each arm is wrapped in `Some` and there is no fallback arm,
    // so a caller can tell "named a real variant" apart from "named nothing
    // we know about" (new() conflates the two into UnknownError).
    let parse_exact_arms = variants_with_codes.iter().map(|(variant_name, code, _)| {
        quote! {
            #code => Some(#enum_name::#variant_name)
        }
    });

    // Generate the const slice of every variant for iteration.
    // Lets callers enumerate every known code without hand-maintaining a list.
    let all_variants = variants_with_codes.iter().map(|(variant_name, _, _)| {
        quote! {
            #enum_name::#variant_name
        }
    });
    let variant_count = variants_with_codes.len();
    let planned_count = planned_variants.len();
    let planned_arms = planned_variants.iter().map(|variant_name| {
        quote! { #enum_name::#variant_name }
    });

    quote! {
        #(#attrs)*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #vis enum #enum_name {
            #(#enum_variants,)*
        }

        impl #enum_name {
            /// Return this enum variant's canonical short code (e.g., `"E356"`).
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms,)*
                }
            }

            /// Parse a short code into an enum variant.
            ///
            /// Unknown values map to `UnknownError`.
            pub fn new(code: &str) -> Self {
                match code {
                    #(#new_arms,)*
                    _ => #enum_name::#unknown_ident,
                }
            }

            /// Parse a short code into an enum variant, without a silent fallback.
            ///
            /// Returns `None` when `code` does not exactly match any declared
            /// variant's code. Unlike [`Self::new`], this never coerces an
            /// unrecognized string into the unknown-code sentinel variant, so
            /// callers that must distinguish "this names a real code" from
            /// "this is a typo" (e.g. validating user-supplied CLI arguments)
            /// should use this instead of comparing `new()`'s result against
            /// the sentinel variant by name.
            pub fn parse_exact(code: &str) -> Option<Self> {
                match code {
                    #(#parse_exact_arms,)*
                    _ => None,
                }
            }

            /// Return a stable documentation URL for this code.
            pub fn documentation_url(&self) -> String {
                format!("https://talkbank.org/errors/{}", self.as_str())
            }

            /// Return every known variant in declaration order.
            ///
            /// Used by tooling that needs to enumerate all codes (e.g., the
            /// `chatter validate --list-checks` flag). The returned slice is
            /// `'static`, callers do not need to allocate.
            pub fn all() -> &'static [Self; #variant_count] {
                const ALL: [#enum_name; #variant_count] = [
                    #(#all_variants,)*
                ];
                &ALL
            }

            /// Iterator over every known variant in declaration order.
            pub fn iter() -> std::slice::Iter<'static, Self> {
                Self::all().iter()
            }

            /// Every variant marked `#[status(planned)]`: documented in
            /// `spec/errors/` but not yet enforced by the validator.
            ///
            /// Generated from the attributes, so it cannot name a code that
            /// does not exist and cannot misspell one, which a hand-written
            /// list of code STRINGS could do and did.
            pub fn planned() -> &'static [Self; #planned_count] {
                const PLANNED: [#enum_name; #planned_count] = [
                    #(#planned_arms,)*
                ];
                &PLANNED
            }
        }

        impl std::fmt::Display for #enum_name {
            /// Format this error code using its canonical short code.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

    }
}
