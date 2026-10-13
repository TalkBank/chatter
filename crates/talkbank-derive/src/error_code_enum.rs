//! Registry emission accepts only a complete, ordered metadata owner.

use proc_macro2::TokenStream;
use quote::quote;

use crate::registry::{CompleteRegistry, Enforcement};

/// Emit the admitted registry without reinterpreting syntax or status metadata.
pub(crate) fn emit_registry(registry: &CompleteRegistry) -> TokenStream {
    let name = registry.name();
    let vis = registry.visibility();
    let attrs = registry.attributes();
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
        quote! { #name::#ident => #code }
    });
    let parse_arms = variants.iter().map(|variant| {
        let ident = variant.name();
        let code = variant.code();
        quote! { #code => Some(#name::#ident) }
    });
    let all = variants.iter().map(|variant| {
        let ident = variant.name();
        quote! { #name::#ident }
    });
    let planned: Vec<_> = registry
        .variants_in(Enforcement::Planned)
        .map(|variant| {
            let ident = variant.name();
            quote! { #name::#ident }
        })
        .collect();
    let opt_in: Vec<_> = registry
        .variants_in(Enforcement::OptIn)
        .map(|variant| {
            let ident = variant.name();
            quote! { #name::#ident }
        })
        .collect();
    let count = variants.len();
    let planned_count = planned.len();
    let opt_in_count = opt_in.len();
    quote! {
        #(#attrs)*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #vis enum #name { #(#enum_variants,)* }

        impl #name {
            /// Return this variant's canonical short code.
            pub fn as_str(&self) -> &'static str {
                match self { #(#as_str_arms,)* }
            }

            /// Parse an exact short code; unknown strings are refused.
            pub fn parse_exact(code: &str) -> Option<Self> {
                match code { #(#parse_arms,)* _ => None }
            }

            /// Return the documentation URL for this canonical code.
            pub fn documentation_url(&self) -> String {
                format!("https://talkbank.org/errors/{}", self.as_str())
            }

            /// Every known variant in admitted declaration/code order.
            pub fn all() -> &'static [Self; #count] {
                const ALL: [#name; #count] = [#(#all,)*];
                &ALL
            }

            /// Iterate over every known variant in admitted code order.
            pub fn iter() -> std::slice::Iter<'static, Self> {
                Self::all().iter()
            }

            /// Documented variants not yet enforced by the validator.
            pub fn planned() -> &'static [Self; #planned_count] {
                const PLANNED: [#name; #planned_count] = [#(#planned,)*];
                &PLANNED
            }

            /// Enforced variants requiring an explicit rule option.
            pub fn opt_in() -> &'static [Self; #opt_in_count] {
                const OPT_IN: [#name; #opt_in_count] = [#(#opt_in,)*];
                &OPT_IN
            }
        }

        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }
    }
}
