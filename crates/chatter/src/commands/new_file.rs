//! Create new minimal valid CHAT files.
//!
//! Generates a transcript containing exactly the required headers (`@UTF8`, `@Begin`,
//! `@End`, `@Languages`, `@Participants`, `@ID`) so the output is immediately parseable
//! by `chatter validate`. An optional utterance line can be included as a starting point.
//!
//! The file is built as a typed [`ChatFile`] and rendered with [`WriteChat`],
//! the same path every other CHAT-emitting surface takes.
//!
//! It used to delegate to a `MinimalChatFile` template in the parser-tests
//! crate, "ensuring the template stays in sync with the parser's own test
//! fixtures". That reason was not true: no fixture ever used it, this command
//! was its only consumer, and the coupling put a test crate in the release
//! binary. The template also hand-wrote `@UTF8`/`@Begin`/`@End` with format
//! strings, which design rule 15 forbids by name.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use talkbank_model::errors::ErrorCollector;
use talkbank_model::model::{TranscriptName, WriteChat};
use talkbank_transform::build_chat::{
    ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat,
};

/// Create a minimal valid CHAT file that conforms to the File Format and Header sections of the manual.
///
/// The generated transcript always contains the required `@UTF8`, `@Begin`, `@End`, `@Languages`, `@Participants`, and `@ID` headers.
/// These headers establish the file encoding, participant list, and utterance metadata that the manual describes before any tiers are parsed.
///
/// Generates a valid CHAT file with:
/// - Required `@UTF8` header
/// - Required `@Begin` and `@End` markers
/// - Required `@Languages`, `@Participants`, and `@ID` headers
/// - Optional utterance line
///
/// # Arguments
///
/// * `output` - Output file path (None = print to stdout)
/// * `speaker` - Speaker code (e.g., "CHI", "MOT")
/// * `language` - ISO 639-3 language code (e.g., "eng", "spa")
/// * `role` - Participant role (e.g., "Target_Child", "Mother")
/// * `corpus` - Corpus identifier (e.g., "corpus", "mydata")
/// * `utterance` - Optional utterance content
///
/// When an utterance is provided, it becomes the first utterance line (Main Tier) in the file so callers can start
/// from a ready-to-parse transcript. The generated structure follows the CHAT manual’s canonical ordering so downstream
/// validation/alignment tools see predictable headers.
pub fn create_new_file(
    output: Option<&Path>,
    speaker: &str,
    language: &str,
    role: &str,
    corpus: &str,
    utterance: Option<&str>,
) {
    let content = match build_validated(speaker, language, role, corpus, utterance, output) {
        Ok(text) => text,
        Err(why) => {
            eprintln!("Error: {why}");
            std::process::exit(1);
        }
    };

    // Write to output
    match output {
        Some(path) => {
            if let Err(e) = fs::write(path, &content) {
                eprintln!("Error writing file: {}", e);
                std::process::exit(1);
            }
            eprintln!("✓ Created {}", path.display());
        }
        None => {
            // Print to stdout
            if let Err(e) = io::stdout().write_all(content.as_bytes()) {
                eprintln!("Error writing to stdout: {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use talkbank_model::ParseErrors;
    use talkbank_model::model::{
        ChatFile, CorpusName, Header, LanguageCode, Line, ParticipantRole, SpeakerCode,
    };
    use talkbank_parser::ParseProduct;
    use talkbank_parser::TreeSitterParser;
    use tempfile::tempdir;
    use thiserror::Error;

    /// Error cases used by `new_file` integration-style tests.
    #[derive(Debug, Error)]
    enum TestError {
        #[error("Tempdir error")]
        TempDir { source: std::io::Error },
        #[error("IO error")]
        Io { source: std::io::Error },
        #[error("Parse error")]
        Parse { source: ParseErrors },
        #[error("Missing header: {header:?}")]
        MissingHeader { header: RequiredHeader },
        #[error("Missing participant entry")]
        MissingParticipant,
        #[error("Missing ID header")]
        MissingIdHeader,
        #[error("Missing utterance")]
        MissingUtterance,
    }

    /// Required headers that must appear in a minimal valid CHAT file.
    #[derive(Debug, Clone, Copy)]
    enum RequiredHeader {
        Utf8,
        Begin,
        End,
        Languages,
        Participants,
        Id,
    }

    /// Creates a minimal file and checks that all required headers are present.
    #[test]
    fn creates_valid_file_with_defaults() -> Result<(), TestError> {
        let dir = tempdir().map_err(|source| TestError::TempDir { source })?;
        let path = dir.path().join("test.cha");

        create_new_file(Some(&path), "CHI", "eng", "Target_Child", "corpus", None);

        let chat_file = parse_file(&path)?;
        require_header(&chat_file, RequiredHeader::Utf8)?;
        require_header(&chat_file, RequiredHeader::Begin)?;
        require_header(&chat_file, RequiredHeader::Languages)?;
        require_header(&chat_file, RequiredHeader::Participants)?;
        require_header(&chat_file, RequiredHeader::Id)?;
        require_header(&chat_file, RequiredHeader::End)?;

        require_language(
            &chat_file,
            LanguageCode::new("eng").expect("test literal is non-empty"),
        )?;
        require_participant(&chat_file, "CHI", "Target_Child")?;
        require_id_header(&chat_file, "eng", "corpus", "CHI", "Target_Child")?;
        Ok(())
    }

    /// Creates a file with caller-supplied metadata and verifies round-trip parse values.
    #[test]
    fn creates_file_with_custom_params() -> Result<(), TestError> {
        let dir = tempdir().map_err(|source| TestError::TempDir { source })?;
        let path = dir.path().join("test.cha");

        create_new_file(
            Some(&path),
            "MOT",
            "spa",
            "Mother",
            "mydata",
            Some("hola mundo ."),
        );

        let chat_file = parse_file(&path)?;
        require_language(
            &chat_file,
            LanguageCode::new("spa").expect("test literal is non-empty"),
        )?;
        require_participant(&chat_file, "MOT", "Mother")?;
        require_id_header(&chat_file, "spa", "mydata", "MOT", "Mother")?;
        require_utterance(&chat_file, "MOT", "hola mundo .")?;
        Ok(())
    }

    /// Parses file.
    ///
    /// `create_new_file` fixtures are expected to be clean, so this
    /// reproduces the pre-`ParseProduct` strict contract explicitly: a
    /// [`talkbank_parser::ParseProduct::Built`] with an error-severity
    /// diagnostic is treated as a failure the same as
    /// [`talkbank_parser::ParseProduct::Unbuildable`], rather than silently
    /// accepting a recovered-but-invalid model.
    fn parse_file(path: &Path) -> Result<ChatFile, TestError> {
        let content = fs::read_to_string(path).map_err(|source| TestError::Io { source })?;
        let parser = TreeSitterParser::new().expect("grammar loads");
        match parser.parse_chat_file(&content) {
            talkbank_parser::ParseProduct::Built { file, diagnostics } => {
                if diagnostics
                    .iter()
                    .any(|d| matches!(d.severity, talkbank_model::Severity::Error))
                {
                    return Err(TestError::Parse {
                        source: ParseErrors::from(diagnostics),
                    });
                }
                Ok(file)
            }
            talkbank_parser::ParseProduct::Unbuildable { diagnostics } => Err(TestError::Parse {
                source: ParseErrors::from(diagnostics),
            }),
        }
    }

    /// Assert that a required header appears in the parsed file.
    fn require_header(chat_file: &ChatFile, required: RequiredHeader) -> Result<(), TestError> {
        let found = chat_file.lines.iter().any(|line| {
            let Line::Header { header, .. } = line else {
                return false;
            };
            matches!(
                (required, header.as_ref()),
                (RequiredHeader::Utf8, Header::Utf8)
                    | (RequiredHeader::Begin, Header::Begin)
                    | (RequiredHeader::End, Header::End)
                    | (RequiredHeader::Languages, Header::Languages { .. })
                    | (RequiredHeader::Participants, Header::Participants { .. })
                    | (RequiredHeader::Id, Header::ID(_))
            )
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingHeader { header: required })
        }
    }

    /// Assert that the `` header includes the expected code.
    fn require_language(chat_file: &ChatFile, expected: LanguageCode) -> Result<(), TestError> {
        let found = chat_file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => {
                matches!(header.as_ref(), Header::Languages { codes } if codes.contains(&expected))
            }
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingHeader {
                header: RequiredHeader::Languages,
            })
        }
    }

    /// Assert that `` includes the expected speaker and role.
    fn require_participant(
        chat_file: &ChatFile,
        speaker: &str,
        role: &str,
    ) -> Result<(), TestError> {
        let expected_speaker = SpeakerCode::new(speaker);
        let expected_role = ParticipantRole::new(role);
        let found = chat_file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Participants { entries } => entries.iter().any(|entry| {
                    entry.speaker_code == expected_speaker && entry.role == expected_role
                }),
                _ => false,
            },
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingParticipant)
        }
    }

    /// Assert that an `` line matches the expected participant metadata.
    fn require_id_header(
        chat_file: &ChatFile,
        language: &str,
        corpus: &str,
        speaker: &str,
        role: &str,
    ) -> Result<(), TestError> {
        let expected_language =
            LanguageCode::new(language).expect("test call sites pass non-empty literals");
        let expected_corpus = CorpusName::new(corpus);
        let expected_speaker = SpeakerCode::new(speaker);
        let expected_role = ParticipantRole::new(role);

        let found = chat_file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::ID(id) => {
                    id.language.contains(&expected_language)
                        && id.corpus == expected_corpus
                        && id.speaker == expected_speaker
                        && id.role == expected_role
                }
                _ => false,
            },
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingIdHeader)
        }
    }

    /// Assert that the generated file contains the expected first utterance.
    fn require_utterance(
        chat_file: &ChatFile,
        speaker: &str,
        content: &str,
    ) -> Result<(), TestError> {
        let expected_speaker = SpeakerCode::new(speaker);
        let found = chat_file.lines.iter().any(|line| match line {
            Line::Utterance(utterance) => {
                utterance.main.speaker == expected_speaker
                    && utterance.main.content.to_content_string() == content
            }
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingUtterance)
        }
    }

    /// What the command promises: a file that `chatter validate` can read.
    ///
    /// The five tests this replaces lived in the parser-tests crate and
    /// asserted the same thing about a string template. Most of what they
    /// checked is now carried by the types: `build` returns a `ChatFile`, so
    /// "is it a CHAT file" is not a question a test can ask any more. What
    /// survives is the part no type covers, which is that the RENDERED TEXT
    /// parses back.
    #[test]
    fn the_generated_file_parses() {
        let text = build_validated("CHI", "eng", "Target_Child", "corpus", None, None)
            .expect("a default file must build and validate");
        let parser = TreeSitterParser::new().expect("parser");
        let parsed = parser.parse_chat_file(&text);
        assert!(
            matches!(parsed, ParseProduct::Built { .. }),
            "generated file must parse:\n{text}"
        );
    }

    /// The optional utterance goes through the fragment parser, so it arrives
    /// as a typed main tier rather than an interpolated line.
    #[test]
    fn the_generated_file_with_an_utterance_parses_and_keeps_it() {
        let text = build_validated("MOT", "spa", "Mother", "brown", Some("hola mundo ."), None)
            .expect("a file with an utterance must build and validate");
        assert!(text.contains("*MOT:\thola mundo ."), "got:\n{text}");
        let parser = TreeSitterParser::new().expect("parser");
        let parsed = parser.parse_chat_file(&text);
        assert!(
            matches!(parsed, ParseProduct::Built { .. }),
            "generated file must parse:\n{text}"
        );
    }

    /// An EMPTY language is refused at construction, naming the flag.
    ///
    /// Pins the actual boundary, not the one I assumed. A malformed but
    /// non-empty code like `not-a-language` constructs fine and is reported by
    /// `chatter validate` instead; that split is the model's, deliberately, so
    /// a parser can build a value it then diagnoses. The first version of this
    /// test asserted `not-a-language` was refused and failed at once.
    #[test]
    fn an_empty_language_is_refused_by_name() {
        let why = build_validated("CHI", "", "Target_Child", "corpus", None, None)
            .expect_err("an empty language code must be refused");
        assert!(why.contains("--language"), "{why}");
    }
}

/// Describe the file this command writes, for the shared CHAT builder.
///
/// # Why a description and not lines
///
/// `talkbank_transform::build_chat` is the project's general CHAT-generation
/// entry point, used by the MICASE converter and documented as the path "for
/// any converter". This command assembled its own `Vec<Line>` instead, which
/// made it a second owner of what a minimal file's header block IS: a header
/// added to `build_header_lines` would not have reached `new-file`, and
/// nothing would have caught the drift.
///
/// The struct literal is deliberate over a builder: a new
/// `TranscriptDescription` field breaks this line rather than silently
/// defaulting, which is the failure worth having.
fn describe(
    speaker: &str,
    language: &str,
    role: &str,
    corpus: &str,
    utterance: Option<&str>,
) -> TranscriptDescription {
    TranscriptDescription {
        langs: vec![language.to_string()],
        participants: vec![ParticipantDesc::new(speaker, role, corpus)],
        media_name: None,
        media_type: None,
        pid: None,
        media_status: None,
        date: None,
        situation: None,
        options: None,
        transcriber: None,
        comments: Vec::new(),
        utterances: utterance
            .map(|text| UtteranceDesc {
                speaker: speaker.to_string(),
                text: text.to_string(),
                start_ms: None,
                end_ms: None,
                lang: None,
            })
            .into_iter()
            .collect(),
    }
}

/// Build the file, RUN VALIDATION on it, and refuse to write one that fails.
///
/// # The command's promise is a state, so it goes through the state transition
///
/// `new-file`'s contract is "a minimal VALID CHAT file". `build` returns a
/// `ChatFile<NotValidated>`, so until this existed that contract lived in a doc
/// comment and a test, and a `--speaker` the validator rejects would have been
/// written to disk under a command that calls its output valid. The model
/// already has the phase transition, `validate_into`, and this runs it.
///
/// Note what the marker does and does not prove. `validate_into` streams
/// diagnostics into the sink and changes state UNCONDITIONALLY, so
/// `ChatFile<Validated>` means validation RAN, not that it passed. The sink is
/// therefore checked here rather than trusted to the type; a `Validated` that
/// any input can reach is a label rather than a proof, and treating it as the
/// latter is how this would go wrong.
fn build_validated(
    speaker: &str,
    language: &str,
    role: &str,
    corpus: &str,
    utterance: Option<&str>,
    output: Option<&Path>,
) -> Result<String, String> {
    // The FLAGS are this command's input, so their names are this command's to
    // report. `build_chat` refuses an empty language too, but it can only name
    // `@Languages`, because a builder does not know it was fed by a CLI.
    if language.trim().is_empty() {
        return Err("--language cannot be empty".to_string());
    }
    if corpus.trim().is_empty() {
        return Err("--corpus cannot be empty".to_string());
    }
    if speaker.trim().is_empty() {
        return Err("--speaker cannot be empty".to_string());
    }

    let desc = describe(speaker, language, role, corpus, utterance);
    let file = build_chat(&desc).map_err(|e| e.to_string())?;

    // Rules about a transcript's own file name (E531) compare against the stem
    // the file will be WRITTEN as, so the destination is what to validate under.
    let errors = ErrorCollector::new();

    // Rules about a transcript's own file name (E531) compare against the stem
    // it will be WRITTEN as, so the destination is what to validate under.
    // `Anonymous` for stdout is a DECISION: with no file there is no file name,
    // so those rules correctly do not run.
    let name = match output {
        Some(path) => TranscriptName::for_path(path),
        None => TranscriptName::Anonymous,
    };

    let validated = file.validate_into(&errors, name);

    if errors.has_errors() {
        return Err(format!(
            "the requested file does not validate, so it was not written. \
             `chatter validate` would reject it:\n{}",
            errors
                .to_vec()
                .iter()
                .map(|e| format!("  {} {}", e.code.as_str(), e.message))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    Ok(validated.to_chat_string())
}
