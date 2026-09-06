//! Regenerate the curated CHECK mapping inventory (not runtime parity).
fn main() -> Result<(), talkbank_parser_tests::test_error::TestError> {
    talkbank_parser_tests::check_mapping_audit::regenerate()
}
