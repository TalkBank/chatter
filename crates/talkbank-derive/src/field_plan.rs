//! Admission shared by field derives; emitters cannot consume unchecked syntax.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Member};

/// Admission policy for each field derive.
pub(crate) trait DeriveKind {
    const ATTRIBUTE: &'static str;
    const NAME: &'static str;
    const HAS_CONTEXT: bool;
}

/// Equality and diff must consume the same semantic plan.
pub(crate) struct Semantic;
impl DeriveKind for Semantic {
    const ATTRIBUTE: &'static str = "semantic_eq";
    const NAME: &'static str = "SemanticEq";
    const HAS_CONTEXT: bool = true;
}

/// Span movement cannot consume a semantic exclusion plan.
pub(crate) struct Shift;
impl DeriveKind for Shift {
    const ATTRIBUTE: &'static str = "span_shift";
    const NAME: &'static str = "SpanShift";
    const HAS_CONTEXT: bool = false;
}

/// Admitted participation, not a fallback for malformed metadata.
#[derive(Clone, Copy)]
enum Participation {
    Include,
    Skip,
}

fn admit_participation<K: DeriveKind>(attrs: &[Attribute]) -> syn::Result<Participation> {
    let mut matching = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(K::ATTRIBUTE));
    let Some(attr) = matching.next() else {
        return Ok(Participation::Include);
    };
    if let Some(duplicate) = matching.next() {
        return Err(syn::Error::new_spanned(
            duplicate,
            format!("duplicate #[{}(skip)] attribute", K::ATTRIBUTE),
        ));
    }
    let flag = attr.parse_args::<Ident>()?;
    if flag != "skip" {
        return Err(syn::Error::new_spanned(flag, "expected `skip`"));
    }
    Ok(Participation::Skip)
}

fn require_field_location<K: DeriveKind>(attrs: &[Attribute]) -> syn::Result<()> {
    if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident(K::ATTRIBUTE)) {
        return Err(syn::Error::new_spanned(
            attr,
            format!("#[{}(skip)] is only supported on fields", K::ATTRIBUTE),
        ));
    }
    Ok(())
}

/// A field whose metadata and generated bindings have already been admitted.
pub(crate) struct FieldPlan {
    pub(crate) member: Member,
    pub(crate) ty: syn::Type,
    pub(crate) left: Ident,
    pub(crate) right: Ident,
    participation: Participation,
}

impl FieldPlan {
    pub(crate) fn included(&self) -> bool {
        matches!(self.participation, Participation::Include)
    }

    pub(crate) fn push_path(&self) -> TokenStream {
        match &self.member {
            Member::Named(name) => {
                let literal = syn::LitStr::new(&name.to_string(), name.span());
                quote! { path.push_field(#literal); }
            }
            Member::Unnamed(index) => {
                let index = index.index as usize;
                quote! { path.push_index(#index); }
            }
        }
    }
}

/// Pattern shape independent of field participation.
pub(crate) enum Shape {
    Named,
    Tuple,
    Unit,
}

/// The actual selected field and its normalization, issued together.
struct ContextSpan {
    member: Member,
    binding: Ident,
    kind: crate::helpers::SpanFieldKind,
}

/// A complete admitted field group, including its existing context policy.
pub(crate) struct FieldsPlan {
    pub(crate) shape: Shape,
    pub(crate) fields: Vec<FieldPlan>,
    context: Option<ContextSpan>,
}

impl FieldsPlan {
    fn admit<K: DeriveKind>(fields: &Fields, enum_variant: bool) -> syn::Result<Self> {
        let shape = match fields {
            Fields::Named(_) => Shape::Named,
            Fields::Unnamed(_) => Shape::Tuple,
            Fields::Unit => Shape::Unit,
        };
        let plans = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let member = match &field.ident {
                    Some(name) => Member::Named(name.clone()),
                    None => Member::Unnamed(syn::Index::from(index)),
                };
                Ok(FieldPlan {
                    member,
                    ty: field.ty.clone(),
                    left: format_ident!("__tb_left_{index}", span = Span::mixed_site()),
                    right: format_ident!("__tb_right_{index}", span = Span::mixed_site()),
                    participation: admit_participation::<K>(&field.attrs)?,
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;
        let context = if K::HAS_CONTEXT {
            let selected = match fields {
                    Fields::Named(_) => crate::helpers::find_span_field(fields).and_then(|span| {
                        plans.iter().find(|plan| matches!(&plan.member, Member::Named(name) if name == &span.ident))
                            .map(|plan| (plan, span.kind))
                    }),
                    Fields::Unnamed(_) if enum_variant => crate::helpers::find_span_index(fields)
                        .map(|span| (&plans[span.index], span.kind)),
                    Fields::Unnamed(_) | Fields::Unit => None,
                };
            selected.map(|(plan, kind)| ContextSpan {
                member: plan.member.clone(),
                binding: plan.left.clone(),
                kind,
            })
        } else {
            None
        };
        Ok(Self {
            shape,
            fields: plans,
            context,
        })
    }

    pub(crate) fn active(&self) -> impl Iterator<Item = &FieldPlan> {
        self.fields.iter().filter(|field| field.included())
    }

    /// Bind only semantic/shift participants, plus the selected left context when requested.
    pub(crate) fn pattern(&self, variant: &Ident, right: bool, with_context: bool) -> TokenStream {
        let bindings = self.fields.iter().map(|field| {
            let ident = if right { &field.right } else { &field.left };
            let context = with_context
                && !right
                && self
                    .context
                    .as_ref()
                    .is_some_and(|span| span.member == field.member);
            let binding = if field.included() || context {
                quote! { #ident }
            } else {
                quote! { _ }
            };
            match &field.member {
                Member::Named(member) => quote! { #member: #binding },
                Member::Unnamed(_) => binding,
            }
        });
        match self.shape {
            Shape::Named => quote! { Self::#variant { #(#bindings),* } },
            Shape::Tuple => quote! { Self::#variant(#(#bindings),*) },
            Shape::Unit => quote! { Self::#variant },
        }
    }

    pub(crate) fn variant_name_arm(&self, variant: &Ident) -> TokenStream {
        match self.shape {
            Shape::Named => quote! { Self::#variant { .. } => stringify!(#variant), },
            Shape::Tuple => quote! { Self::#variant(..) => stringify!(#variant), },
            Shape::Unit => quote! { Self::#variant => stringify!(#variant), },
        }
    }

    pub(crate) fn span_expr(&self, bound: bool) -> TokenStream {
        let Some(context) = &self.context else {
            return quote! { crate::Location::Unlocated };
        };
        let member = &context.member;
        let binding = &context.binding;
        let value = if bound {
            quote! { *#binding }
        } else {
            quote! { self.#member }
        };
        match context.kind {
            crate::helpers::SpanFieldKind::Span => quote! { crate::Location::Located(#value) },
            // Parenthesized: `value` may be a deref, and `*x.method()` would
            // dereference the method's result.
            crate::helpers::SpanFieldKind::OptionSpan => {
                quote! { (#value).map_or(crate::Location::Unlocated, crate::Location::Located) }
            }
            crate::helpers::SpanFieldKind::Location => quote! { #value },
            crate::helpers::SpanFieldKind::OptionLocation => {
                quote! { (#value).unwrap_or(crate::Location::Unlocated) }
            }
        }
    }
}

/// An admitted variant owns its field plan.
pub(crate) struct VariantPlan {
    pub(crate) name: Ident,
    pub(crate) fields: FieldsPlan,
}

/// Unions cannot reach an emitter.
pub(crate) enum BodyPlan {
    Struct(FieldsPlan),
    Enum(Vec<VariantPlan>),
}

/// The sole field-derive input admission boundary.
pub(crate) struct InputPlan<K> {
    pub(crate) name: Ident,
    pub(crate) generics: syn::Generics,
    pub(crate) body: BodyPlan,
    kind: std::marker::PhantomData<K>,
}

impl<K: DeriveKind> InputPlan<K> {
    pub(crate) fn admit(input: DeriveInput) -> syn::Result<Self> {
        require_field_location::<K>(&input.attrs)?;
        let body = match &input.data {
            Data::Struct(data) => BodyPlan::Struct(FieldsPlan::admit::<K>(&data.fields, false)?),
            Data::Enum(data) => BodyPlan::Enum(
                data.variants
                    .iter()
                    .map(|variant| {
                        require_field_location::<K>(&variant.attrs)?;
                        Ok(VariantPlan {
                            name: variant.ident.clone(),
                            fields: FieldsPlan::admit::<K>(&variant.fields, true)?,
                        })
                    })
                    .collect::<syn::Result<Vec<_>>>()?,
            ),
            Data::Union(_) => {
                return Err(syn::Error::new_spanned(
                    &input,
                    format!("{} cannot be derived for unions", K::NAME),
                ));
            }
        };
        Ok(Self {
            name: input.ident,
            generics: input.generics,
            body,
            kind: std::marker::PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Boundary parsing contracts: unsupported input must not acquire a plan.
    fn rejects<K: DeriveKind>(source: &str) {
        let syntax = syn::parse_str(source).expect("valid Rust derive input");
        assert!(InputPlan::<K>::admit(syntax).is_err(), "{source}");
    }

    fn accepts<K: DeriveKind>(source: &str) {
        let syntax = syn::parse_str(source).expect("valid Rust derive input");
        assert!(InputPlan::<K>::admit(syntax).is_ok(), "{source}");
    }

    #[test]
    fn semantic_metadata_rejects_malformed_duplicate_and_misplaced_annotations() {
        for args in [
            "(skp)",
            "",
            "()",
            " = \"skip\"",
            "(skip, skip)",
            "(skip = true)",
            "(skip,)",
        ] {
            rejects::<Semantic>(&format!("struct S {{ #[semantic_eq{args}] value: u32 }}"));
            rejects::<Semantic>(&format!("struct S(#[semantic_eq{args}] u32);"));
        }
        rejects::<Semantic>("struct S { #[semantic_eq(skip)] #[semantic_eq(skip)] value: u32 }");
        rejects::<Semantic>("#[semantic_eq(skip)] struct S;");
        rejects::<Semantic>("enum S { #[semantic_eq(skip)] V }");
        rejects::<Semantic>("union S { value: u32 }");
    }

    #[test]
    fn shift_metadata_rejects_malformed_duplicate_and_misplaced_annotations() {
        for args in [
            "(skp)",
            "",
            "()",
            " = \"skip\"",
            "(skip, skip)",
            "(skip = true)",
            "(skip,)",
        ] {
            rejects::<Shift>(&format!("struct S {{ #[span_shift{args}] value: u32 }}"));
            rejects::<Shift>(&format!("enum S {{ V(#[span_shift{args}] u32) }}"));
        }
        rejects::<Shift>("struct S { #[span_shift(skip)] #[span_shift(skip)] value: u32 }");
        rejects::<Shift>("#[span_shift(skip)] struct S;");
        rejects::<Shift>("enum S { #[span_shift(skip)] V }");
        rejects::<Shift>("union S { value: u32 }");
    }

    #[test]
    fn each_plan_admits_only_its_own_policy_without_reinterpreting_other_attributes() {
        accepts::<Semantic>("struct S { #[semantic_eq(skip)] #[span_shift(skip)] value: u32 }");
        accepts::<Shift>("struct S { #[semantic_eq(skip)] #[span_shift(skip)] value: u32 }");
        accepts::<Semantic>("struct S;");
        accepts::<Shift>("struct S;");
        accepts::<Semantic>(
            "enum S { V(#[semantic_eq(skip)] u32), N { #[semantic_eq(skip)] r#type: u32 } }",
        );
        accepts::<Shift>(
            "enum S { V(#[span_shift(skip)] u32), N { #[span_shift(skip)] r#type: u32 } }",
        );
    }
}
