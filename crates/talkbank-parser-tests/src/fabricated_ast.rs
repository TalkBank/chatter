//! Two ratchets over how model values come to exist.
//!
//! The lexical counts are not correctness or CHAT-coverage metrics. They hold
//! two populations at their measured size so that each may only shrink.
//!
//! # What this counts, and why those spellings
//!
//! **Fabrications.** `new_unchecked` bypasses checked model construction.
//! The historical `from_checked_literal` spelling remains counted conservatively,
//! though it now requires sealed `NonEmptyLiteral` evidence and cannot accept raw
//! strings. These lexical totals are not a count of reachable invariant holes.
//! Until 2026-10-18 the
//! list also carried `Span::DUMMY`, the sentinel `Span { start: 0, end: 0 }`,
//! which was ALSO the legal zero-width position at the first byte of a file.
//! The source-location batches deleted it: a value with no source position is
//! `Location::Unlocated`, a variant, not a span that happens to be zero, so
//! the overloaded value this gate was built to count no longer exists.
//!
//! **Parser-minted unlocated values.** A parser is where values are born, and
//! it has every node's span in hand; a parser that writes `Location::Unlocated`
//! has thrown away a location it had. The tree-sitter parser's ceiling is
//! ZERO. The re2c parser's lexer keeps no offsets for the items inside a tier,
//! so its count is a measured ceiling that falls as the lexer learns them.
//! Only PRODUCTION text counts here: a parser's own tests build values by hand
//! like every other test. `DiagnosticLocation::Unlocated` is a different
//! statement (a diagnostic with nothing to point at, which a parser may
//! legitimately raise about an unrepresentable input) and is not counted.
//!
//! # Why a ratchet rather than a ban
//!
//! Both fabrication spellings are load-bearing today. CHAT semantic examples
//! belong in specs/reference inputs, but constructor, wire-format and
//! failure-boundary tests legitimately exercise values that no parser should
//! admit. Preserve those contracts. Typed construction is a supported producer,
//! distinct from parser provenance; derived raw spelling cannot contradict its
//! structure. The lexical ratchet prioritizes review, not automatic deletion of
//! tests.
//!
//! # The dev-dependency shortcut, tried 2026-09-08 and half misread
//!
//! Cargo permits DEV-dependency cycles, so `talkbank-model` can name
//! `talkbank-parser` as a dev-dependency even though the parser depends on the
//! model. For the LIB-TEST target it is useless, and the compiler says why:
//!
//! > there are multiple different versions of crate `talkbank_model` in the
//! > dependency graph
//!
//! That target IS one copy of `talkbank-model` and the parser links against
//! ANOTHER, so `parse_word` returns a `Word` that is not the `Word` a
//! `#[cfg(test)] mod` in `src/` can use. Two distinct types with one name.
//!
//! What this paragraph said until an adversarial review on 2026-09-08 was
//! that the shortcut therefore does not exist at all. Wrong: an INTEGRATION
//! test under `talkbank-model/tests/` links against the non-test lib and has
//! no second copy to fight, and this workspace already relies on exactly that
//! (`talkbank-derive` dev-depends on `talkbank-model`, by design). So a test
//! in this crate CAN parse, from `tests/`. The reason the migrations went to
//! `talkbank-parser-tests` instead is cost, not possibility: it already
//! depends on every parser, and a dev-dependency on tree-sitter would make
//! the model's own test build wait for the grammar to compile.
//!
//! # Every direction fails, and that is deliberate
//!
//! Over the ceiling is a regression. UNDER it is also a failure, with the new
//! number in the message, because a ceiling that silently sits above reality
//! has stopped having teeth: it leaves room for a new fabrication to arrive
//! under cover of an unrelated deletion. Lower it in the commit that earned it.
//!
//! A ceiling row naming a crate the walk never saw is the third, and it was
//! the one this gate could not say. Absence was read as zero on both sides, so
//! a renamed or deleted crate looked exactly like a cleaned one and the gate
//! printed "lower the ceiling". [`Standing`] gives the three situations three
//! names, and the rename case then fails twice over: the old row is
//! [`Standing::Vanished`] and the new name is [`Standing::Unlisted`] carrying
//! the whole population against a ceiling of zero.
//!
//! # It was a Python script until 2026-09-08, and that was the wrong altitude
//!
//! `test_hygiene`'s module doc argues the case in full and this gate is the
//! same argument applied to itself: a check under `scripts/` reads the
//! filesystem directly, so no probe can plant into it; it needs hand-written
//! wiring in the justfile to run at all; and it grows its own baseline format
//! and its own hand-rolled both-directions test. As a [`Gate`] it reads through
//! a [`Tree`](crate::gate::Tree), carries an
//! [`Examined`](crate::gate::Examined) witness, declares probes that a compiler
//! demands, and runs under `cargo test --workspace --tests`, which is CI, by
//! construction rather than by wiring.
//!
//! One measurement changed with the move, and it is worth stating: the Python
//! read `git ls-files`, so an untracked file could not move the count. This
//! walks the tree, like every other gate here, so an untracked file in
//! `crates/` counts. That is the stricter reading and the one the sibling gates
//! already take.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, UnprovenRule, listing, report};
use crate::test_hygiene::blank_literals;

/// The spellings that mint a model value without a parse behind it.
///
/// Deliberately literal rather than a syntactic analysis: this is a ratchet on
/// a population, not a proof about any one call site.
const FABRICATIONS: &[&str] = &["new_unchecked", "from_checked_literal"];

/// The spellings that leave a model value without a source position.
///
/// Matched only where the preceding character is not part of an identifier,
/// so `DiagnosticLocation::Unlocated` is not a hit. The second spelling is the
/// serde-default function; a parser has no business naming it either.
const UNLOCATED_MINTS: &[&str] = &["Location::Unlocated", "Location::unlocated"];

/// Per-crate ceiling.
///
/// A RATCHET. These may only go DOWN, lowered in the same commit that earned
/// it. A crate ABSENT from this list has a ceiling of ZERO, which is what stops
/// a new crate full of fabrication from arriving without any number moving.
const CEILING: &[(&str, usize)] = &[
    // `chatter` left the list on 2026-10-18, batches 3c and 3d: its one
    // sentinel was a `Span::DUMMY`.
    // `talkbank-lsp` left the list on 2026-09-09: its seven sentinels were
    // the hand-built utterances of the dependency-graph tests and of the
    // post-clitic hover-label test, all parsed CHAT now.
    // 160 -> 154 the same day: the emptiness-decision tests of
    // `dependent_tier/kind.rs` and the trailing-separator tests of
    // `content/tier_content.rs`, over parsed files now. Every state they
    // built by hand (`TextTier::empty()`, whitespace-only content) is what
    // the parser builds for a tier line with nothing, or only whitespace,
    // after the tab.
    //
    // 172 -> 160 the same day: five small inline modules, over parsed files
    // now: `%pho`/`%mod` alignment, cross-speaker overlap grouping, the
    // `@Time Duration` / `@Time Start` format checks, E756 for an empty
    // dependent tier, and E305 with its CA-mode exemption.
    //
    // 179 -> 172 the same day: seven inline overlap-region tests and five
    // inline whole-file alignment tests, over parsed files now. The unpaired
    // overlap rows are valid CHAT on purpose: E348 is deliberately
    // suppressed (`spec/errors/E348.md`), and the extractor's not-well-paired
    // verdict is what they pin.
    //
    // 186 -> 179 the same day: five of the six inline `%sin` alignment tests,
    // over parsed tiers; the sixth (empty on empty) stays, since a `%sin:`
    // line with no token is E342 at parse.
    //
    // 206 -> 186 the same day: the inline test modules of
    // `validation/header/mod.rs` (nine header-value tests; two stay, over a
    // speaker code containing `:` that no parse builds) and of
    // `validation/cross_utterance/scoped_markers.rs` (thirteen scope-balance
    // tests, whose bare `Group` no parse builds either; the two
    // parser-producible angle shapes stand in). Rows over parsed files now.
    //
    // 210 -> 206 the same day: the four `%mor` tier chunk-sequence tests,
    // over a parsed tier now; the item-level tests stay, holding no span.
    //
    // 223 -> 210 the same day: the seven `%mor` to `%gra` alignment tests
    // and the three `%wor` corroboration tests, over parsed tiers now. What
    // that found: the gra originals used `SUBJ`, which is not a Universal
    // Dependencies relation (E761) and could never have been parsed from
    // valid CHAT, and a `%gra` of one `PUNCT` with no `ROOT` is E722.
    //
    // 266 -> 223 the same day: the twenty cross-utterance linker tests, six
    // files of hand-built dialogues whose `Terminator::QuotedNewLine` stood
    // in for the text `+"/.`, rows over parsed dialogues now, under the same
    // opt-in rule selection, with every exact count and message fragment.
    //
    // 278 -> 266 the same day: the seven word-validation `insta` snapshots,
    // each over a `WordContent` list written by hand (one put a stress marker
    // in the content of `WOrd`, a text containing none). Five said
    // `[No errors]` and are `CLEAN` words in the parsed word table now; the
    // two messages are asserted there over parsed input, E232's through the
    // independent backend, the one route from CHAT text to it. The crate's
    // `insta` dev-dependency went with them.
    //
    // 290 -> 278 the same day: the per-domain unit-counting tests, rows over
    // parsed tiers now. Every hand-built shape has a spelling the parser
    // accepts; six are invalid CHAT on purpose and say which code.
    //
    // 313 -> 290 the same day: the twelve `%wor` timing tests and the four
    // `%wor` sidecar tests, over a parsed main tier and `%wor` tier now. The
    // only hand-built input no parse produces was an EMPTY main tier; the
    // state it reached (zero eligible slots) is reached from a parse by an
    // event-only utterance. The location file used to say no end-to-end
    // route reaches the sidecar value; a parsed utterance owns both tiers.
    //
    // 329 -> 313 the same day: fifteen utterance-metadata tests, each of which
    // had written the CHAT it meant in a comment beside the struct it built;
    // the comment is the input now. Every assertion travelled, provenance
    // included. Two stay, about a constructed "analysis pending" state that
    // no parse reaches and that builds no utterance.
    //
    // 347 -> 329 the same day: ten `MainTier` method tests, over parsed tiers
    // now. What that move found: `generate_wor_tier` copies
    // `word.inline_bullet`, and no main-tier PARSE ever sets it (timing after
    // a word is a separate `internal_bullet` item). The field is set by the
    // `%wor` parsers, by JSON, and by a consumer timing a main tier in memory
    // before projecting it, which is how forced alignment writes `%wor`. The
    // two tests of that copy branch stay, saying so, as its only pin.
    //
    // 358 -> 347 the same day: six alignment count-mismatch tests, which
    // asserted a diagnostic landed at `Span::new(0, 20)` beside a
    // comment saying that span was `*CHI: one two .`. The replacement is a
    // STRONGER claim, not the same one relocated: it reads the main tier's
    // span off the parse and asserts the diagnostic points there, whatever
    // the offset, which a hardcoded number cannot say. What stays asserts
    // internal values a user never sees, `WorTimingSidecar::Drifted` and the
    // absence of an `ErrorContext` before it is populated.
    //
    // 385 -> 358 the same day: sixteen utterance-balance tests, which built
    // an `Utterance` and called `check_quotation_balance`,
    // `check_underline_balance` and their siblings directly. Four of the five
    // codes were demonstrated by spec examples already; E242's were not, in
    // the sense that mattered, because the spec exercises a curly-quote scan
    // in the parser while the validator's rule reads postcodes, and a first
    // draft of the move replaced the postcode tests with curly-quote rows and
    // deleted that rule's only coverage while staying green. The review
    // caught it; the postcode rows are back with their messages. The single
    // survivor asserts the return value of a `pub(crate)` function, which
    // nothing outside the crate can call; its file says so.
    //
    // 445 -> 385 the same day, and the same move: twenty-four
    // header-structure tests, which handed `check_header_order` and
    // `check_gem_balance` a header sequence assembled in the test with
    // `Span::DUMMY` at every position. Header ORDER and GEM BALANCE are
    // properties of a header block, which a `.cha` file states directly, so
    // that sequence was a second way of writing what the format already
    // writes. The whole `mod tests` went; nothing in that file builds an AST
    // any more.
    //
    // 484 -> 445 on 2026-09-08: twenty-two word-validation tests moved to
    // `talkbank-parser-tests`, where a test can parse its own input. They
    // could not be fixed where they stood, because every parser depends on
    // this crate, so a test inside it has no way to parse; that dependency is
    // the whole reason the population is concentrated here.
    //
    // The move is not a relabelling. Two of the twenty-two turned out to
    // assert rules over `Word` values the CANONICAL parser never produces
    // (re2c does build one of them), and one asserted a reading the spec no
    // longer holds: `:test` was claimed for E246, and a leading `:` has been
    // E765 since 0.19.0 on 2026-09-05. Nothing could contradict it while the
    // test built the `Word` itself. A third, `test_valid_compound`, passed
    // vacuously all along: `Word::simple("un+do")` carries no
    // `CompoundMarker`, so the check it was named for never ran.
    //
    // 484, not the 485 the Python recorded: one occurrence sits inside a
    // STRING LITERAL, which the Python counted and this does not. Nobody
    // removed it.
    // 153 -> 143 on 2026-09-09: the thirteen word-language resolution tests of
    // `validation/word/language/tests.rs`, over parsed documents now
    // (`word_language_from_source.rs`); the tag-mapping test stays, since it
    // builds no word.
    // 143 -> 134 on 2026-09-09: `tests/temporal_validation_tests.rs`, nine
    // `Span::DUMMY` sentinels in hand-built main tiers, is eleven rows of CHAT
    // in `temporal_from_source.rs`.
    // 134 -> 127 on 2026-09-09: the three inline tests of `model/mod.rs`,
    // two of them over a parsed word and tier now
    // (`word_structure_from_source.rs`), the third a constructor's echo,
    // deleted.
    // 127 -> 122 on 2026-09-09: the walker suite, `alignment/helpers/walk/
    // tests.rs`, is `walk_from_source.rs` in this crate, every content list
    // parsed from a line.
    // 122 -> 114 on 2026-09-09: the alignment computation read each tier's
    // span through `map_or(Span::DUMMY, ..)` and then only ever used it
    // inside the branch that already held the tier; the branch reads the
    // tier's own span now and the partner side of the grouped warning is an
    // `Option<Span>`. The one sentinel the module still writes is the
    // location of a warning about tiers with no known location (re2c sets
    // no tier spans), in one documented function.
    // 114 -> 107 on 2026-09-09: the word type's cleaned-text tests parse
    // their spellings (`↫sch↫schaap`, `∆fast∆`) instead of building the
    // pieces by hand under an ignored cleaned text, and its mutation test
    // builds its replacement texts through the checked constructor.
    // 107 -> 105 the same day: `non_empty_literal!` proves a literal
    // non-empty at compile time and reaches the type through
    // `from_checked_literal`, a door this list counts (its definition and
    // the macro's one call are the two sites that arrived); four test
    // literals in `validation/{utterance,word}/tests.rs` went through it.
    // 100 -> 99 on 2026-10-18: the gate reported the ceiling above reality
    // (drift that predates the span geometry batch, whose sentinel counts
    // are unchanged), tightened with the source-location batch 3a.
    // 99 -> 23 on 2026-10-18, batches 3c and 3d: `Span::DUMMY` is gone from
    // the language. Every former sentinel is a `Location::Unlocated` argument
    // to a constructor that now demands one, and that spelling is not a
    // fabrication outside the parsers (see `UNLOCATED_MINTS`).
    // 23 -> 19: remove the unused unchecked phonetic and shortening
    // constructors, including their unchecked primitive calls.
    // 19 -> 18: WOR projection retains nonempty text admission and uses
    // the checked word constructor instead of unchecked display construction.
    // 18 -> 17: simple replacement accepts WordText; paired redaction retains
    // the projection's admitted text instead of constructing it unchecked.
    // 17 -> 2: remove Word::simple and all three unchecked text/word
    // constructors; migrate tests to checked input. The remaining two are
    // the evidence-taking literal constructor and its macro invocation.
    ("talkbank-model", 2),
    // 8 -> 6 the same day: the user-defined and unsupported tier dispatchers
    // read their prefix through `expect_present` and one `tier_name` helper
    // (a MISSING prefix builds no tier, where its empty placeholder text
    // used to become the label `x`, or nothing), and the stored `x` label is
    // `NonEmptyString::from_head_and_tail`, non-empty by its first character.
    //
    // 9 -> 8 on 2026-09-09: the tree-sitter parser builds a `Word` through
    // the checked `Word::new(NonEmptyString, WordText)`, its two emptiness
    // guards now the constructors' refusals, so the parser's one
    // `new_unchecked` call is gone.
    // 6 -> 0 on 2026-10-18, batches 3c and 3d: the six were `Span::DUMMY`.
    // The row is gone; the crate's ceiling is the implicit zero.
    // 23 -> 21 on 2026-09-09: the two tier labels the converter wrote for a
    // `%phoaln`/`%xphoint` line that would not parse are `non_empty_literal!`.
    // 21 -> 18 on 2026-10-18: the same drift report as `talkbank-model`,
    // tightened with the source-location batch 3a.
    // 18 -> 4 the same day, batches 3c and 3d: the rest were `Span::DUMMY`,
    // and the converter's unlocated items are counted by the second ratchet.
    // 4 -> 3: structured Word construction no longer seeds unchecked lexical text.
    // 3 -> 2: user-defined labels retain their observed head and suffix through
    // NonEmptyString's structurally non-empty constructor.
    // 2 -> 0: both word producers admit shared WordText and WordShortening;
    // conversion only retains those values. The implicit ceiling is now zero.
    // 10 -> 9 and 12 -> 11 on 2026-09-08, the first two conversions this
    // ratchet was built to make provable: `test_complex_signature` and
    // `raw_text_rebuild_preserves_nonlexical_word_markers` now PARSE their
    // input instead of stating it twice.
    // 9 -> 6 the same day: the three hand-built words of the utterance
    // writer tests are `Word::new` over compile-time non-empty literals; the
    // three `Span::DUMMY` terminators stay, a hand-built terminator having no
    // source and the writer reading no span.
    // 6 -> 0 on 2026-10-18, batches 3c and 3d: the three `Span::DUMMY`
    // terminators of the utterance writer tests are `Location::Unlocated`,
    // and the row is gone.
    // 11 -> 7 the same day: the sanitizer's placeholders are proven text
    // (`PlaceholderToken` holds a `NonEmptyString`, a literal prefix and
    // the index), handed to the word-text types through `From`; the
    // shortening placeholder and one test literal are `non_empty_literal!`.
    // 6 -> 0 on 2026-10-18, batches 3c and 3d: all six were `Span::DUMMY`.
];

/// Per-parser ceiling on unlocated values minted in PRODUCTION code.
///
/// A RATCHET like [`CEILING`], over the parser crates only: every other crate
/// may build an unlocated value, which is what `Location::Unlocated` is for.
const PARSER_UNLOCATED_CEILING: &[(&str, usize)] = &[
    // Every node has a span, and the fragment API rebases what it wraps. The
    // one fallback arm it had (an annotated word's wrapper span, read off the
    // word "in case" it was unlocated) went with batch 3c: the construct
    // starts where its node starts.
    ("talkbank-parser", 0),
    // The re2c lexer records offsets for tiers, words and headers, not for the
    // separators, terminators, markers and `%mor`/`%gra` items inside a tier,
    // which it rebuilds from kinds alone. Measured on 2026-10-18 with batch
    // 3c; falls as the lexer learns the offsets.
    // 110 -> 107 on 2026-10-18: a spelling change, not learned offsets. The
    // main tier's three positional locations became one
    // `MainTierLocations::UNLOCATED`, which this textual count does not see;
    // the three unlocated parts are still unlocated.
    // 107 -> 55 on 2026-10-18 (Part B1): learned offsets. Tokens for
    // terminators, linkers, overlap and underline markers, the action marker
    // and `%wor` separators carry their whole lexeme, located through
    // `convert::locate`; main-tier locations and dependent- and text-tier
    // extents come from the line machine (`LexedSource::main_tier_locations`,
    // `LexedSource::extent`). What remains: items whose rich token keeps only
    // a payload (the `[- eng]` precode, events, freecodes, long features,
    // nonvocals, other spoken events), hull extents for retraces, groups,
    // quotations and replaced words, word-body markers and the media comma's
    // leading whitespace (Parts B3 and B4).
    // 55 -> 44 on 2026-10-19 (Part B3): every `WordBodyItem` marker carries
    // its lexeme (compound, stress, syllable pause, clitic boundary, overlap
    // point, CA element and delimiter, underline begin and end) and the
    // lengthening run is a `LengtheningRun`, so `convert/words.rs` places
    // them through `convert::locate`. What remains there is the word's own
    // `map_or` fallback for a subtoken-rebuilt `raw_text`.
    // 44 -> 21 on 2026-10-19 (Part B4, lexer): every tag-extracting rule
    // (`emit_t1!`, `emit_t1t2!`) emits an `Extracted` carrying its payload AND
    // its whole lexeme, so events, freecodes, long features, nonvocals, the
    // other-spoken event, the `[- eng]` precode, bullets (main-tier, inline
    // and `%wor`) and the media comma's leading whitespace are placed at
    // themselves. What remains: hull extents for retraces, groups, quotations
    // and replaced words and their `Annotated` wrappers (no opening or
    // closing token kept yet), an orphan annotation lowered as a freecode
    // (`ParsedAnnotation` and `ScopedAnnotationParsed` still carry payload
    // slices; carrying `Extracted` there is the companion to the hull work),
    // the subtoken-rebuilt word's `raw_text`, and the honest `Unlocated`
    // arms of `locate` and the slice fallbacks.
    // 21 -> 3 on 2026-10-19 (Part B4, hulls): every annotation keeps its
    // lexeme (`ScopedAnnotationParsed`, `ParsedAnnotation::Retrace`, the
    // precode and postcode tokens), groups and quotations keep their
    // delimiters (`Enclosure`), a retrace keeps its marker and, when it took
    // a bare group's brackets, those too, and a replacement keeps its `[:`.
    // Retraces, groups, quotations, replaced words, their `Annotated`
    // wrappers (at both content levels) and orphan-annotation freecodes are
    // placed as hulls; the parser's pre-parse mirror of E757 is gone, since
    // the model rule now fires on these wrappers.
    // 3 -> 2 on 2026-10-19 (Part B4, words): a word the parser assembles from
    // subtokens borrows its covering source slice (first token through last)
    // instead of concatenating display forms, so every word is placed and
    // the annotated hull refuses like the retrace hulls instead of skipping.
    // What remains, textually: the one honest fallback arm of `located`,
    // which every placement routes through, and the separator slice
    // fallback. Both answer `Unlocated` only for a slice that is not from
    // the source being converted, a programming error.
    // Shared separator lowering removed the second fallback; only
    // convert::located still mints an unlocated model value.
    ("talkbank-parser-re2c", 1),
];

/// Fabricated-AST construction may only fall.
pub struct FabricatedAstGate;

impl Gate for FabricatedAstGate {
    fn name(&self) -> &'static str {
        "fabricated-AST construction"
    }

    fn check(&self, tree: ReadTree) -> Outcome {
        let counted = match count_by_crate(&tree) {
            Ok(counted) => counted,
            Err(failure) => return Outcome::failed(failure),
        };
        let fabricated: BTreeMap<String, usize> = counted
            .iter()
            .map(|(crate_name, counts)| (crate_name.clone(), counts.fabrications))
            .collect();
        // Only the parsers are held on unlocated mints; every other crate's
        // count is measured and ignored, which is what makes the row list the
        // whole policy.
        let parser_rows: BTreeMap<&str, usize> = PARSER_UNLOCATED_CEILING.iter().copied().collect();
        let unlocated: BTreeMap<String, usize> = counted
            .iter()
            .filter(|(crate_name, _)| parser_rows.contains_key(crate_name.as_str()))
            .map(|(crate_name, counts)| (crate_name.clone(), counts.unlocated_mints))
            .collect();

        let mut verdict = Verdict::default();
        verdict.absorb(
            "fabrication",
            &fabricated,
            &CEILING.iter().copied().collect(),
        );
        verdict.absorb("parser-minted unlocated value", &unlocated, &parser_rows);

        if verdict.is_clean() {
            let total: usize = fabricated.values().sum();
            let carrying = fabricated.values().filter(|found| **found > 0).count();
            let minted: Vec<String> = unlocated
                .iter()
                .map(|(crate_name, now)| format!("{crate_name} {now}"))
                .collect();
            return tree.clean(format!(
                "fabricated-AST construction: {total} across {carrying} crate(s), at the \
                 ceiling; parser-minted unlocated values: {}, at the ceiling",
                minted.join(", ")
            ));
        }
        Outcome::failed(report([
            listing(
                "FAIL: the population GREW. A test that builds its own AST states the\n\
                 input twice, and nothing forces the raw text and the parsed structure\n\
                 to agree; a parser that leaves a value unlocated has thrown away a\n\
                 span it had. Reach for a spec example, which is lowered to a real\n\
                 file and run through both stages, or pass the node's span:",
                &verdict.grew,
            ),
            listing(
                "FAIL: the ceiling now sits ABOVE reality, which leaves room for a new\n\
                 fabrication to arrive under cover of an unrelated deletion. Lower\n\
                 these in this commit:",
                &verdict.shrank,
            ),
            listing(
                "FAIL: the ceiling names a crate the walk never saw. A rename moves\n\
                 every one of its fabrications to a crate with no row, and a deletion\n\
                 is progress that has to be banked rather than left as headroom.\n\
                 Move the row or delete it:",
                &verdict.vanished,
            ),
        ]))
    }

    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a new fabrication in a crate already at its ceiling",
            "1 added",
            |edit| {
                edit.write(
                    PROBE_FILE,
                    "fn f() { let s = Span::new_unchecked(0, 0); }\n",
                );
                Ok(())
            },
        )
        .refusing(
            "a crate ABSENT from the ceiling, which starts at zero",
            "talkbank-probe-new: 0 -> 1",
            |edit| {
                edit.write(
                    "crates/talkbank-probe-new/src/lib.rs",
                    "fn f() { let s = Span::new_unchecked(0, 0); }\n",
                );
                Ok(())
            },
        )
        .refusing(
            "an unlocated value minted in the tree-sitter parser",
            "talkbank-parser: 0 -> 1",
            |edit| {
                edit.write(
                    PARSER_PROBE_FILE,
                    "fn f() -> talkbank_model::Location { talkbank_model::Location::Unlocated }\n",
                );
                Ok(())
            },
        )
        .accepting(
            "an unlocated value in a parser's TESTS is not a mint",
            |edit| {
                edit.write(
                PARSER_PROBE_FILE,
                "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn g() -> talkbank_model::Location { \
                 talkbank_model::Location::Unlocated }\n}\n",
            );
                Ok(())
            },
        )
        .accepting(
            "an unlocated DIAGNOSTIC is not an unlocated value",
            |edit| {
                edit.write(
                    PARSER_PROBE_FILE,
                    "fn f() -> talkbank_model::DiagnosticLocation { \
                 talkbank_model::DiagnosticLocation::Unlocated }\n",
                );
                Ok(())
            },
        )
        .accepting(
            "an unlocated value outside the parsers is a builder's right",
            |edit| {
                edit.write(
                    PROBE_FILE,
                    "fn f() -> crate::Location { crate::Location::Unlocated }\n",
                );
                Ok(())
            },
        )
        .refusing(
            "the ceiling sits above reality, so progress is unbanked",
            "sits ABOVE reality",
            |edit| {
                // Every fabrication in the largest crate at once. Banking is
                // the half people forget, and a ceiling above reality is where
                // a new fabrication hides.
                edit.hide_files_under("crates/talkbank-model/src");
                Ok(())
            },
        )
        .refusing(
            "the ceiling names a crate that is no longer in the tree",
            "no file of that crate is in the tree",
            |edit| {
                // A RENAME is this plant plus the same files under a new name,
                // and the new name has no ceiling row, so it fails twice. The
                // deletion half is the one that used to pass silently: with
                // absence reading as zero, `chatter` went 1 -> 0 and the gate
                // said "lower the ceiling", banking a cleanup nobody did.
                // `chatter` has no row now; the re2c parser sits in both lists.
                edit.remove_dir("crates/talkbank-parser-re2c");
                Ok(())
            },
        )
        .refusing(
            "the crates tree cannot be ENUMERATED",
            "could not enumerate",
            |edit| {
                edit.fail_walk_under("crates");
                Ok(())
            },
        )
        .accepting(
            "a fabrication NAMED in a comment is not a fabrication",
            |edit| {
                edit.write(
                    PROBE_FILE,
                    "// Never write new_unchecked here, and never from_checked_literal.\n\
                     /// `new_unchecked` is the hazard this gate counts.\n",
                );
                Ok(())
            },
        )
        .accepting("a file outside `crates/` is out of scope", |edit| {
            edit.write(
                "xtask/src/probe.rs",
                "fn f() { let s = Span::new_unchecked(0, 0); }\n",
            );
            Ok(())
        })
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}

/// What is known about one crate: what the walk found, what the ceiling records.
///
/// Both halves used to be read with `unwrap_or_default`, which made ABSENCE
/// mean zero on each side and hid that the two absences are different facts. A
/// crate the walk never saw is not a crate it saw and found clean, and reading
/// the first as the second let a RENAMED or DELETED crate report as progress:
/// its whole population moved to a name with no ceiling row, and this gate
/// printed "lower the ceiling", which banks a cleanup nobody did.
///
/// Three variants because the cross-product has three inhabited cells. The
/// fourth, in neither map, is not constructible: every standing is built from a
/// key that came out of one map or the other, so there is no arm to write and
/// no `unreachable!` to panic through.
enum Standing<'a> {
    /// Walked and listed: both numbers are real.
    Listed {
        crate_name: &'a str,
        now: usize,
        was: usize,
    },
    /// Walked, with no ceiling row. The ceiling is ZERO by the policy stated on
    /// [`CEILING`], which is what makes a new crate full of fabrication a
    /// failure rather than a silence.
    Unlisted { crate_name: &'a str, now: usize },
    /// Listed, never walked.
    Vanished { crate_name: &'a str, was: usize },
}

/// Every crate either side knows about, paired with what is known about it.
///
/// Built by walking each map once rather than by unioning the key sets and
/// looking both up, so the impossible combination never has to be named.
fn standings<'a>(
    counted: &'a BTreeMap<String, usize>,
    recorded: &BTreeMap<&'a str, usize>,
) -> Vec<Standing<'a>> {
    let mut all: Vec<Standing<'a>> = counted
        .iter()
        .map(
            |(crate_name, &now)| match recorded.get(crate_name.as_str()) {
                Some(&was) => Standing::Listed {
                    crate_name,
                    now,
                    was,
                },
                None => Standing::Unlisted { crate_name, now },
            },
        )
        .collect();
    all.extend(
        recorded
            .iter()
            .filter(|(crate_name, _)| !counted.contains_key(**crate_name))
            .map(|(&crate_name, &was)| Standing::Vanished { crate_name, was }),
    );
    all
}

/// The file every fabrication-counting probe writes into.
///
/// Under a crate that already has a ceiling, so the probe that adds one moves
/// that crate's number rather than introducing a crate.
const PROBE_FILE: &str = "crates/talkbank-model/src/probe_fabricated_ast.rs";

/// The file the unlocated-mint probes write into: production text of the
/// tree-sitter parser, whose ceiling is zero.
const PARSER_PROBE_FILE: &str = "crates/talkbank-parser/src/probe_unlocated.rs";

/// The three failure lists one population contributes to the report.
///
/// Both populations are judged by the same three rules ([`Standing`]), so the
/// judging is written once and each population names itself in its rows.
#[derive(Default)]
struct Verdict {
    grew: Vec<String>,
    shrank: Vec<String>,
    vanished: Vec<String>,
}

impl Verdict {
    fn is_clean(&self) -> bool {
        self.grew.is_empty() && self.shrank.is_empty() && self.vanished.is_empty()
    }

    /// Judge one population against its ceiling rows.
    fn absorb(
        &mut self,
        population: &str,
        counted: &BTreeMap<String, usize>,
        recorded: &BTreeMap<&str, usize>,
    ) {
        for standing in standings(counted, recorded) {
            match standing {
                Standing::Listed {
                    crate_name,
                    now,
                    was,
                } => match now.cmp(&was) {
                    Ordering::Greater => self.grew.push(format!(
                        "{crate_name}: {was} -> {now}, {} added ({population})",
                        now - was
                    )),
                    Ordering::Less => self.shrank.push(format!(
                        "(\"{crate_name}\", {now}),   // was {was} ({population})"
                    )),
                    Ordering::Equal => {}
                },
                Standing::Unlisted { crate_name, now } => {
                    if now > 0 {
                        self.grew.push(format!(
                            "{crate_name}: 0 -> {now}, {now} added \
                             (no ceiling row, so the ceiling is zero; {population})"
                        ));
                    }
                }
                Standing::Vanished { crate_name, was } => self.vanished.push(format!(
                    "{crate_name}: the ceiling records {was}, and no file of that \
                     crate is in the tree ({population})"
                )),
            }
        }
    }
}

/// What one crate's files add up to, in both populations.
#[derive(Clone, Copy, Default)]
struct Counts {
    fabrications: usize,
    unlocated_mints: usize,
}

/// Occurrences per crate, over the CODE of each file.
///
/// # Errors
///
/// When the walk fails or a file cannot be read. Either makes the count a floor
/// rather than a measurement, and a floor compared against a ceiling reports
/// progress that did not happen.
fn count_by_crate(tree: &ReadTree) -> Result<BTreeMap<String, Counts>, String> {
    let files = tree
        .files_under("crates")
        .map_err(|err| format!("FAIL: could not enumerate the crates tree: {err}"))?;
    let mut per: BTreeMap<String, Counts> = BTreeMap::new();
    for path in files {
        let as_str = path.as_str();
        if as_str.contains("/target/") {
            continue;
        }
        let Some(crate_name) = crate_of(as_str) else {
            continue;
        };
        // A file of ANY kind sights the crate, and only a Rust file is read.
        // That is what makes a crate with no fabrication an entry holding zero
        // rather than no entry at all, which is the distinction
        // [`Standing::Vanished`] rests on: `Cargo.toml` is enough to say the
        // crate is here.
        let found = if path.extension_is("rs") {
            let text = tree.read_to_string(&path).map_err(|err| {
                format!("FAIL: a file could not be read, so the count is a floor: {err}")
            })?;
            Counts {
                fabrications: fabrications_in(&text),
                unlocated_mints: if is_test_path(as_str) {
                    0
                } else {
                    unlocated_mints_in(production_text(&text))
                },
            }
        } else {
            Counts::default()
        };
        // Not `entry(crate_name.to_owned())`, which allocates the key on every
        // file to throw it away on all but the first. The name is only owned
        // when it is first seen.
        match per.get_mut(crate_name) {
            Some(running) => {
                running.fabrications += found.fabrications;
                running.unlocated_mints += found.unlocated_mints;
            }
            None => {
                per.insert(crate_name.to_owned(), found);
            }
        }
    }
    Ok(per)
}

/// The crate a `crates/<name>/...` path belongs to.
///
/// Borrowed from the path, and owned by the caller's map key. A first draft
/// returned `&'static str` by leaking the name of any crate absent from
/// [`CEILING`], which reads as a bounded one-off and is not: most crates carry
/// no fabrication at all, so they are all absent, and every file of every one
/// of them leaked a copy on every run of every probe.
fn crate_of(path: &str) -> Option<&str> {
    let within = path.strip_prefix("crates/")?;
    // A file directly under `crates/` belongs to no crate. `split('/').next()`
    // answered `Some("README.md")` for one, which was harmless while only Rust
    // files reached here and is not now that every file sights its crate.
    let end = within.find('/')?;
    Some(&within[..end])
}

/// Occurrences in the CODE of `source`: not in its comments, not in its
/// strings.
///
/// Comments are excluded because this counts a PRACTICE, and prose about the
/// practice is its opposite. Counting both is wrong in two directions:
/// documenting the hazard scores as committing it, and, worse, deleting a real
/// `Span::DUMMY` while adding a sentence about it leaves the total unmoved,
/// which is the masking direction a ratchet must never err in. The Python this
/// replaced counted comments until 2026-09-08 and fired for exactly that
/// reason, on a change whose only sin was six sentences explaining why the
/// sentinel is dangerous.
///
/// STRINGS are excluded for the same reason and for one more: this module names
/// both spellings in a `const` and writes them into probe fixtures, so counting
/// string content made the gate fail on its own source the moment it moved into
/// `crates/`. A gate that cannot be written without tripping itself is
/// measuring the wrong thing.
///
/// [`blank_literals`] does both, preserving length, and is the same blanking
/// the hygiene gates and `gate_discipline` use.
fn fabrications_in(source: &str) -> usize {
    let blanked = blank_literals(source);
    FABRICATIONS
        .iter()
        .map(|needle| blanked.matches(needle).count())
        .sum()
}

/// Whether a path holds tests by convention: an integration `tests/` tree or
/// a `tests.rs` / `_tests.rs` module file.
fn is_test_path(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("tests.rs")
}

/// The production part of a source file: everything before its first
/// `#[cfg(test)]`. Inline test modules sit at the end of a file in this
/// workspace, and the limit is declared in [`UNPROVEN`].
fn production_text(source: &str) -> &str {
    match source.find("#[cfg(test)]") {
        Some(end) => &source[..end],
        None => source,
    }
}

/// Occurrences of [`UNLOCATED_MINTS`] in the CODE of `source`, where the
/// character before the match is not part of an identifier: so
/// `crate::Location::Unlocated` and `talkbank_model::Location::Unlocated`
/// count and `DiagnosticLocation::Unlocated` does not.
fn unlocated_mints_in(source: &str) -> usize {
    let blanked = blank_literals(source);
    UNLOCATED_MINTS
        .iter()
        .flat_map(|needle| blanked.match_indices(needle))
        .filter(|(at, _)| {
            !blanked[..*at]
                .chars()
                .next_back()
                .is_some_and(|before| before.is_alphanumeric() || before == '_')
        })
        .count()
}

/// What no plant of this gate reaches.
static UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "SCOPE: everything outside `crates/`",
        "`spec/`, `apps/`, `xtask/` and the root `tests/` are never counted, \
         which the last must-pass probe pins as deliberate. It is a real limit \
         rather than a probe gap: a fabrication in `spec/tools` is invisible to \
         this ratchet.",
    ),
    UnprovenRule::new(
        "BLOCK COMMENTS, which `blank_literals` does not handle",
        "`/* new_unchecked */` counts, because `blank_literals` blanks `//` line \
         comments and string literals only. A plant of that shape WOULD make \
         the gate fire, as a false positive of the counter rather than of the \
         rule, so it is a bad probe. Three sibling gates declare the identical \
         limit.",
    ),
    UnprovenRule::new(
        "SPELLING: the unlocated-mint count is textual",
        "`UNLOCATED_MINTS` are matched as text. A `use talkbank_model::Location::Unlocated;` \
         followed by bare `Unlocated` counts once for the import however many \
         values it mints, and a consumer's `Location::Unlocated => ..` match arm \
         in parser production code counts as a mint although it mints nothing. \
         The first is a cheap evasion and the second forbids a legitimate match \
         in a crate held at zero; both are limits of a text scan, declared \
         rather than probed, because a probe of either would pass or fail for \
         the counter's reason, not the rule's.",
    ),
    UnprovenRule::new(
        "PLACEMENT of `#[cfg(test)]`, which ends the production text",
        "The unlocated-mint count reads a file up to its first `#[cfg(test)]`, \
         which is where this workspace puts inline test modules. A test module \
         ABOVE production code would hide the production text below it from \
         the count. A plant of that shape would pass, as a false negative of \
         the cut rather than of the rule, so it is not probed.",
    ),
];
