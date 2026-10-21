//! Recursive span movement from an admitted field plan.

use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::visit::Visit;

use crate::field_plan::{BodyPlan, FieldsPlan, InputPlan, Shift};

/// Only participating fields contribute inferred type-parameter bounds.
/// Opaque type macros retain the conservative bounds: their expansion can
/// reference a parameter even when it is not visible in the input tokens.
#[derive(Default)]
struct ActiveTypeParameters {
    names: HashSet<syn::Ident>,
    opaque: bool,
}

impl<'ast> Visit<'ast> for ActiveTypeParameters {
    fn visit_type(&mut self, ty: &'ast syn::Type) {
        match ty {
            syn::Type::Macro(_) | syn::Type::Verbatim(_) => self.opaque = true,
            _ => syn::visit::visit_type(self, ty),
        }
    }

    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if path.path.leading_colon.is_none()
            && let Some(segment) = path.path.segments.first()
        {
            self.names.insert(segment.ident.clone());
        }
        syn::visit::visit_type_path(self, path);
    }
}

fn shift_generics(input: &InputPlan<Shift>) -> syn::Generics {
    let mut active = ActiveTypeParameters::default();
    let mut visit_fields = |fields: &FieldsPlan| {
        for field in fields.active() {
            active.visit_type(&field.ty);
        }
    };
    match &input.body {
        BodyPlan::Struct(fields) => visit_fields(fields),
        BodyPlan::Enum(variants) => {
            for variant in variants {
                visit_fields(&variant.fields);
            }
        }
    }
    let mut generics = input.generics.clone();
    for param in generics.type_params_mut() {
        if active.opaque || active.names.contains(&param.ident) {
            param
                .bounds
                .push(syn::parse_quote!(talkbank_model::SpanShift));
        }
    }
    generics
}

/// Emit span movement without binding user names as locals.
pub(crate) fn impl_span_shift(input: &InputPlan<Shift>) -> TokenStream {
    let name = &input.name;
    let generics = shift_generics(input);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let body = match &input.body {
        BodyPlan::Enum(variants) if variants.is_empty() => {
            quote! { let _ = (offset, delta); match *self {} }
        }
        BodyPlan::Struct(fields) => {
            let shifts = fields.active().map(|field| {
                let member = &field.member;
                quote! { talkbank_model::SpanShift::shift_spans_after(&mut self.#member, offset, delta); }
            });
            quote! { #(#shifts)* }
        }
        BodyPlan::Enum(variants) => {
            let arms = variants.iter().map(|variant| {
                let pattern = variant.fields.pattern(&variant.name, false, false);
                let shifts = variant.fields.active().map(|field| {
                    let binding = &field.left;
                    quote! { talkbank_model::SpanShift::shift_spans_after(#binding, offset, delta); }
                });
                quote! { #pattern => { #(#shifts)* } }
            });
            quote! { match self { #(#arms,)* } }
        }
    };
    quote! {
        impl #impl_generics talkbank_model::SpanShift for #name #ty_generics #where_clause {
            fn shift_spans_after(&mut self, offset: usize, delta: isize) { #body }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    #[test]
    fn generic_bounds_follow_active_field_types_and_preserve_declared_constraints() {
        for (source, expected) in [
            (
                "struct Node<T, U> { values: Vec<Option<T>>, #[span_shift(skip)] cache: U }",
                "struct Node<T: talkbank_model::SpanShift, U> { }",
            ),
            (
                "enum Node<T, U> { Tuple(T, #[span_shift(skip)] U), Named { value: Option<T> } }",
                "struct Node<T: talkbank_model::SpanShift, U> { }",
            ),
            (
                "struct Node<'a, T: Clone = String, const N: usize = 2> where T: Default { #[span_shift(skip)] cache: &'a [T; N] }",
                "struct Node<'a, T: Clone = String, const N: usize = 2> where T: Default { }",
            ),
            (
                "struct Node<T> { value: T, #[span_shift(skip)] cache: T }",
                "struct Node<T: talkbank_model::SpanShift> { }",
            ),
            (
                "struct Node<T: Owner, U> { value: <T as Owner>::Item, #[span_shift(skip)] cache: U }",
                "struct Node<T: Owner + talkbank_model::SpanShift, U> { }",
            ),
            (
                "struct Node<T> { value: hidden_type!(), #[span_shift(skip)] cache: T }",
                "struct Node<T: talkbank_model::SpanShift> { }",
            ),
        ] {
            let input = InputPlan::<Shift>::admit(syn::parse_str(source).expect("input syntax"))
                .expect("valid shift plan");
            let expected: syn::DeriveInput = syn::parse_str(expected).expect("expected syntax");
            let actual = shift_generics(&input);
            // syn synthesizes a colon when printing a newly populated bounds
            // list; compare emitted syntax, not optional punctuation storage.
            assert_eq!(
                actual.to_token_stream().to_string(),
                expected.generics.to_token_stream().to_string(),
                "{source}"
            );
            assert_eq!(actual.where_clause, expected.generics.where_clause);
        }
    }
}
