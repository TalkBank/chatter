//! Fallible JSON boundaries shared by the ignored diagnostic reports.
//! These projections are investigation aids, not semantic-equivalence proofs.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Two successfully serialized, metadata-filtered model projections.
pub struct JsonPair {
    left: Value,
    right: Value,
}

/// A witnessed JSON difference, never a traversal-limit placeholder.
pub struct JsonDifference {
    /// Object/array path of the differing values.
    pub path: String,
    /// Display form of the left value or its explicit absence.
    pub left: String,
    /// Display form of the right value or its explicit absence.
    pub right: String,
}

/// The comparison stopped before it could establish equality or difference.
#[derive(Debug)]
pub struct ComparisonLimit {
    path: String,
    maximum: usize,
}

impl std::fmt::Display for ComparisonLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JSON comparison exceeded depth {} at {}",
            self.maximum, self.path
        )
    }
}

impl std::error::Error for ComparisonLimit {}

impl JsonPair {
    /// Both serializations must succeed before a comparison can begin.
    pub fn serialize<T: Serialize>(left: &T, right: &T) -> serde_json::Result<Self> {
        let mut left = serde_json::to_value(left)?;
        let mut right = serde_json::to_value(right)?;
        strip_spans(&mut left);
        strip_spans(&mut right);
        Ok(Self { left, right })
    }

    /// Search in field/index order, refusing to infer a result past the limit.
    pub fn first_difference(
        &self,
        maximum: usize,
    ) -> Result<Option<JsonDifference>, ComparisonLimit> {
        compare(&self.left, &self.right, String::new(), 0, maximum)
    }
}

fn strip_spans(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("span");
            map.remove("content_span");
            for value in map.values_mut() {
                strip_spans(value);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(strip_spans),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn compare(
    left: &Value,
    right: &Value,
    path: String,
    depth: usize,
    maximum: usize,
) -> Result<Option<JsonDifference>, ComparisonLimit> {
    if depth > maximum {
        return Err(ComparisonLimit { path, maximum });
    }
    let difference = |path, left, right| Some(JsonDifference { path, left, right });
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, left) in a {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                let Some(right) = b.get(key) else {
                    return Ok(difference(child, left.to_string(), "<missing>".into()));
                };
                if let Some(found) = compare(left, right, child, depth + 1, maximum)? {
                    return Ok(Some(found));
                }
            }
            for (key, right) in b {
                if !a.contains_key(key) {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    return Ok(difference(child, "<missing>".into(), right.to_string()));
                }
            }
            Ok(None)
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Ok(difference(
                    format!("{path}[]"),
                    format!("len={}", a.len()),
                    format!("len={}", b.len()),
                ));
            }
            for (index, (left, right)) in a.iter().zip(b).enumerate() {
                if let Some(found) =
                    compare(left, right, format!("{path}[{index}]"), depth + 1, maximum)?
                {
                    return Ok(Some(found));
                }
            }
            Ok(None)
        }
        (a, b) if a == b => Ok(None),
        (a, b) => Ok(difference(path, a.to_string(), b.to_string())),
    }
}

/// Serialized bytes exist only after the report serializer succeeds.
pub struct JsonReport(Vec<u8>);

impl JsonReport {
    /// Prepare report bytes without creating or changing an output file.
    pub fn serialize(report: &impl Serialize) -> serde_json::Result<Self> {
        serde_json::to_vec_pretty(report).map(Self)
    }

    /// Consume prepared bytes and report whether the destination write succeeded.
    pub fn write(self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.0)
    }
}

/// Discover a nonempty sorted file population, propagating traversal failures.
pub fn collect_cha_files(base: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(base) {
        let entry = entry?;
        // Keep the report's extension-based population, including symlinked
        // files. A subsequent read must report an unusable selected entry.
        if entry.path().extension().is_some_and(|ext| ext == "cha") {
            files.push(entry.into_path());
        }
    }
    if files.is_empty() {
        return Err(
            std::io::Error::new(std::io::ErrorKind::NotFound, "no CHAT files discovered").into(),
        );
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RefusedSerialization;
    impl Serialize for RefusedSerialization {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("deliberate serializer refusal"))
        }
    }

    #[test]
    fn serializer_refusal_cannot_create_a_comparison_or_report() {
        assert!(JsonPair::serialize(&RefusedSerialization, &RefusedSerialization).is_err());
        assert!(JsonReport::serialize(&RefusedSerialization).is_err());
    }

    #[test]
    fn comparison_limit_is_neither_equality_nor_a_difference()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut value = Value::Null;
        for _ in 0..27 {
            value = serde_json::json!({"child": value});
        }
        let pair = JsonPair::serialize(&value, &value)?;
        for maximum in [20, 25] {
            assert!(pair.first_difference(maximum).is_err());
        }
        assert!(pair.first_difference(27)?.is_none());
        Ok(())
    }

    #[test]
    fn comparison_keeps_real_paths_and_ignores_only_span_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let left = serde_json::json!({"items": [{"value": 1, "span": [0, 1]}]});
        let same = serde_json::json!({"items": [{"value": 1, "span": [8, 9]}]});
        let right = serde_json::json!({"items": [{"value": 2}]});
        assert!(
            JsonPair::serialize(&left, &same)?
                .first_difference(20)?
                .is_none()
        );
        let Some(found) = JsonPair::serialize(&left, &right)?.first_difference(20)? else {
            return Err("missing value difference".into());
        };
        assert_eq!(found.path, "items[0].value");
        assert_eq!((found.left.as_str(), found.right.as_str()), ("1", "2"));
        Ok(())
    }

    #[test]
    fn missing_inputs_and_failed_output_are_errors() -> Result<(), Box<dyn std::error::Error>> {
        // An existing regular file cannot contain a child directory. No output
        // is created and no external corpus or temporary directory is needed.
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(collect_cha_files(&manifest).is_err());
        let impossible = manifest.join("unavailable.cha");
        assert!(collect_cha_files(&impossible).is_err());
        assert!(
            JsonReport::serialize(&Vec::<u8>::new())?
                .write(&impossible)
                .is_err()
        );
        Ok(())
    }
}
