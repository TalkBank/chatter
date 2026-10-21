use talkbank_derive::SemanticEq;
use talkbank_model::Span;
// The SemanticEq expansion names `crate::model` and `crate::Location`, so a
// crate deriving it brings both into its root.
use talkbank_model::Location;
use talkbank_model::model;

// Verify SemanticEq can be derived on a struct with skip attribute
#[derive(Debug, Clone, SemanticEq)]
struct Simple {
    value: String,
    #[semantic_eq(skip)]
    span: Span,
}

fn main() {}
