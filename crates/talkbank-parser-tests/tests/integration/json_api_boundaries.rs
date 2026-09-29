//! Generic export failures are API evidence, not malformed CHAT specimens.

use serde::{Serialize, Serializer};
use std::cell::Cell;
use std::error::Error;
use talkbank_transform::json::{
    JsonError, JsonResult, to_json_pretty_unvalidated, to_json_pretty_validated,
    to_json_unvalidated, to_json_validated, validate_json_string,
};

/// A fallible downstream serializer must never be turned into schema invalidity,
/// retried, or replaced by a fabricated JSON value.
#[test]
fn json_export_variants_preserve_caller_serialization_failure() {
    struct RefusingValue {
        attempts: Cell<usize>,
    }
    impl Serialize for RefusingValue {
        fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            self.attempts.set(self.attempts.get() + 1);
            Err(serde::ser::Error::custom("caller declined export"))
        }
    }
    let exporters: [fn(&RefusingValue) -> JsonResult<String>; 4] = [
        to_json_validated,
        to_json_pretty_validated,
        to_json_unvalidated,
        to_json_pretty_unvalidated,
    ];
    for export in exporters {
        let value = RefusingValue {
            attempts: Cell::new(0),
        };
        let error = export(&value).expect_err("no output may escape a failed serializer");
        assert_eq!(
            value.attempts.get(),
            1,
            "do not retry a caller-owned serializer"
        );
        assert_eq!(
            error.source().expect("retained serde error").to_string(),
            "caller declined export"
        );
        let JsonError::SerializationError(cause) = error else {
            panic!("serialization failure is not schema invalidity");
        };
        assert!(cause.is_data());
        assert_eq!(cause.to_string(), "caller declined export");
    }
}

/// Generic intermediate values can be serialized without being CHAT documents.
/// Schema refusal is distinct from a malformed-JSON decoding failure.
#[test]
fn json_export_schema_policy_preserves_failure_category() {
    let intermediate = vec![1_u8, 2];
    for output in [
        to_json_unvalidated(&intermediate),
        to_json_pretty_unvalidated(&intermediate),
    ] {
        let output = output.expect("generic intermediate output is supported");
        assert_eq!(
            serde_json::from_str::<Vec<u8>>(&output).expect("JSON"),
            intermediate
        );
        assert!(matches!(
            validate_json_string(&output),
            Err(JsonError::SchemaValidationError { .. })
        ));
    }
    for result in [
        to_json_validated(&intermediate),
        to_json_pretty_validated(&intermediate),
    ] {
        let JsonError::SchemaValidationError { message } =
            result.expect_err("an array is not a CHAT document")
        else {
            panic!("schema mismatch must retain its category");
        };
        assert!(!message.is_empty());
    }
    let JsonError::SerializationError(cause) =
        validate_json_string("{\n").expect_err("incomplete JSON must be refused")
    else {
        panic!("malformed JSON is not a schema mismatch");
    };
    assert!(cause.is_eof());
    assert_eq!(cause.line(), 2);
}
