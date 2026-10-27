//! Parser API types for CHAT format parsing.

mod chat_parser;
mod context;
mod kind;
pub use kind::ParserKind;
mod outcome;
mod product;
mod single_utterance;

pub use chat_parser::ChatParser;
pub use context::FragmentSemanticContext;
pub use outcome::ParseOutcome;
pub use product::ParseProduct;
pub use single_utterance::{SingleUtterance, UtteranceCardinalityError};
