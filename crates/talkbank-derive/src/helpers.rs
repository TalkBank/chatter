//! Helper utilities shared by derive macros that work with span-carrying fields.
//!
//! These helpers retain canonical span-field discovery for semantic diff context.
//! The admitted field plan owns metadata and span-normalization emission.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use syn::Fields;

/// Metadata for a named field whose type can carry a span.
#[derive(Clone)]
pub struct SpanFieldInfo {
    pub ident: syn::Ident,
    pub is_option: bool,
}

/// Selects the named field derive macros should treat as the canonical span.
///
/// Preference order is: `span`, then `speaker_span`, then last span-like field.
pub fn find_span_field(fields: &Fields) -> Option<SpanFieldInfo> {
    let mut candidate: Option<SpanFieldInfo> = None;
    let mut speaker_candidate: Option<SpanFieldInfo> = None;

    for field in fields {
        let ident = match &field.ident {
            Some(ident) => ident.clone(),
            None => continue,
        };
        let name = ident.to_string();
        if !is_span_like(&field.ty) {
            continue;
        }
        let info = SpanFieldInfo {
            ident,
            is_option: is_option_span(&field.ty),
        };
        if name == "span" {
            return Some(info);
        }
        if name == "speaker_span" {
            speaker_candidate = Some(info.clone());
        }
        candidate = Some(info);
    }

    speaker_candidate.or(candidate)
}

/// Metadata for a tuple-field index whose type can carry a span.
pub struct SpanIndexInfo {
    pub index: usize,
    pub is_option: bool,
}

/// Finds the first tuple field that looks like `Span` or `Option<Span>`.
pub fn find_span_index(fields: &Fields) -> Option<SpanIndexInfo> {
    let Fields::Unnamed(fields) = fields else {
        return None;
    };

    for (idx, field) in fields.unnamed.iter().enumerate() {
        if !is_span_like(&field.ty) {
            continue;
        }
        return Some(SpanIndexInfo {
            index: idx,
            is_option: is_option_span(&field.ty),
        });
    }

    None
}

/// Return whether a type can be normalized as a span.
fn is_span_like(ty: &syn::Type) -> bool {
    is_span_type(ty) || is_option_span(ty)
}

/// Recognizes the syntactic path spelling `Span`, not resolved Rust type identity.
fn is_span_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Span";
    }
    false
}

/// Returns whether the type is `Option<Span>`.
fn is_option_span(ty: &syn::Type) -> bool {
    let syn::Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    if segment.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    for arg in &args.args {
        if let syn::GenericArgument::Type(inner) = arg
            && is_span_type(inner)
        {
            return true;
        }
    }
    false
}
