//! Registry emission accepts only a complete, ordered metadata owner.

use proc_macro2::TokenStream;
use quote::quote;

use crate::registry::{CompleteRegistry, Enforcement, RegistryVariant};

/// Keep the array length and its elements under exactly the same predicates.
/// Counting occurs in the target compiler, after configuration stripping.
fn variant_array<'a>(
    variants: impl Iterator<Item = &'a RegistryVariant>,
) -> (TokenStream, TokenStream) {
    let (values, units): (Vec<_>, Vec<_>) = variants
        .map(|variant| {
            let ident = variant.name();
            let configuration = variant.configuration();
            (
                quote! { #configuration Self::#ident },
                quote! { #configuration () },
            )
        })
        .unzip();
    (
        quote! { [#(#values),*] },
        quote! { { <[()]>::len(&[#(#units),*]) } },
    )
}

/// Emit the admitted registry without reinterpreting syntax or status metadata.
pub(crate) fn emit_registry(registry: &CompleteRegistry) -> TokenStream {
    let name = registry.name();
    let vis = registry.visibility();
    let attrs = registry.attributes();
    let configuration = registry.configuration();
    let variants = registry.variants();
    let enum_variants = variants.iter().map(|variant| {
        let ident = variant.name();
        let code = variant.code();
        let attrs = variant.attributes();
        quote! { #(#attrs)* #[serde(rename = #code)] #ident }
    });
    let as_str_arms = variants.iter().map(|variant| {
        let ident = variant.name();
        let code = variant.code();
        let configuration = variant.configuration();
        quote! { #configuration #name::#ident => #code }
    });
    let parse_arms = variants.iter().map(|variant| {
        let ident = variant.name();
        let code = variant.code();
        let configuration = variant.configuration();
        quote! { #configuration #code => ::core::option::Option::Some(#name::#ident) }
    });
    let (all, count) = variant_array(variants.iter());
    let (planned, planned_count) = variant_array(registry.variants_in(Enforcement::Planned));
    let (opt_in, opt_in_count) = variant_array(registry.variants_in(Enforcement::OptIn));
    quote! {
        #(#attrs)*
        #[derive(::core::clone::Clone, ::core::marker::Copy, ::core::fmt::Debug,
            ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::cmp::PartialOrd,
            ::core::cmp::Ord, ::core::hash::Hash, ::serde::Serialize,
            ::serde::Deserialize, ::schemars::JsonSchema)]
        #vis enum #name { #(#enum_variants,)* }

        // Syntax admission proves the sentinel is declared; target compilation
        // additionally proves that configuration has not removed it.
        #configuration
        const _: () = { let _ = #name::UnknownError; };

        #configuration
        impl #name {
            /// Return this variant's canonical short code.
            pub fn as_str(&self) -> &'static ::core::primitive::str {
                match self { #(#as_str_arms,)* }
            }

            /// Parse an exact short code; unknown strings are refused.
            pub fn parse_exact(code: &::core::primitive::str) -> ::core::option::Option<Self> {
                match code { #(#parse_arms,)* _ => ::core::option::Option::None }
            }

            /// Return the documentation URL for this canonical code.
            pub fn documentation_url(&self) -> ::std::string::String {
                ::std::format!("https://talkbank.org/errors/{}", self.as_str())
            }

            /// Every known variant in admitted declaration/code order.
            pub fn all() -> &'static [Self; #count] {
                &#all
            }

            /// Iterate over every known variant in admitted code order.
            pub fn iter() -> ::core::slice::Iter<'static, Self> {
                Self::all().iter()
            }

            /// Documented variants not yet enforced by the validator.
            pub fn planned() -> &'static [Self; #planned_count] {
                &#planned
            }

            /// Enforced variants requiring an explicit rule option.
            pub fn opt_in() -> &'static [Self; #opt_in_count] {
                &#opt_in
            }
        }

        #configuration
        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    }
}
