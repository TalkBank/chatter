//! Refuse to let fabricated-AST construction grow, per crate.
//!
//! # What this counts, and why those two spellings
//!
//! `new_unchecked` mints a model value without the check its safe constructor
//! performs. `Span::DUMMY` is `Span { start: 0, end: 0 }`, which is ALSO the
//! legal zero-width position at the first byte of a file, so a fabricated span
//! and a measured one at the start of a document are the same value. The type's
//! own comment concedes it, opening with "KNOWN HAZARD, for maintainers" and
//! admitting that "the VALUE is still overloaded, and that part is not fixed".
//! A paragraph explaining why something is safe is a work item with an address,
//! and this gate is the address.
//!
//! # Why a ratchet rather than a ban
//!
//! Both spellings are load-bearing today. `talkbank-model` declares no parser
//! dependency, so it cannot turn CHAT text into a model at all; every test in
//! it fabricates the AST it then judges, stating the raw text and the parsed
//! structure separately with nothing forcing the two to agree. A test that
//! fabricates its own input cannot be wrong about the format, only about
//! itself.
//!
//! The cure is a type change: make the AST constructible only from a parse
//! product, at which point that family does not get better tests, it stops
//! compiling and has to move onto spec-derived input. That is a large
//! migration. This ratchet is what makes it provable while it happens.
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
const FABRICATIONS: &[&str] = &["new_unchecked", "Span::DUMMY", "from_checked_literal"];

/// Per-crate ceiling.
///
/// A RATCHET. These may only go DOWN, lowered in the same commit that earned
/// it. A crate ABSENT from this list has a ceiling of ZERO, which is what stops
/// a new crate full of fabrication from arriving without any number moving.
const CEILING: &[(&str, usize)] = &[
    ("chatter", 1),
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
    // asserted a diagnostic landed at `Span::from_usize(0, 20)` beside a
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
    ("talkbank-model", 105),
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
    ("talkbank-parser", 6),
    // 23 -> 21 on 2026-09-09: the two tier labels the converter wrote for a
    // `%phoaln`/`%xphoint` line that would not parse are `non_empty_literal!`.
    ("talkbank-parser-re2c", 21),
    // 10 -> 9 and 12 -> 11 on 2026-09-08, the first two conversions this
    // ratchet was built to make provable: `test_complex_signature` and
    // `raw_text_rebuild_preserves_nonlexical_word_markers` now PARSE their
    // input instead of stating it twice.
    // 9 -> 6 the same day: the three hand-built words of the utterance
    // writer tests are `Word::new` over compile-time non-empty literals; the
    // three `Span::DUMMY` terminators stay, a hand-built terminator having no
    // source and the writer reading no span.
    ("talkbank-parser-tests", 6),
    // 11 -> 7 the same day: the sanitizer's placeholders are proven text
    // (`PlaceholderToken` holds a `NonEmptyString`, a literal prefix and
    // the index), handed to the word-text types through `From`; the
    // shortening placeholder and one test literal are `non_empty_literal!`.
    ("talkbank-transform", 7),
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
        let recorded: BTreeMap<&str, usize> = CEILING.iter().copied().collect();

        let mut grew: Vec<String> = Vec::new();
        let mut shrank: Vec<String> = Vec::new();
        let mut vanished: Vec<String> = Vec::new();
        for standing in standings(&counted, &recorded) {
            match standing {
                Standing::Listed {
                    crate_name,
                    now,
                    was,
                } => match now.cmp(&was) {
                    Ordering::Greater => {
                        grew.push(format!("{crate_name}: {was} -> {now}, {} added", now - was));
                    }
                    Ordering::Less => {
                        shrank.push(format!("(\"{crate_name}\", {now}),   // was {was}"));
                    }
                    Ordering::Equal => {}
                },
                Standing::Unlisted { crate_name, now } => {
                    if now > 0 {
                        grew.push(format!(
                            "{crate_name}: 0 -> {now}, {now} added \
                             (no ceiling row, so the ceiling is zero)"
                        ));
                    }
                }
                Standing::Vanished { crate_name, was } => {
                    vanished.push(format!(
                        "{crate_name}: the ceiling records {was}, and no file of that \
                         crate is in the tree"
                    ));
                }
            }
        }

        if grew.is_empty() && shrank.is_empty() && vanished.is_empty() {
            let total: usize = counted.values().sum();
            let carrying = counted.values().filter(|found| **found > 0).count();
            return tree.clean(format!(
                "fabricated-AST construction: {total} across {carrying} crate(s), at the \
                 ceiling"
            ));
        }
        Outcome::failed(report([
            listing(
                "FAIL: fabricated-AST construction GREW. A test that builds its own\n\
                 AST states the input twice, and nothing forces the raw text and the\n\
                 parsed structure to agree. Reach for a spec example, which is\n\
                 lowered to a real file and run through both stages, before reaching\n\
                 for another fabricated one:",
                &grew,
            ),
            listing(
                "FAIL: the ceiling now sits ABOVE reality, which leaves room for a new\n\
                 fabrication to arrive under cover of an unrelated deletion. Lower\n\
                 these in this commit:",
                &shrank,
            ),
            listing(
                "FAIL: the ceiling names a crate the walk never saw. A rename moves\n\
                 every one of its fabrications to a crate with no row, and a deletion\n\
                 is progress that has to be banked rather than left as headroom.\n\
                 Move the row or delete it:",
                &vanished,
            ),
        ]))
    }

    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a new fabrication in a crate already at its ceiling",
            "1 added",
            |edit| {
                edit.write(PROBE_FILE, "fn f() { let s = Span::DUMMY; }\n");
                Ok(())
            },
        )
        .refusing(
            "a crate ABSENT from the ceiling, which starts at zero",
            "talkbank-probe-new: 0 -> 1",
            |edit| {
                edit.write(
                    "crates/talkbank-probe-new/src/lib.rs",
                    "fn f() { let s = Span::DUMMY; }\n",
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
                edit.remove_dir("crates/chatter");
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
                    "// Never write Span::DUMMY here, and never new_unchecked.\n\
                     /// `Span::DUMMY` is the hazard this gate counts.\n",
                );
                Ok(())
            },
        )
        .accepting("a file outside `crates/` is out of scope", |edit| {
            edit.write("xtask/src/probe.rs", "fn f() { let s = Span::DUMMY; }\n");
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

/// The file every counting probe writes into.
///
/// Under a crate that already has a ceiling, so the probe that adds one moves
/// that crate's number rather than introducing a crate.
const PROBE_FILE: &str = "crates/talkbank-model/src/probe_fabricated_ast.rs";

/// Occurrences per crate, over the CODE of each file.
///
/// # Errors
///
/// When the walk fails or a file cannot be read. Either makes the count a floor
/// rather than a measurement, and a floor compared against a ceiling reports
/// progress that did not happen.
fn count_by_crate(tree: &ReadTree) -> Result<BTreeMap<String, usize>, String> {
    let files = tree
        .files_under("crates")
        .map_err(|err| format!("FAIL: could not enumerate the crates tree: {err}"))?;
    let mut per: BTreeMap<String, usize> = BTreeMap::new();
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
            fabrications_in(&text)
        } else {
            0
        };
        // Not `entry(crate_name.to_owned())`, which allocates the key on every
        // file to throw it away on all but the first. The name is only owned
        // when it is first seen.
        match per.get_mut(crate_name) {
            Some(running) => *running += found,
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
        "`/* Span::DUMMY */` counts, because `blank_literals` blanks `//` line \
         comments and string literals only. A plant of that shape WOULD make \
         the gate fire, as a false positive of the counter rather than of the \
         rule, so it is a bad probe. Three sibling gates declare the identical \
         limit.",
    ),
];
