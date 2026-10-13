//! Registry syntax becomes admitted variants, ordered variants, then a complete registry.

use proc_macro2::TokenStream;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, LitStr, Visibility};

use crate::metadata;

/// Exclusive enforcement state; malformed syntax cannot become Active.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Enforcement {
    Active,
    Planned,
    OptIn,
}

impl Enforcement {
    fn admit(attrs: &[Attribute]) -> syn::Result<Self> {
        let Some(attr) = metadata::unique(attrs, "status")? else {
            return Ok(Self::Active);
        };
        let ident = attr.parse_args::<Ident>()?;
        match ident.to_string().as_str() {
            "planned" => Ok(Self::Planned),
            "opt_in" => Ok(Self::OptIn),
            _ => Err(syn::Error::new_spanned(
                ident,
                "expected one of: planned, opt_in",
            )),
        }
    }
}

/// Existing sort policy: split at the first ASCII digit and parse the suffix as u32.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct CodeKey {
    prefix: String,
    number: u32,
}

/// The source literal and its once-parsed key are issued together.
struct CodeLiteral {
    literal: LitStr,
    key: CodeKey,
}

impl CodeLiteral {
    fn admit(literal: LitStr) -> syn::Result<Self> {
        let value = literal.value();
        let invalid = || {
            syn::Error::new_spanned(
                &literal,
                format!("code {value:?} must contain an ASCII digit and a u32 decimal suffix"),
            )
        };
        let split = value
            .find(|ch: char| ch.is_ascii_digit())
            .ok_or_else(invalid)?;
        let (prefix, digits) = value.split_at(split);
        let number = digits.parse().map_err(|_| invalid())?;
        let key = CodeKey {
            prefix: prefix.to_owned(),
            number,
        };
        Ok(Self { literal, key })
    }
}

/// A source variant with complete, strictly parsed metadata.
pub(crate) struct RegistryVariant {
    name: Ident,
    code: CodeLiteral,
    enforcement: Enforcement,
    attributes: Vec<Attribute>,
}

impl RegistryVariant {
    pub(crate) fn name(&self) -> &Ident {
        &self.name
    }
    pub(crate) fn code(&self) -> &LitStr {
        &self.code.literal
    }
    pub(crate) fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

struct RegistryParts {
    name: Ident,
    visibility: Visibility,
    attributes: Vec<Attribute>,
    variants: Vec<RegistryVariant>,
}

/// Invocation syntax belongs to one macro boundary, including its arguments.
pub(crate) struct RegistrySyntax(DeriveInput);

impl RegistrySyntax {
    pub(crate) fn parse(arguments: TokenStream, item: TokenStream) -> syn::Result<Self> {
        if !arguments.is_empty() {
            return Err(syn::Error::new_spanned(
                arguments,
                "error_code_enum does not accept arguments",
            ));
        }
        syn::parse2(item).map(Self)
    }

    pub(crate) fn admit(self) -> syn::Result<AdmittedVariants> {
        let input = self.0;
        let Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input,
                "error_code_enum can only be used on enums",
            ));
        };
        if !input.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "error_code_enum does not support generic parameters",
            ));
        }
        if let Some(clause) = &input.generics.where_clause {
            return Err(syn::Error::new_spanned(
                clause,
                "error_code_enum does not support where clauses",
            ));
        }
        metadata::require_variant_location(&input.attrs, "code")?;
        metadata::require_variant_location(&input.attrs, "status")?;
        let variants = data
            .variants
            .iter()
            .map(|variant| {
                if !matches!(variant.fields, Fields::Unit) {
                    return Err(syn::Error::new_spanned(
                        &variant.fields,
                        "error_code_enum only supports unit variants",
                    ));
                }
                if let Some((_, expression)) = &variant.discriminant {
                    return Err(syn::Error::new_spanned(
                        expression,
                        "error_code_enum does not support explicit discriminants",
                    ));
                }
                let Some(attr) = metadata::unique(&variant.attrs, "code")? else {
                    return Err(syn::Error::new_spanned(
                        variant,
                        format!(
                            "Variant {} missing #[code(\"...\")] attribute",
                            variant.ident
                        ),
                    ));
                };
                let code = CodeLiteral::admit(attr.parse_args::<LitStr>()?)?;
                let enforcement = Enforcement::admit(&variant.attrs)?;
                let attributes = variant
                    .attrs
                    .iter()
                    .filter(|attr| !attr.path().is_ident("code") && !attr.path().is_ident("status"))
                    .cloned()
                    .collect();
                Ok(RegistryVariant {
                    name: variant.ident.clone(),
                    code,
                    enforcement,
                    attributes,
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;
        Ok(AdmittedVariants(RegistryParts {
            name: input.ident,
            visibility: input.vis,
            attributes: input.attrs,
            variants,
        }))
    }
}

/// Valid variant metadata is not yet evidence of declaration order.
pub(crate) struct AdmittedVariants(RegistryParts);

impl AdmittedVariants {
    pub(crate) fn order(self) -> syn::Result<OrderedVariants> {
        for (before, after) in self.0.variants.iter().zip(self.0.variants.iter().skip(1)) {
            if after.code.key <= before.code.key {
                return Err(syn::Error::new_spanned(
                    &after.name,
                    format!(
                        "error codes must be declared in ascending order: {}{} follows {}{}. \
                     Declaration order IS the sort order (see the `Ord` derive), so move \
                     this variant rather than relaxing the rule.",
                        after.code.key.prefix,
                        after.code.key.number,
                        before.code.key.prefix,
                        before.code.key.number
                    ),
                ));
            }
        }
        Ok(OrderedVariants(self.0))
    }
}

/// Ordered variants cannot emit an enum until its required unknown variant exists.
pub(crate) struct OrderedVariants(RegistryParts);

impl OrderedVariants {
    pub(crate) fn complete(self) -> syn::Result<CompleteRegistry> {
        const UNKNOWN_VARIANT: &str = "UnknownError";
        if !self
            .0
            .variants
            .iter()
            .any(|variant| variant.name == UNKNOWN_VARIANT)
        {
            return Err(syn::Error::new_spanned(
                &self.0.name,
                format!("ErrorCode enum must have {UNKNOWN_VARIANT} variant"),
            ));
        }
        Ok(CompleteRegistry(self.0))
    }
}

/// The only registry input accepted by emission; construction stays in this module.
pub(crate) struct CompleteRegistry(RegistryParts);

impl CompleteRegistry {
    pub(crate) fn name(&self) -> &Ident {
        &self.0.name
    }
    pub(crate) fn visibility(&self) -> &Visibility {
        &self.0.visibility
    }
    pub(crate) fn attributes(&self) -> &[Attribute] {
        &self.0.attributes
    }
    pub(crate) fn variants(&self) -> &[RegistryVariant] {
        &self.0.variants
    }
    pub(crate) fn variants_in(
        &self,
        enforcement: Enforcement,
    ) -> impl Iterator<Item = &RegistryVariant> {
        self.0
            .variants
            .iter()
            .filter(move |variant| variant.enforcement == enforcement)
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
