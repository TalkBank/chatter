//! Equality emission from an admitted field plan.

use proc_macro2::TokenStream;
use quote::quote;

use crate::bindings::TypeArguments;
use crate::field_plan::{BodyPlan, InputPlan, Semantic};

/// Emit equality without reparsing metadata or reconstructing bindings.
pub(crate) fn impl_semantic_eq(input: &InputPlan<Semantic>) -> TokenStream {
    let name = &input.name;
    let other = &input.bindings.other;
    let (impl_generics, _, where_clause) = input.generics.split_for_impl();
    let ty_generics = TypeArguments::new(&input.generics);
    let body = match &input.body {
        BodyPlan::Enum(variants) if variants.is_empty() => {
            quote! { let _ = #other; match *self {} }
        }
        BodyPlan::Struct(fields) => {
            let comparisons = fields.active().map(|field| {
                let member = &field.member;
                quote! { crate::model::SemanticEq::semantic_eq(&self.#member, &#other.#member) }
            });
            quote! { true #(&& #comparisons)* }
        }
        BodyPlan::Enum(variants) => {
            let arms = variants.iter().map(|variant| {
                let left = variant.fields.pattern(&variant.name, false, false);
                let right = variant.fields.pattern(&variant.name, true, false);
                let comparisons = variant.fields.active().map(|field| {
                    let left = &field.left;
                    let right = &field.right;
                    quote! { crate::model::SemanticEq::semantic_eq(#left, #right) }
                });
                quote! { (#left, #right) => true #(&& #comparisons)* }
            });
            quote! { match (self, #other) { #(#arms,)* _ => false } }
        }
    };
    quote! {
        #[allow(non_upper_case_globals, non_camel_case_types)] // Generic declarations were linted on the input type.
        impl #impl_generics crate::model::SemanticEq for #name #ty_generics #where_clause {
            fn semantic_eq(&self, #other: &Self) -> ::core::primitive::bool { #body }
        }
    }
}
