#!/usr/bin/env node
/**
 * Generate Rust constants for tree-sitter node types
 *
 * Reads node-types.json from the tree-sitter grammar and generates a Rust module
 * with string constants for each node type, including doc comments from
 * node-type-docs.json.
 *
 * Usage: node scripts/generate-node-types.js [grammar-dir]
 *
 * If grammar-dir is not provided, defaults to grammar/ (relative to repo root).
 * Output goes to stdout; redirect to the desired file.
 */

import { grammarDir, namedKinds } from './node-kinds.js';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

const treeSitterDir = grammarDir(repoRoot, process.argv);
const nodeTypesPath = path.join(treeSitterDir, 'src', 'node-types.json');
const nodeTypes = JSON.parse(fs.readFileSync(nodeTypesPath, 'utf8'));

// Load doc descriptions
const docsPath = path.join(__dirname, 'node-type-docs.json');
const docs = JSON.parse(fs.readFileSync(docsPath, 'utf8'));

// Extract all unique named node type names
// Skip anonymous nodes (punctuation, keywords) - these are already in the grammar
const types = namedKinds(nodeTypes);

// Convert to sorted array for consistent output
const sortedTypes = Array.from(types).sort();

// Assign each type to a category for section grouping
function categorize(type) {
  // Document structure
  if (['document', 'line', 'utterance', 'utterance_end', 'header', 'header_gap',
       'header_sep', 'pre_begin_header', 'continuation'].includes(type)) {
    return '01_document_structure';
  }
  // Begin/End/UTF8
  if (['begin_header', 'end_header', 'utf8_header'].includes(type)) {
    return '02_required_headers';
  }
  // Participant headers
  if (type.startsWith('participant') || type === 'id_header' || type === 'id_prefix'
      || type === 'id_contents' || type.startsWith('id_')) {
    return '03_participant_headers';
  }
  // @Languages / @Options / @Media / other structured headers
  if (['languages_header', 'languages_prefix', 'languages_contents',
       'language_code', 'options_header', 'options_prefix', 'options_contents',
       'option_name', 'media_header', 'media_prefix', 'media_contents',
       'media_filename', 'media_type', 'media_status', 'media_url',
       'audio_value', 'video_value', 'notrans_value', 'unlinked_value',
       'missing_value', 'male_value', 'female_value',
       'date_header', 'date_prefix', 'date_contents',
       'time_duration_header', 'time_duration_prefix', 'time_duration_contents',
       'time_start_header', 'time_start_prefix',
       'transcription_header', 'transcription_prefix', 'transcription_option',
       'number_header', 'number_prefix', 'number_option',
       'recording_quality_header', 'recording_quality_prefix', 'recording_quality_option',
       'types_header', 'types_prefix', 'types_activity', 'types_design', 'types_group',
       'age_format',
      ].includes(type)) {
    return '04_structured_headers';
  }
  // Simple text headers (prefix + header pairs)
  if ((type.endsWith('_header') || type.endsWith('_prefix'))
      && !type.startsWith('id_') && !type.startsWith('participant')
      && !type.startsWith('mor_') && !type.startsWith('gra_')
      && !type.startsWith('pho_') && !type.startsWith('sin_')
      && !type.startsWith('wor_')) {
    return '05_text_headers';
  }
  // Main tier
  if (['main_tier', 'speaker', 'tier_body', 'tier_sep'].includes(type)) {
    return '06_main_tier';
  }
  // Terminators
  if (['terminator', 'period', 'question', 'exclamation', 'broken_question',
       'interrupted_question', 'interruption', 'self_interrupted_question',
       'self_interruption', 'trailing_off', 'trailing_off_question',
       'unmarked_ending'].includes(type)) {
    return '07_terminators';
  }
  // Linkers
  if (type.startsWith('linker')) {
    return '08_linkers';
  }
  // Content items and words
  if (['content_item', 'base_content_item', 'contents', 'standalone_word',
       'word_with_optional_annotations', 'nonword', 'nonword_with_optional_annotations',
       'text_segment', 'rest_of_line', 'anything'].includes(type)) {
    return '09_content_items';
  }
  // Events
  if (['event', 'event_marker', 'event_segment', 'other_spoken_event',
       'nonvocal', 'nonvocal_begin', 'nonvocal_begin_marker',
       'nonvocal_end', 'nonvocal_end_marker', 'nonvocal_simple'].includes(type)) {
    return '10_events';
  }
  // Groups and annotations
  if (['group_with_annotations', 'quotation', 'main_pho_group', 'main_sin_group',
       'base_annotation', 'base_annotations', 'final_codes'].includes(type)) {
    return '11_groups';
  }
  // Annotations (brackets)
  if (['alt_annotation', 'explanation_annotation', 'error_marker_annotation',
       'replacement', 'para_annotation', 'duration_annotation',
       'percent_annotation', 'freecode', 'postcode',
       'retrace_complete', 'retrace_partial', 'retrace_multiple',
       'retrace_reformulation', 'retrace_uncertain',
       'scoped_best_guess', 'scoped_uncertain',
       'scoped_stressing', 'scoped_contrastive_stressing',
       'exclude_marker', 'tag_marker', 'vocative_marker',
       'long_feature', 'long_feature_begin', 'long_feature_begin_marker',
       'long_feature_end', 'long_feature_end_marker', 'long_feature_label',
       'underline_begin', 'underline_end'].includes(type)) {
    return '12_annotations';
  }
  // Overlap
  if (['overlap_point', 'indexed_overlap_follows', 'indexed_overlap_precedes'].includes(type)) {
    return '13_overlap';
  }
  // Pauses and intonation
  if (['pause_token', 'falling_to_low', 'falling_to_mid',
       'rising_to_high', 'rising_to_mid', 'level_pitch'].includes(type)) {
    return '14_prosody';
  }
  // CA markers
  if (type.startsWith('ca_')) {
    return '15_conversation_analysis';
  }
  // Dependent tiers (generic)
  if (type === 'dependent_tier' || type.endsWith('_dependent_tier')
      || type.endsWith('_tier_prefix')) {
    return '16_dependent_tiers';
  }
  // %mor tier
  if (type.startsWith('mor_') || type === 'langcode') {
    return '17_mor_tier';
  }
  // %gra tier
  if (type.startsWith('gra_')) {
    return '18_gra_tier';
  }
  // %pho tier
  if (type.startsWith('pho_')) {
    return '19_pho_tier';
  }
  // %sin tier
  if (type.startsWith('sin_')) {
    return '20_sin_tier';
  }
  // %wor tier
  if (type.startsWith('wor_')) {
    return '21_wor_tier';
  }
  // Media/timing
  if (['inline_bullet', 'inline_pic', 'text_with_bullets',
       'text_with_bullets_and_pics'].includes(type)) {
    return '22_media_timing';
  }
  // Symbols and punctuation
  if (['comma', 'colon', 'semicolon', 'pipe', 'plus', 'hyphen', 'tilde',
       'ampersand', 'star', 'zero', 'less_than', 'greater_than',
       'left_bracket', 'right_bracket', 'right_brace',
       'double_quote', 'left_double_quote', 'right_double_quote',
       'separator', 'non_colon_separator',
       'break_for_coding', 'uptake_symbol',
       'quoted_new_line', 'quoted_period_simple'].includes(type)) {
    return '23_symbols';
  }
  // Whitespace
  if (['whitespaces', 'space', 'tab', 'newline'].includes(type)) {
    return '24_whitespace';
  }
  // Catch-all for anything missed
  return '25_other';
}

const categoryLabels = {
  '01_document_structure': 'Document Structure',
  '02_required_headers': 'Required Headers',
  '03_participant_headers': 'Participant Headers (@Participants, @ID)',
  '04_structured_headers': 'Structured Headers',
  '05_text_headers': 'Text Headers',
  '06_main_tier': 'Main Tier',
  '07_terminators': 'Terminators',
  '08_linkers': 'Linkers',
  '09_content_items': 'Content Items and Words',
  '10_events': 'Events',
  '11_groups': 'Groups',
  '12_annotations': 'Annotations',
  '13_overlap': 'Overlap',
  '14_prosody': 'Prosody (Pauses and Intonation)',
  '15_conversation_analysis': 'Conversation Analysis Markers',
  '16_dependent_tiers': 'Dependent Tiers',
  '17_mor_tier': '%mor Morphology Tier',
  '18_gra_tier': '%gra Grammatical Relations Tier',
  '19_pho_tier': '%pho Phonology Tier',
  '20_sin_tier': '%sin Tier',
  '21_wor_tier': '%wor Original Word Tier',
  '22_media_timing': 'Media and Timing',
  '23_symbols': 'Symbols and Punctuation',
  '24_whitespace': 'Whitespace',
  '25_other': 'Other',
};

// Group types by category
const grouped = new Map();
for (const type of sortedTypes) {
  const cat = categorize(type);
  if (!grouped.has(cat)) {
    grouped.set(cat, []);
  }
  grouped.get(cat).push(type);
}

// Sort categories by key
const sortedCategories = Array.from(grouped.keys()).sort();

// Generate Rust code
console.log('//! Generated node type constants from tree-sitter grammar.');
console.log('//!');
console.log('//! DO NOT EDIT THIS FILE MANUALLY!');
console.log('//! This file is auto-generated by `scripts/generate-node-types.js`.');
console.log('//!');
console.log('//! To regenerate:');
console.log('//! ```sh');
console.log('//! node scripts/generate-node-types.js > crates/talkbank-parser/src/node_types.rs');
console.log('//! ```');
console.log('//!');
console.log(`//! All ${sortedTypes.length} named node types from the tree-sitter CHAT grammar`);
console.log('//! are available as compile-time `&str` constants.');
console.log('//!');
console.log('//! # Example');
console.log('//! ```');
console.log('//! use talkbank_parser::node_types::BEGIN_HEADER;');
console.log('//!');
console.log('//! assert_eq!(BEGIN_HEADER, "begin_header");');
console.log('//! ```');
console.log('//!');
console.log('//! # Related CHAT Manual Sections');
console.log('//!');
console.log('//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>');
console.log('//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>');
console.log('//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>');
console.log('');
console.log('#![allow(dead_code)] // Generated full grammar catalog; downstream crates use subsets.');
console.log('#![allow(clippy::doc_markdown)]');
console.log('');

// Generate constants grouped by category
let firstCategory = true;
for (const cat of sortedCategories) {
  const typesInCat = grouped.get(cat);
  const label = categoryLabels[cat] || cat;

  if (!firstCategory) {
    console.log('');
  }
  firstCategory = false;

  console.log(`// === ${label} ===`);
  console.log('');
  for (const type of typesInCat) {
    const constName = type.toUpperCase().replace(/-/g, '_');
    const doc = docs[type] || `CST node type: \`${type}\``;
    console.log(`/// ${doc}.`);
    console.log(`pub const ${constName}: &str = "${type}";`);
  }
}

// Generate statistics comment
console.log('');
console.log(`// Total node types: ${sortedTypes.length}`);
