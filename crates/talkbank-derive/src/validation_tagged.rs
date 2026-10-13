//! Validation tag syntax is admitted before any implementation is emitted.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Variant};

use crate::metadata;

#[derive(Clone, Copy)]
enum DerivedTag {
    Clean,
    Warning,
    Error,
}

impl DerivedTag {
    fn to_tokens(self) -> TokenStream {
        match self {
            Self::Clean => quote! { crate::model::ValidationTag::Clean },
            Self::Warning => quote! { crate::model::ValidationTag::Warning },
            Self::Error => quote! { crate::model::ValidationTag::Error },
        }
    }
}

enum VariantShape {
    Unit,
    Tuple,
    Named,
}

struct TaggedVariant {
    name: syn::Ident,
    shape: VariantShape,
    tag: DerivedTag,
}

/// A complete tagging enum owns the actual generics and admitted variant policy.
pub(crate) struct TaggedEnum {
    name: syn::Ident,
    generics: syn::Generics,
    variants: Vec<TaggedVariant>,
}

impl TaggedEnum {
    pub(crate) fn admit(input: DeriveInput) -> syn::Result<Self> {
        let Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input,
                "ValidationTagged can only be derived for enums",
            ));
        };
        metadata::require_variant_location(&input.attrs, "validation_tag")?;
        let variants = data
            .variants
            .iter()
            .map(|variant| {
                for field in &variant.fields {
                    metadata::require_variant_location(&field.attrs, "validation_tag")?;
                }
                let tag = resolve_variant_tag(variant)?;
                let shape = match variant.fields {
                    Fields::Unit => VariantShape::Unit,
                    Fields::Unnamed(_) => VariantShape::Tuple,
                    Fields::Named(_) => VariantShape::Named,
                };
                Ok(TaggedVariant {
                    name: variant.ident.clone(),
                    shape,
                    tag,
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;
        Ok(Self {
            name: input.ident,
            generics: input.generics,
            variants,
        })
    }
}

/// Emission cannot accept raw syntax or resolve an annotation a second time.
pub(crate) fn emit_tagged_enum(input: &TaggedEnum) -> TokenStream {
    let name = &input.name;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let arms = input.variants.iter().map(|variant| {
        let name = &variant.name;
        let tag = variant.tag.to_tokens();
        match variant.shape {
            VariantShape::Unit => quote! { Self::#name => #tag },
            VariantShape::Tuple => quote! { Self::#name(..) => #tag },
            VariantShape::Named => quote! { Self::#name { .. } => #tag },
        }
    });
    quote! {
        impl #impl_generics crate::model::ValidationTagged for #name #ty_generics #where_clause {
            fn validation_tag(&self) -> crate::model::ValidationTag {
                match self { #(#arms,)* }
            }
        }
    }
}

fn resolve_variant_tag(variant: &Variant) -> syn::Result<DerivedTag> {
    if let Some(attr) = metadata::unique(&variant.attrs, "validation_tag")? {
        let ident = attr.parse_args::<syn::Ident>()?;
        return match ident.to_string().as_str() {
            "clean" => Ok(DerivedTag::Clean),
            "warning" => Ok(DerivedTag::Warning),
            "error" => Ok(DerivedTag::Error),
            _ => Err(syn::Error::new_spanned(
                ident,
                "expected one of: clean, warning, error",
            )),
        };
    }
    let name = variant.ident.to_string();
    // Absence selects the documented naming policy, not an error fallback.
    if name.ends_with("Error") {
        Ok(DerivedTag::Error)
    } else if name.ends_with("Warning") || name.ends_with("Unsupported") {
        Ok(DerivedTag::Warning)
    } else {
        Ok(DerivedTag::Clean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_reject_malformed_duplicate_and_misplaced_owned_attributes() {
        for flag in [
            "",
            "()",
            "(urgent)",
            " = \"error\"",
            "(error,)",
            "(error, clean)",
            "(error = true)",
        ] {
            let syntax =
                syn::parse_str(&format!("enum Tags {{ #[validation_tag{flag}] Good }}")).unwrap();
            assert!(TaggedEnum::admit(syntax).is_err(), "{flag}");
        }
        for source in [
            "enum Tags { #[validation_tag(error)] #[validation_tag(clean)] Good }",
            "#[validation_tag(error)] enum Tags { Good }",
            "enum Tags { Good { #[validation_tag(error)] value: u32 } }",
            "enum Tags { Good(#[validation_tag(error)] u32) }",
            "struct Tags;",
            "union Tags { value: u32 }",
        ] {
            assert!(
                TaggedEnum::admit(syn::parse_str(source).unwrap()).is_err(),
                "{source}"
            );
        }
    }
}
