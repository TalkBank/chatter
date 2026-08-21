#!/usr/bin/env node

// Generates the CA element and delimiter types from the symbol registry.
//
// Before this existed, the character-to-meaning mapping for these 25 symbols
// was written by hand in the two enums, in their two `to_symbol` impls, in the
// tree-sitter parser's two `*_from_char` dispatch tables, and in two tests
// whose only job was to notice when those copies disagreed. Everything a
// program can ask about a symbol now comes from one record.
//
// `notation_family` is generated as an accessor ON PURPOSE. Provenance used to
// be readable only from the name of the array a symbol sat in, which is how two
// disfluency marks came to be filed as Conversation Analysis notation. A caller
// that needs to know "is this CA" must now ask the symbol, and will get an
// answer that excludes blocking and segment repetition, which is correct.

const path = require('node:path');
const {
  PARSE_ROLES,
  NOTATION_FAMILIES,
  failWith,
  loadRegistry,
  lookupOrFail,
  rustCharPattern,
  rustLiteral,
  rustVariant,
  rustfmt,
  writeGenerated,
} = require('./registry.js');

const fail = failWith('CA type generation failed');

// Prose only. The Rust VARIANT name is derived by `rustVariant`, exactly as a
// symbol's variant is; an earlier cut wrote both out here, which is the same
// stored-mirror the registry deleted for symbol ids, one level up.
const FAMILY_DOCS = {
  conversation_analysis: 'Conversation Analysis notation.',
  disfluency:
    'A disfluency mark from the CHAT manual Disfluency Transcription chapter.\n    /// CLAN classifies these explicitly as NOT CA.',
};

// Rust naming and Rust prose, which have no business in a language-neutral
// registry that also feeds grammar.js and the book. Keyed by parse role. The
// loop below iterates PARSE_ROLES, the closed vocabulary registry.js owns and
// validates every symbol against; it is NOT derived from the registry FILE.
const ROLE_TYPES = {
  word_attached: {
    typeName: 'CAElementType',
    title: 'A marker that attaches to a word token.',
    detail:
      'Named for its PARSE ROLE and not its provenance: the type carries any\n'
      + 'symbol that joins the word token, whatever notation it comes from. Ask\n'
      + '[`CAElementType::notation_family`] for provenance; never infer it from\n'
      + 'the name of this type or of the array it is generated into.',
  },
  paired_stretch: {
    typeName: 'CADelimiterType',
    title: 'A PAIRED marker that opens and closes a stretch of a word.',
    detail:
      'Each of these must appear an even number of times in a word; an\n'
      + 'unmatched one is rejected as an unbalanced delimiter.',
  },
};

function renderType(role, symbols) {
  const spec = lookupOrFail(ROLE_TYPES, role, 'ROLE_TYPES', fail);
  const { typeName, title, detail } = spec;
  const members = symbols.filter((s) => s.parse_role === role);
  if (members.length === 0) fail(`no symbols with parse_role ${role}`);

  // One arm shape, five right-hand sides. Indentation is left to rustfmt, which
  // reindents every arm anyway.
  const arms = (rhs) => members.map((s) => `${typeName}::${s.rustVariant} => ${rhs(s)},`).join('\n');

  const variants = members
    .map((s) => {
      const note =
        s.notation_family === 'disfluency' ? ' (a disfluency mark, not CA notation)' : '';
      return `/// \`${s.char}\` ${s.codepoint}: ${s.gloss}${note}\n${s.rustVariant},`;
    })
    .join('\n');

  const detailDoc = detail
    .split('\n')
    .map((line) => `/// ${line}`)
    .join('\n');

  return `/// ${title}
///
${detailDoc}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, SemanticEq,
    SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum ${typeName} {
${variants}
}

impl ${typeName} {
    /// Every variant, in registry order.
    pub const ALL: &'static [${typeName}] = &[${members.map((s) => `${typeName}::${s.rustVariant}`).join(', ')}];

    /// The CHAT symbol this variant is written as.
    pub fn to_symbol(&self) -> &'static str {
        match self {
${arms((s) => rustLiteral(s.char))}
        }
    }

    /// The variant a character denotes, or \`None\` when it denotes none.
    ///
    /// Total, so a caller cannot reach a panicking "this was already checked"
    /// branch by forgetting a guard.
    pub fn from_char(ch: char) -> Option<${typeName}> {
        match ch {
${members.map((s) => `${rustCharPattern(s.char)} => Some(${typeName}::${s.rustVariant}),`).join('\n')}
            _ => None,
        }
    }

    /// Which notation this symbol belongs to. NOT implied by this type's name.
    pub fn notation_family(&self) -> NotationFamily {
        match self {
${arms((s) => `NotationFamily::${rustVariant(s.notation_family)}`)}
        }
    }

    /// A one-line human meaning, the same text the book tables carry.
    pub fn gloss(&self) -> &'static str {
        match self {
${arms((s) => rustLiteral(s.gloss))}
        }
    }

    /// A CHAT main tier that uses this symbol and must parse.
    pub fn example(&self) -> &'static str {
        match self {
${arms((s) => rustLiteral(s.example))}
        }
    }
}
`;
}

function render(symbols) {
  const families = NOTATION_FAMILIES.map((id) => {
    const doc = lookupOrFail(FAMILY_DOCS, id, 'FAMILY_DOCS', fail);
    return `    /// ${doc}\n    ${rustVariant(id)},`;
  }).join('\n');

  return rustfmt(`// @generated by spec/symbols/generate_ca_types.js from spec/symbols/symbol_registry.json
// DO NOT EDIT MANUALLY. Run \`just symbols-gen\`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Where a CHAT symbol comes from, as opposed to how it parses.
///
/// These are independent facts and conflating them is what put two disfluency
/// marks into categories named for Conversation Analysis. Parse role decides
/// which type a symbol belongs to; this decides nothing about parsing at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotationFamily {
${families}
}

${PARSE_ROLES.map((role) => renderType(role, symbols)).join('\n')}`);
}

function main() {
  const { repoRoot, symbols } = loadRegistry();
  writeGenerated(
    repoRoot,
    path.join(repoRoot, 'crates', 'talkbank-model', 'src', 'generated', 'ca_symbols.rs'),
    render(symbols),
  );
}

main();
