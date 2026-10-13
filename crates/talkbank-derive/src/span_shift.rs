//! Recursive span movement from an admitted field plan.

use proc_macro2::TokenStream;
use quote::quote;

use crate::field_plan::{BodyPlan, InputPlan, Shift};

/// Emit span movement without binding user names as locals.
pub(crate) fn impl_span_shift(input: &InputPlan<Shift>) -> TokenStream {
    let name = &input.name;
    let mut generics = input.generics.clone();
    for param in generics.type_params_mut() {
        param
            .bounds
            .push(syn::parse_quote!(talkbank_model::SpanShift));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let body = match &input.body {
        BodyPlan::Struct(fields) => {
            let shifts = fields.active().map(|field| {
                let member = &field.member;
                quote! { self.#member.shift_spans_after(offset, delta); }
            });
            quote! { #(#shifts)* }
        }
        BodyPlan::Enum(variants) => {
            let arms = variants.iter().map(|variant| {
                let pattern = variant.fields.pattern(&variant.name, false, false);
                let shifts = variant.fields.active().map(|field| {
                    let binding = &field.left;
                    quote! { #binding.shift_spans_after(offset, delta); }
                });
                quote! { #pattern => { #(#shifts)* } }
            });
            quote! { match self { #(#arms,)* } }
        }
    };
    quote! {
        impl #impl_generics talkbank_model::SpanShift for #name #ty_generics #where_clause {
            fn shift_spans_after(&mut self, offset: u32, delta: i32) { #body }
        }
    }
}
