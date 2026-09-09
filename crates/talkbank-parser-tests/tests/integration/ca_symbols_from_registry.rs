// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Every Conversation Analysis symbol in the registry reaches the word
//! content the generated table names, through the parser.
//!
//! `spec/symbols/symbol_registry.json` is the one owner of the closed
//! vocabulary; `CAElementType::from_char` and `CADelimiterType::from_char`
//! are generated from it. The word converter carried its own hand-written
//! copies of both tables until 2026-09-09, agreeing with the generated ones
//! by luck: the module holding the generated dispatch was not even
//! declared, so nothing called it and nothing compared the two. The
//! converter dispatches through the generated tables now, and this test is
//! what notices if the registry, the grammar and the converter ever stop
//! agreeing: each symbol's own registry example is parsed, and the word it
//! marks must carry the variant `from_char` names for it.
//!
//! SURVIVES a type, as a MEASUREMENT: of the parser against the registry it
//! is generated from. The tables cannot drift any more; the grammar's
//! token sets still can.

use std::path::Path;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::content::word::WordContent;
use talkbank_model::model::{CADelimiterType, CAElementType};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// One registry record, the fields this test reads.
#[derive(serde::Deserialize)]
struct Symbol {
    id: String,
    codepoint: String,
    parse_role: String,
    example: String,
}

#[derive(serde::Deserialize)]
struct Registry {
    symbols: Vec<Symbol>,
}

/// The registry's symbols, read from the spec tree beside this crate.
fn registry() -> Result<Vec<Symbol>, TestError> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/symbols/symbol_registry.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|err| TestError::Failure(format!("reading {}: {err}", path.display())))?;
    let registry: Registry = serde_json::from_str(&text)
        .map_err(|err| TestError::Failure(format!("decoding {}: {err}", path.display())))?;
    Ok(registry.symbols)
}

/// The character a registry codepoint (`U+2051`) names.
fn char_of(codepoint: &str) -> Result<char, TestError> {
    let hex = codepoint
        .strip_prefix("U+")
        .ok_or_else(|| TestError::Failure(format!("codepoint {codepoint:?} is not `U+hex`")))?;
    let value = u32::from_str_radix(hex, 16)
        .map_err(|err| TestError::Failure(format!("codepoint {codepoint:?}: {err}")))?;
    char::from_u32(value)
        .ok_or_else(|| TestError::Failure(format!("codepoint {codepoint:?} is no char")))
}

/// What the parser put in the words of a main tier for one symbol: the CA
/// element types and delimiter types, in order.
fn ca_pieces(
    line: &str,
    parser: &TreeSitterParser,
) -> Result<(Vec<CAElementType>, Vec<CADelimiterType>), TestError> {
    let tier = parser
        .parse_main_tier(line)
        .map_err(|err| TestError::Failure(format!("{line:?} did not parse: {err:?}")))?;
    let mut elements = Vec::new();
    let mut delimiters = Vec::new();
    walk_words(&tier.content.content, None, &mut |leaf| {
        if let WordItem::Word(word) = leaf {
            for piece in word.content().iter() {
                match piece {
                    WordContent::CAElement(element) => elements.push(element.element_type),
                    WordContent::CADelimiter(delimiter) => {
                        delimiters.push(delimiter.delimiter_type)
                    }
                    _ => {}
                }
            }
        }
    });
    Ok((elements, delimiters))
}

/// A `word_attached` symbol's example carries exactly its element and no
/// delimiter; a `paired_stretch` symbol's example carries its delimiter
/// twice, opening and closing the stretch, and no element. Any other role
/// is not this test's.
#[test]
fn every_registry_symbol_reaches_the_generated_variant_through_the_parser() -> Result<(), TestError>
{
    let parser = TreeSitterParser::new()?;
    let mut wrong = Vec::new();
    let mut checked = 0;
    for symbol in registry()? {
        let ch = char_of(&symbol.codepoint)?;
        match symbol.parse_role.as_str() {
            "word_attached" => {
                checked += 1;
                let Some(expected) = CAElementType::from_char(ch) else {
                    wrong.push(format!(
                        "{}: {ch:?} is in the registry and not in CAElementType::from_char",
                        symbol.id
                    ));
                    continue;
                };
                match ca_pieces(&symbol.example, &parser) {
                    Ok((elements, delimiters)) if elements == [expected] && delimiters.is_empty() => {}
                    Ok((elements, delimiters)) => wrong.push(format!(
                        "{}: expected element [{expected:?}] from {:?}, got elements {elements:?} and delimiters {delimiters:?}",
                        symbol.id, symbol.example
                    )),
                    Err(err) => wrong.push(format!("{}: {err:?}", symbol.id)),
                }
            }
            "paired_stretch" => {
                checked += 1;
                let Some(expected) = CADelimiterType::from_char(ch) else {
                    wrong.push(format!(
                        "{}: {ch:?} is in the registry and not in CADelimiterType::from_char",
                        symbol.id
                    ));
                    continue;
                };
                match ca_pieces(&symbol.example, &parser) {
                    Ok((elements, delimiters))
                        if delimiters == [expected, expected] && elements.is_empty() => {}
                    Ok((elements, delimiters)) => wrong.push(format!(
                        "{}: expected delimiters [{expected:?}, {expected:?}] from {:?}, got delimiters {delimiters:?} and elements {elements:?}",
                        symbol.id, symbol.example
                    )),
                    Err(err) => wrong.push(format!("{}: {err:?}", symbol.id)),
                }
            }
            _ => {}
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(
        checked >= 25,
        "only {checked} CA symbols in the registry; the table this pins has 25 arms"
    );
    Ok(())
}
