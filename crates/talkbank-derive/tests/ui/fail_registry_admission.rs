use talkbank_derive::error_code_enum;

#[error_code_enum(unexpected)]
enum OuterArguments {
    #[code("E001")]
    UnknownError,
}

#[error_code_enum]
enum DuplicateCode {
    #[code("E001")]
    #[code("E002")]
    UnknownError,
}

#[error_code_enum]
enum MalformedCompanion {
    #[code(42)]
    #[code("E001")]
    UnknownError,
}

#[error_code_enum]
enum UnknownStatus {
    #[code("E001")]
    #[status(typo)]
    UnknownError,
}

#[error_code_enum]
enum DuplicateStatus {
    #[code("E001")]
    #[status(planned)]
    #[status(planned)]
    UnknownError,
}

#[error_code_enum]
enum ConflictingStatus {
    #[code("E001")]
    #[status(planned)]
    #[status(opt_in)]
    UnknownError,
}

#[error_code_enum]
enum GenericRegistry<const N: usize> {
    #[code("E001")]
    UnknownError,
}

#[error_code_enum]
enum RegistryWhereClause where (): Clone {
    #[code("E001")]
    UnknownError,
}

#[error_code_enum]
#[repr(u8)]
enum ExplicitDiscriminant {
    #[code("E001")]
    UnknownError = 7,
}

fn main() {}
