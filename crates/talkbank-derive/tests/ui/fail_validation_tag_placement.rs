use talkbank_derive::ValidationTagged;
pub use talkbank_model::model;

#[derive(ValidationTagged)]
#[validation_tag(error)]
enum Container {
    Clean,
}

#[derive(ValidationTagged)]
enum NamedField {
    Clean {
        #[validation_tag(error)]
        value: u32,
    },
}

#[derive(ValidationTagged)]
enum TupleField {
    Clean(#[validation_tag(error)] u32),
}

fn main() {}
