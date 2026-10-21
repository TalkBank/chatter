//! Structured diff emission from the same admitted plan as equality.

use proc_macro2::TokenStream;
use quote::quote;

use crate::field_plan::{BodyPlan, FieldsPlan, InputPlan, Semantic};

fn field_diffs(fields: &FieldsPlan, bound: bool) -> TokenStream {
    let diffs = fields.active().map(|field| {
        let push_path = field.push_path();
        let member = &field.member;
        let left = &field.left;
        let right = &field.right;
        let call = if bound {
            quote! { crate::model::SemanticDiff::semantic_diff_into(#left, #right, path, report, ctx); }
        } else {
            quote! { crate::model::SemanticDiff::semantic_diff_into(&self.#member, &other.#member, path, report, ctx); }
        };
        quote! {
            if report.is_truncated() { break '__tb_diff; }
            #push_path
            #call
            path.pop();
        }
    });
    let span = fields.span_expr(bound);
    quote! {
        let __prev_location = ctx.push_location(#span);
        '__tb_diff: { #(#diffs)* }
        ctx.pop_location(__prev_location);
    }
}

/// Emit diff traversal; all field decisions were made by the producer.
pub(crate) fn impl_semantic_diff(input: &InputPlan<Semantic>) -> TokenStream {
    let name = &input.name;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let body = match &input.body {
        BodyPlan::Enum(variants) if variants.is_empty() => {
            quote! { let _ = (other, path, report, ctx); match *self {} }
        }
        BodyPlan::Struct(fields) => {
            if fields.active().next().is_none() {
                quote! { let _ = (other, path, report, ctx); }
            } else {
                field_diffs(fields, false)
            }
        }
        BodyPlan::Enum(variants) => {
            let arms = variants.iter().map(|variant| {
                let left = variant.fields.pattern(&variant.name, false, true);
                let right = variant.fields.pattern(&variant.name, true, true);
                let diffs = field_diffs(&variant.fields, true);
                quote! { (#left, #right) => { #diffs } }
            });
            let names: Vec<_> = variants
                .iter()
                .map(|variant| variant.fields.variant_name_arm(&variant.name))
                .collect();
            quote! {
                if report.is_truncated() { return; }
                match (self, other) {
                    #(#arms,)*
                    _ => {
                        let left_variant = match self { #(#names)* };
                        let right_variant = match other { #(#names)* };
                        report.push_with_context(path, crate::model::SemanticDiffKind::VariantMismatch,
                            left_variant, right_variant, ctx);
                    }
                }
            }
        }
    };
    quote! {
        impl #impl_generics crate::model::SemanticDiff for #name #ty_generics #where_clause {
            fn semantic_diff_into(&self, other: &Self, path: &mut crate::model::SemanticPath,
                report: &mut crate::model::SemanticDiffReport, ctx: &mut crate::model::SemanticDiffContext) {
                #body
            }
        }
    }
}
