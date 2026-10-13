//! Strict cardinality and location admission for owned helper attributes.

use syn::Attribute;

/// Return absence or the unique attribute, never erase malformed companions.
pub(crate) fn unique<'a>(attrs: &'a [Attribute], name: &str) -> syn::Result<Option<&'a Attribute>> {
    let mut matching = attrs.iter().filter(|attr| attr.path().is_ident(name));
    let first = matching.next();
    if let Some(duplicate) = matching.next() {
        return Err(syn::Error::new_spanned(
            duplicate,
            format!("duplicate #[{name}(...)] attribute"),
        ));
    }
    Ok(first)
}

/// An owned variant annotation cannot acquire meaning at another location.
pub(crate) fn require_variant_location(attrs: &[Attribute], name: &str) -> syn::Result<()> {
    if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident(name)) {
        return Err(syn::Error::new_spanned(
            attr,
            format!("#[{name}(...)] is only supported on variants"),
        ));
    }
    Ok(())
}
