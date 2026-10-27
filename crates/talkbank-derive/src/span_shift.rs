//! Recursive span movement from an admitted field plan.

use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::visit::Visit;

use crate::field_plan::{BodyPlan, FieldsPlan, InputPlan, Shape, Shift};

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

/// Reconstruct all fields, retaining opaque fields without cloning them.
fn mapped_fields(fields: &FieldsPlan, constructor: TokenStream) -> TokenStream {
    let bindings = fields
        .fields
        .iter()
        .map(|field| {
            let member = &field.member;
            let binding = &field.left;
            match fields.shape {
                Shape::Named => quote! { #member: #binding },
                Shape::Tuple | Shape::Unit => quote! { #binding },
            }
        })
        .collect::<Vec<_>>();
    let values = fields.fields.iter().map(|field| {
        let member = &field.member;
        let binding = &field.left;
        let value = if field.included() {
            quote! { talkbank_model::SpanShift::try_map_spans(#binding, __tb_map)? }
        } else {
            quote! { #binding }
        };
        match fields.shape {
            Shape::Named => quote! { #member: #value },
            Shape::Tuple | Shape::Unit => value,
        }
    });
    match fields.shape {
        Shape::Named => {
            quote! { #constructor { #(#bindings,)* } => Ok(#constructor { #(#values,)* }) }
        }
        Shape::Tuple => quote! { #constructor(#(#bindings,)*) => Ok(#constructor(#(#values,)*)) },
        Shape::Unit => quote! { #constructor => Ok(#constructor) },
    }
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
    let map_arms = match &input.body {
        BodyPlan::Struct(fields) => mapped_fields(fields, quote! { Self }),
        BodyPlan::Enum(variants) => {
            let arms = variants.iter().map(|variant| {
                let variant_name = &variant.name;
                mapped_fields(&variant.fields, quote! { Self::#variant_name })
            });
            quote! { #(#arms,)* }
        }
    };
    let mut error_name = String::from("__TalkbankSpanMapError");
    while input.generics.params.iter().any(|param| match param {
        syn::GenericParam::Type(param) => param.ident == error_name,
        syn::GenericParam::Const(param) => param.ident == error_name,
        syn::GenericParam::Lifetime(param) => param.lifetime.ident == error_name,
    }) {
        error_name.push('_');
    }
    let error = syn::Ident::new(&error_name, proc_macro2::Span::mixed_site());
    quote! {
        impl #impl_generics talkbank_model::SpanShift for #name #ty_generics #where_clause {
            fn shift_spans_after(&mut self, offset: usize, delta: isize) { #body }
            fn try_map_spans<#error>(
                self,
                __tb_map: &mut impl FnMut(talkbank_model::Span) -> Result<talkbank_model::Span, #error>,
            ) -> Result<Self, #error> {
                match self { #map_arms }
            }
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
