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

/// What a span-carrying field's type is, which decides how its diff context
/// span is read.
#[derive(Clone, Copy)]
pub enum SpanFieldKind {
    /// `Span` (may still be the `0..0` sentinel until the migration ends).
    Span,
    /// `Option<Span>`.
    OptionSpan,
    /// `Location`: located or not, by type.
    Location,
    /// `Option<Location>`.
    OptionLocation,
}

/// Metadata for a named field whose type can carry a span.
#[derive(Clone)]
pub struct SpanFieldInfo {
    pub ident: syn::Ident,
    pub kind: SpanFieldKind,
}

/// Selects the named field derive macros should treat as the canonical span.
///
/// Preference order is: `location` or `span`, then `speaker_span`, then the
/// last span-like field.
pub fn find_span_field(fields: &Fields) -> Option<SpanFieldInfo> {
    let mut candidate: Option<SpanFieldInfo> = None;
    let mut speaker_candidate: Option<SpanFieldInfo> = None;

    for field in fields {
        let ident = match &field.ident {
            Some(ident) => ident.clone(),
            None => continue,
        };
        let name = ident.to_string();
        let Some(kind) = span_field_kind(&field.ty) else {
            continue;
        };
        let info = SpanFieldInfo { ident, kind };
        if name == "span" || name == "location" {
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
    pub kind: SpanFieldKind,
}

/// Finds the first tuple field that looks like `Span` or `Option<Span>`.
pub fn find_span_index(fields: &Fields) -> Option<SpanIndexInfo> {
    let Fields::Unnamed(fields) = fields else {
        return None;
    };

    for (idx, field) in fields.unnamed.iter().enumerate() {
        if let Some(kind) = span_field_kind(&field.ty) {
            return Some(SpanIndexInfo { index: idx, kind });
        }
    }

    None
}

/// Classify a field type by its syntactic spelling (not resolved type
/// identity): `Span`, `Option<Span>`, `Location` or `Option<Location>`.
fn span_field_kind(ty: &syn::Type) -> Option<SpanFieldKind> {
    if is_named(ty, "Span") {
        return Some(SpanFieldKind::Span);
    }
    if is_named(ty, "Location") {
        return Some(SpanFieldKind::Location);
    }
    match option_inner(ty) {
        Some(inner) if is_named(inner, "Span") => Some(SpanFieldKind::OptionSpan),
        Some(inner) if is_named(inner, "Location") => Some(SpanFieldKind::OptionLocation),
        Some(_) | None => None,
    }
}

/// Whether the type path's last segment is `name`.
fn is_named(ty: &syn::Type, name: &str) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == name;
    }
    false
}

/// The `T` of an `Option<T>` spelling.
fn option_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    })
}
