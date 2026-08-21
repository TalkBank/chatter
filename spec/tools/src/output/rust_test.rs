//! # Rust Test Generator
//!
//! Generates Rust test files from specifications

use crate::spec::construct::{ConstructExample, ConstructSpec};

/// Runs wrap for chat file parse.
fn wrap_for_chat_file_parse(example: &ConstructExample, level: &str) -> String {
    let input_type = example.input_type.trim();
    let chat_prelude = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||";
    let has_chat_boundaries = example.input.contains("@Begin") && example.input.contains("@End");

    match input_type {
        // Complete documents should be parsed as-is; fragments in chat blocks are wrapped.
        "chat" | "chat-file" | "document" => {
            if has_chat_boundaries {
                example.input.clone()
            } else {
                format!("{chat_prelude}\n{}\n@End", example.input)
            }
        }
        // Header fragments must be wrapped in minimal file structure.
        "languages_header" | "participants_header" => {
            if input_type == "participants_header" {
                let speaker = extract_participant_speaker(&example.input).unwrap_or("CHI");
                format!(
                    "@UTF8\n@Begin\n@Languages:\teng\n{}\n@ID:\teng|corpus|{}|||||Target_Child|||\n@End",
                    example.input, speaker
                )
            } else {
                format!("@UTF8\n@Begin\n{}\n@End", example.input)
            }
        }
        // Main tier / utterance fragments: input is already a complete tier line.
        "main_tier" | "utterance" => {
            format!("{chat_prelude}\n{}\n@End", example.input)
        }
        // Dependent tier fragments require a main tier anchor.
        "com_dependent_tier" | "gra_dependent_tier" | "mor_dependent_tier"
        | "pho_dependent_tier" => {
            format!("{chat_prelude}\n*CHI:\tword .\n{}\n@End", example.input)
        }
        // Default handling by construct level (directory name).
        _ => match level {
            "word" => format!("{chat_prelude}\n*CHI:\t{} .\n@End", example.input),
            "main_tier" | "utterance" => format!("{chat_prelude}\n{}\n@End", example.input),
            "header" => format!("@UTF8\n@Begin\n{}\n@End", example.input),
            "tiers" => format!("{chat_prelude}\n*CHI:\tword .\n{}\n@End", example.input),
            _ => format!("{chat_prelude}\n{}\n@End", example.input),
        },
    }
}

/// Extracts participant speaker.
fn extract_participant_speaker(input: &str) -> Option<&str> {
    let line = input
        .lines()
        .find(|l| l.trim_start().starts_with("@Participants:"))?;
    let (_, rest) = line.split_once(':')?;
    rest.split_whitespace().next()
}

/// Generate Rust test for a construct example
pub fn generate_construct_test(
    example: &ConstructExample,
    level: &str,
    test_error_path: &str,
) -> String {
    let wrapped = wrap_for_chat_file_parse(example, level);
    format!(
        r#"#[test]
/// Tests expected behavior.
fn test_{name}() -> Result<(), {test_error_path}> {{
    let parser = TreeSitterParser::new()?;
    // `strict_parse` reproduces the pre-`ParseProduct` fail-on-any-diagnostic
    // contract: a construct example is expected to parse completely cleanly.
    let _parsed = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file({wrapped_input:?}))?;

    Ok(())
}}

"#,
        name = example.test_name(),
        wrapped_input = wrapped,
    )
}

/// Generate just the test bodies (no imports) for construct specs
pub fn generate_construct_test_body(specs: &[ConstructSpec], test_error_path: &str) -> String {
    let mut output = String::new();

    output.push_str("// Generated from spec/ by `just spec-gen` - test bodies only\n");
    output.push_str(
        "// DO NOT EDIT MANUALLY - run `just spec-gen`; `just spec-check` says whether this is current\n\n",
    );

    for spec in specs {
        for example in &spec.examples {
            output.push_str(&generate_construct_test(
                example,
                &spec.metadata.level,
                test_error_path,
            ));
        }
    }

    output
}

/// A test file this generator owns.
///
/// Writing and pre-write cleaning both derive from this enum, so the set of
/// files produced and the set of files swept cannot drift apart. They used to
/// be two hand-maintained lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedTestFile {
    /// Construct test bodies, `include!`d by the parser test tree.
    ///
    /// The only variant since R4 deleted `ErrorBodies`: the string-based
    /// error tests asserted "declared codes appear among PARSE diagnostics",
    /// which is strictly weaker than what replaced it in both directions.
    /// The fixture corpus runs every example against a real file and checks
    /// both stages (five examples' codes are SPLIT across stages, which the
    /// per-stage form could never assert), and the observation snapshot
    /// byte-pins the exact per-stage sets. The E531-needs-a-filename hazard
    /// this generator's own comment used to excuse is gone with them.
    ConstructBodies,
}

impl GeneratedTestFile {
    /// Every file this generator writes.
    pub const ALL: &'static [Self] = &[Self::ConstructBodies];

    /// The file name written into the output directory.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::ConstructBodies => "generated_construct_tests_body.rs",
        }
    }

    /// Render this file's contents.
    pub fn render(self, construct_specs: &[ConstructSpec], test_error_path: &str) -> String {
        match self {
            Self::ConstructBodies => generate_construct_test_body(construct_specs, test_error_path),
        }
    }
}

/// Names this generator no longer writes but still sweeps.
///
/// A checkout that predates a change to [`GeneratedTestFile::ALL`] still has
/// the old files on disk. They have no renderer, which is the point: there is
/// nothing to keep in step, only something to remove.
///
/// These two are the standalone twins of the `_body` files, identical to them
/// apart from a `use` preamble. Only the bodies were ever `include!`d, so the
/// pair held 213 `#[test]` functions and 175 KB of tracked source that nothing
/// compiled, while inflating every count made of the suite.
pub const RETIRED_OUTPUT_NAMES: &[&str] = &[
    "generated_construct_tests.rs",
    "generated_error_tests.rs",
    // R4: the string-based error tests are deleted; see `GeneratedTestFile`.
    "generated_error_tests_body.rs",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::construct::*;

    /// Tests generate construct test.
    #[test]
    fn test_generate_construct_test() {
        let example = ConstructExample {
            name: "simple_word".to_string(),
            input: "hello".to_string(),
            description: "Plain word".to_string(),
            expected: ExpectedParseTree {
                cst: "(word\n  (segment))".to_string(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "standalone_word".to_string(),
        };

        let output = generate_construct_test(
            &example,
            "word",
            "talkbank_parser_tests::test_error::TestError",
        );
        assert!(output.contains("fn test_simple_word"));
        assert!(output.contains("parse_chat_file"));
        assert!(output.contains("Result"));
    }

    /// Tests wrap chat fragment in file context.
    #[test]
    fn test_wrap_chat_fragment_in_file_context() {
        let example = ConstructExample {
            name: "overlap_points".to_string(),
            input: "*CHI:\t⌈0 &=laughter⌉ .".to_string(),
            description: "chat fragment".to_string(),
            expected: ExpectedParseTree {
                cst: String::new(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "chat".to_string(),
        };

        let wrapped = wrap_for_chat_file_parse(&example, "main_tier");
        assert!(wrapped.contains("@Begin"));
        assert!(wrapped.contains("@ID:\teng|corpus|CHI"));
        assert!(wrapped.contains("*CHI:\t⌈0 &=laughter⌉ ."));
        assert!(wrapped.contains("@End"));
    }

    /// Tests wrap participants header with matching id.
    #[test]
    fn test_wrap_participants_header_with_matching_id() {
        let example = ConstructExample {
            name: "participants_single".to_string(),
            input: "@Participants:\tMOT Mother".to_string(),
            description: "header fragment".to_string(),
            expected: ExpectedParseTree {
                cst: String::new(),
                wrapped_input: None,
                full_cst: None,
            },
            input_type: "participants_header".to_string(),
        };

        let wrapped = wrap_for_chat_file_parse(&example, "header");
        assert!(wrapped.contains("@Participants:\tMOT Mother"));
        assert!(wrapped.contains("@ID:\teng|corpus|MOT"));
    }
}
