//! Structured diff emission from the same admitted plan as equality.

use proc_macro2::TokenStream;
use quote::quote;

use crate::bindings::{SemanticBindings, TypeArguments};
use crate::field_plan::{BodyPlan, FieldsPlan, InputPlan, Semantic};

fn field_diffs(fields: &FieldsPlan, bound: bool, bindings: &SemanticBindings) -> TokenStream {
    let other = &bindings.other;
    let path = &bindings.path;
    let report = &bindings.report;
    let ctx = &bindings.context;
    let previous = &bindings.previous_location;
    let block = &bindings.diff_block;
    let diffs = fields.active().map(|field| {
        let push_path = field.push_path(path);
        let member = &field.member;
        let left = &field.left;
        let right = &field.right;
        let call = if bound {
            quote! { crate::model::SemanticDiff::semantic_diff_into(#left, #right, #path, #report, #ctx); }
        } else {
            quote! { crate::model::SemanticDiff::semantic_diff_into(&self.#member, &#other.#member, #path, #report, #ctx); }
        };
        quote! {
            if #report.is_truncated() { break #block; }
            #push_path
            #call
            #path.pop();
        }
    });
    let span = fields.span_expr(bound);
    quote! {
        let #previous = #ctx.push_location(#span);
        #block: { #(#diffs)* }
        #ctx.pop_location(#previous);
    }
}

/// Emit diff traversal; all field decisions were made by the producer.
pub(crate) fn impl_semantic_diff(input: &InputPlan<Semantic>) -> TokenStream {
    let name = &input.name;
    let bindings = &input.bindings;
    let other = &bindings.other;
    let path = &bindings.path;
    let report = &bindings.report;
    let ctx = &bindings.context;
    let left_variant = &bindings.left_variant;
    let right_variant = &bindings.right_variant;
    let (impl_generics, _, where_clause) = input.generics.split_for_impl();
    let ty_generics = TypeArguments::new(&input.generics);
    let body = match &input.body {
        BodyPlan::Enum(variants) if variants.is_empty() => {
            quote! { let _ = (#other, #path, #report, #ctx); match *self {} }
        }
        BodyPlan::Struct(fields) => {
            if fields.active().next().is_none() {
                quote! { let _ = (#other, #path, #report, #ctx); }
            } else {
                field_diffs(fields, false, bindings)
            }
        }
        BodyPlan::Enum(variants) => {
            let arms = variants.iter().map(|variant| {
                let left = variant.fields.pattern(&variant.name, false, true);
                let right = variant.fields.pattern(&variant.name, true, true);
                let diffs = field_diffs(&variant.fields, true, bindings);
                quote! { (#left, #right) => { #diffs } }
            });
            let names: Vec<_> = variants
                .iter()
                .map(|variant| variant.fields.variant_name_arm(&variant.name))
                .collect();
            quote! {
                if #report.is_truncated() { return; }
                match (self, #other) {
                    #(#arms,)*
                    _ => {
                        let #left_variant = match self { #(#names)* };
                        let #right_variant = match #other { #(#names)* };
                        #report.push_with_context(#path, crate::model::SemanticDiffKind::VariantMismatch,
                            #left_variant, #right_variant, #ctx);
                    }
                }
            }
        }
    };
    quote! {
        #[allow(non_upper_case_globals, non_camel_case_types)] // Generic declarations were linted on the input type.
        impl #impl_generics crate::model::SemanticDiff for #name #ty_generics #where_clause {
            fn semantic_diff_into(&self, #other: &Self, #path: &mut crate::model::SemanticPath,
                #report: &mut crate::model::SemanticDiffReport, #ctx: &mut crate::model::SemanticDiffContext) {
                #body
            }
        }
    }
}
