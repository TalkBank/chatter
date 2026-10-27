//! Scoped annotation parsing
//!
//! Handles parsing of base annotations like [*], [=], [+], etc.
//! Retrace markers remain in the ordered run beside content annotations;
//! producer failures travel outside that run rather than erasing a marker.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope>

mod list;
mod single;
mod symbols;

pub(in crate::parser) use list::parse_scoped_annotations;
pub(crate) use single::ParsedAnnotation;
