//! Attribute macro implementation for canonical error-code enums.
//!
//! Given an enum whose unit variants each carry a `#[code("E123")]`
//! attribute, this generates the enum itself plus the conversions,
//! enumeration helpers and `Display` that every error-code enum in the
//! workspace is expected to offer.
//!
//! The authoritative account of WHAT is generated is the `quote!` block at
//! the end of this file, whose generated items carry their own rustdoc. A
//! summary lived here until 2026-08-27 and is deliberately not replaced: it
//! was a hand-maintained mirror of that block and had drifted three ways at
//! once, still advertising a `new()` deleted the day before while omitting
//! `all()`, `iter()` and `planned()`.
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

    /// The variant every error-code enum must declare, so that a caller
    /// which must still produce a code for an unrecognized one has a variant
    /// to name. The CHECK below and the MESSAGE it produces read this same
    /// constant; they spelled the name out separately until 2026-08-27, so a
    /// rename could have left the diagnostic naming a variant nothing sought.
    const UNKNOWN_VARIANT: &str = "UnknownError";

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

        // `#[status(planned)]` marks a code that is documented but not yet
        // enforced. Absence means enforced, which is a default that would
        // normally be a hazard (wrong invisibly); it is safe here because the
        // attribute is not authored. Since R1 the whole enum is GENERATED from
        // `spec/codes/error-codes.toml`, and this attribute is emitted from
        // that code's `status`, so there is no second copy to disagree with.
        // `SpecStatusGate` used to reconcile the two and was deleted with the
        // copy.
        let planned = variant.attrs.iter().any(|attr| {
            attr.path().is_ident("status")
                && matches!(&attr.meta, Meta::List(list) if list.tokens.to_string() == "planned")
        });
        if planned {
            planned_variants.push(variant_name);
        }

        if variant_name == UNKNOWN_VARIANT {
            unknown_variant = Some(variant_name);
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

    // The requirement survives the deletion of the fallback arm; the
    // identifier does not. This macro no longer GENERATES anything naming the
    // unknown variant, so it validates its input and binds nothing.
    match unknown_variant {
        Some(_) => {}
        None => {
            return syn::Error::new_spanned(
                enum_name,
                format!("ErrorCode enum must have {UNKNOWN_VARIANT} variant"),
            )
            .to_compile_error();
        }
    }

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

    // Generate parse_exact() match arms: one per declared code, each wrapped
    // in `Some`, with no fallback arm, so a caller can tell "named a real
    // variant" apart from "named nothing we know about". The arms for the
    // deleted fallback constructor were still being built here, and emitted
    // nowhere, until 2026-08-27.
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

            /// Parse a short code into an enum variant, without a silent fallback.
            ///
            /// Returns `None` when `code` does not exactly match any declared
            /// variant's code. THE ONLY string constructor: a fallback form
            /// that coerced an unrecognized string into the unknown-code
            /// sentinel lived here until 2026-08-27 and was deleted once every
            /// internal caller took a variant instead, leaving it with no
            /// production callers and two tests of its own behaviour. Where a
            /// code is known at compile time, name the variant.
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
