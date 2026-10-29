//! Generated bindings admitted against the input's generic namespace.

use std::collections::HashSet;

use proc_macro2::{Ident, Span};
use quote::{ToTokens, quote};
use syn::ext::IdentExt;

/// Rust identifier identity, independent of its optional raw spelling.
/// Original identifiers remain separate for source-preserving emission.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct IdentifierName(Ident);

impl IdentifierName {
    pub(crate) fn of(ident: &Ident) -> Self {
        Self(ident.unraw())
    }

    pub(crate) fn is(&self, name: &str) -> bool {
        self.0 == name
    }
}

/// Preserve generic argument kinds at application sites. A bare const name
/// can otherwise resolve as a type/variant (for example the prelude's `Ok`).
pub(crate) struct TypeArguments<'a>(&'a syn::Generics);

impl<'a> TypeArguments<'a> {
    pub(crate) fn new(generics: &'a syn::Generics) -> Self {
        Self(generics)
    }
}

impl ToTokens for TypeArguments<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        if self.0.params.is_empty() {
            return;
        }
        let arguments = self.0.params.iter().map(|param| match param {
            syn::GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                quote! { #lifetime }
            }
            syn::GenericParam::Type(param) => {
                let name = &param.ident;
                quote! { #name }
            }
            syn::GenericParam::Const(param) => {
                let name = &param.ident;
                quote! { { #name } }
            }
        });
        tokens.extend(quote! { <#(#arguments),*> });
    }
}

/// One reservation owner for method parameters, locals and field patterns.
#[derive(Clone)]
pub(crate) struct BindingScope {
    reserved: HashSet<IdentifierName>,
}

impl BindingScope {
    pub(crate) fn new(generics: &syn::Generics) -> Self {
        Self {
            reserved: generics
                .params
                .iter()
                .map(|param| match param {
                    syn::GenericParam::Type(param) => IdentifierName::of(&param.ident),
                    syn::GenericParam::Const(param) => IdentifierName::of(&param.ident),
                    syn::GenericParam::Lifetime(param) => IdentifierName::of(&param.lifetime.ident),
                })
                .collect(),
        }
    }

    pub(crate) fn reserve(&mut self, base: &str) -> Ident {
        let mut name = base.to_owned();
        loop {
            let ident = Ident::new(&name, Span::mixed_site());
            if self.reserved.insert(IdentifierName::of(&ident)) {
                return ident;
            }
            name.push('_');
        }
    }
}

/// Emitters consume these names instead of introducing unadmitted bindings.
pub(crate) struct SemanticBindings {
    pub(crate) other: Ident,
    pub(crate) path: Ident,
    pub(crate) report: Ident,
    pub(crate) context: Ident,
    pub(crate) previous_location: Ident,
    pub(crate) left_variant: Ident,
    pub(crate) right_variant: Ident,
    pub(crate) diff_block: syn::Lifetime,
}

impl SemanticBindings {
    pub(crate) fn admit(scope: &mut BindingScope) -> Self {
        Self {
            other: scope.reserve("other"),
            path: scope.reserve("path"),
            report: scope.reserve("report"),
            context: scope.reserve("ctx"),
            previous_location: scope.reserve("__prev_location"),
            left_variant: scope.reserve("left_variant"),
            right_variant: scope.reserve("right_variant"),
            diff_block: syn::Lifetime::new(
                &format!("'{}", scope.reserve("__tb_diff")),
                Span::mixed_site(),
            ),
        }
    }
}

/// Only span-shift emission carries mapping and movement parameters.
pub(crate) struct ShiftBindings {
    pub(crate) offset: Ident,
    pub(crate) delta: Ident,
    pub(crate) map: Ident,
    pub(crate) map_error: Ident,
}

impl ShiftBindings {
    pub(crate) fn admit(scope: &mut BindingScope) -> Self {
        Self {
            offset: scope.reserve("offset"),
            delta: scope.reserve("delta"),
            map: scope.reserve("__tb_map"),
            map_error: scope.reserve("__TalkbankSpanMapError"),
        }
    }
}
