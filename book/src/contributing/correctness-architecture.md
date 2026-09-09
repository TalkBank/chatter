# Correctness Architecture

**Status:** Current
**Last modified:** 2026-09-09 08:49 EDT

This is the target design of chatter's correctness machinery, written for the
maintainer who inherits it. It is not a patch list and not a description of the
tree as it stands today. Where the current tree disagrees with this page, the
tree is what has to move.

Read [Testing](testing.md) for what the layers are called today and
[Spec System](../architecture/spec-system.md) for what the spec fields mean.
This page says what the layers are FOR, which of them may be deleted, and how a
successor knows the suite is complete rather than merely green.

Every count on this page carries the command that produced it. The commands are
collected in [Appendix A](#appendix-a-how-every-number-here-was-produced) so
that a number which has drifted can be re-derived rather than believed.

## What the first night of execution established

Written after the first session of work against this plan, because several of
its numbers were wrong and the corrections are more useful than the originals.

**Tests can now be run.** The workspace guard permitted one named test at a
time, justified by measurements from a different repository entirely. Chatter's
whole suite is 31 seconds for 3,048 tests. A guard that prevents measurement
prevents the work it protects, and three of this page's own figures were wrong
because the measurement was unavailable.

**Dead snapshots: 368, then the last 56, and the gate is wired.** The
authoritative answer needs a run, and two independent witnesses: unreferenced
by the run, AND no test function of that name anywhere, or an exact duplicate
at another path, or a crate prefix renamed out of existence. One belonged to an
ignored test and was kept, which is why the second witness is not optional. The
suite proves what it did before and runs faster.

The 56 that survived that sweep were resolved on 2026-09-08 and
`snapshot-hygiene` is now in `gate`. Four families: 47 named `.cha` stems no
file in the repository has, from a corpus layout that was reorganised; 4
belonged to a `snapshot_tests` module that kept its name after being rewritten
to plain assertions; 4 were superseded copies left behind when a test target or
a module moved, with the live ones present under the new name; and 1 named a
test file that does not exist. The first pass at the second witness said eight
of them were live, and it was wrong: `fn pho_tier` matches the accessor
`pho_tier(` as well as a test of that name. A witness a different function can
satisfy is not a witness.

**Undemonstrated error codes: ten, not fifty-three.** The first count did not
read the status the registry already carries. `spec/codes/error-codes.toml`
declares one for every code, and three of its values legitimately have no
example: `not_implemented`, `deprecated`, and `unreachable_from_chat`, whose
documentation describes this exact situation better than the mechanism I had
started to build beside it. 219 codes carry a spec, 166 are demonstrated, 44
are excused, 10 remain, and five of the ten were closed the same night.

The sentence that stood here said two of the ten were named only in the
backend-parity baseline and wanted an adjudication rather than an example. That
was wrong, and how it was wrong is the useful part: the scan behind it searched
for the code NUMBER, while production code names a rule by its `ErrorCode`
VARIANT, and it searched comments, where the number is exactly what a
maintainer writes. Re-derived from the registry's own `variant` field over
non-comment code, every remaining undemonstrated code IS applied by production
code, so every one of them wants an example.

**The fabricated-AST population is two populations.** Of 708 constructions, 107
passed the same string twice and are now `Word::simple`, correct by
construction. (The 708 was taken with the comment-counting rule corrected two
paragraphs below, so it is an overcount of the same kind; the 107 conversions
were counted at their call sites and are unaffected. It is not re-measured
here, because re-measuring the past needs a worktree and the figure that
matters is the one the ratchet now holds.) Of the 66 that pass two different string literals, the shape is
always the same: a raw text carrying CHAT markers and a cleaned text without
them, stated independently, with nothing forcing the second to be what cleaning
the first produces. Worse, the constructor stores the cleaned string as a
single flat text element, where a parse of the same word would produce
structured content naming the marker. So those tests may assert on shapes the
parser cannot produce, which is this page's central claim, now concrete and
countable. They cannot be fixed in place: the crate cannot parse, so they have
to move to one that can.

**The grammar corpus is self-certifying in full, not in part.** This page said
the generator substitutes the parser's own output when a construct spec
declares no expected tree. Measured: ZERO of the 138 construct specs declare
one, and the branch that could set it is unreachable, since it fires only for a
`chat-file` or `document` input fence and no spec uses either. The field the
corpus test asserts is `full_cst`, the whole-document tree, so every GENERATED
corpus case expects exactly what the parser produced when the case was written.
A wrong grammar rule regenerates a wrong expectation and passes, for all 211 of
them. (Two numbers in this paragraph were wrong until 2026-09-08 and are
re-derived above: 139 specs and 233 cases. The 24 cases under
`grammar/test/corpus/manual/` are hand-authored and no generator touches them,
so "self-certifying" is true of the generated tree, not of the corpus.)

The human-authored expectation is not missing, it is disconnected, and it has
rotted while disconnected. 137 of the 138 specs carry a fragment `cst` block and
NOTHING asserts one: the accessor is reachable only through a dead branch, the
one function that would compare it has no caller, and the sole gate on the block
is a paren-balance check whose own doc comment says the block "is read only by
humans, and no human read it". Of those 137, **41 name a node type the grammar
does not have and 43 contain a literal `...` ellipsis**; one is nothing but
`(date_header ...)`.

So the obvious fix, "make the corpus assert the fragment the human wrote", is
not available as stated: a third of the authored blocks are not trees, and a
third name types that do not exist. The real options are to repair the blocks
first, or to make `full_cst` required and derive it once under review, which
buys review rather than authorship. Either way the change is in
`spec/tools/src/output/tree_sitter.rs` around the substitution, it regenerates
all 211 cases, and it is a decision for daylight rather than a night shift.

**The remainder is not evenly distributed.** 485 of the 547 remaining
constructions are in `talkbank-model`. That is the crate without a parser
dependency, and the concentration is not a coincidence but the consequence.

Those two figures read 505 and 601 for a day, and the correction is worth more
than the numbers. The ratchet counted COMMENTS, so prose about the hazard
scored as the hazard, and it fired on 2026-09-08 against a change whose only
sin was six sentences explaining why `Span::DUMMY` is dangerous. Wrong in two
directions, and the second is the defect: deleting a real call while adding a
sentence about it left the total unmoved, which is the masking direction a
ratchet must never err in. 54 of the 601 were prose. Nobody removed anything;
the measurement got right.

## What the review of the first mechanism established

The gate-probe mechanism was written, then reviewed from five angles before it
was committed. Both properties it advertised were reachable around, and the two
findings are worth more than the fixes.

**A clean verdict could be composed by its author.** `Outcome::Clean(String,
Examined)` looked safe because `Examined` had one private constructor. Variants
of a `pub` enum are constructible wherever the enum is visible, so the tuple
variant WAS a public constructor pairing any summary with any witness: a gate
could ignore the tree it was handed, read one file through a tree of its own,
and report clean about a checkout it never opened, with every probe green
because the plant lived in the discarded parameter. The module doc said no such
constructor existed. **A tuple variant of a public enum is a public constructor
of whatever it holds**, and that sentence is the general form.

**A suite of one control probe satisfied every check.** `probes()` having no
default body forces an author to say how a gate can fail; it does not force
them to say anything true. `vec![Probe::control()]` planted nothing, satisfied
the "a gate that rejects everything" check because a control IS a `MustPass`,
and printed "1 probe(s) ... every planted violation was rejected". That is this
project's own bug class inside the mechanism built to close it, at the one
place a new gate's author works. `ProbeSuite`'s constructor takes the first
refusal as arguments and adds the control itself, so both vacuous suites are
now unconstructible and two runtime checks are gone.

**A three-value axis had one reachable value.** Every `Precondition` declared
`PrePush`, the default run tier was `PrePush`, and nothing set
`CHATTER_GATE_TIER`, so the promise that a contributor with a filtered clone
hears "not judged" rather than a defect report was true of no configuration.
`just test` runs at `inner-loop` now, the comparison is a `Precondition::at`
returning `Unmet::{Skip, Fail}` rather than an `if` on a bool inside a test,
and a test asserts the two tiers decide differently over a real precondition.

**An absent directory minted the same witness as an empty one**, so a gate
whose whole scope had been removed reported clean over nothing, with evidence.

**Both Python ratchets became gates.** `test_hygiene`'s module doc argues that
a check written as Python under `scripts/` is at the wrong altitude twice over,
and the first half of the night added two such scripts anyway. Both are gates
now and six files went with them; `scripts/lint/` holds no ratchet at all.

Each conversion found a defect in the script it replaced. The fabricated-AST
count had to stop counting STRING LITERALS as well as comments, because the
gate names both spellings in a `const` and writes them into probe fixtures, so
counting string content made it fail on its own source the moment it moved into
`crates/`; `talkbank-model` went 485 to 484 and nobody removed anything. The
demonstration gate's declared-codes predicate was `looks_like_a_code`, which
reads two characters because its job is to separate a spec from a `README.md`,
and answered yes to `E202_missing_form_type`, a spec that DOCUMENTS E202 rather
than being its file.

**A gate could open a second tree, and now cannot.** `Tree::live` is public and
`pub(crate)` is no barrier inside the crate, so a `check` that ignored its
parameter, read one file through a tree of its own and called `clean` composed
a clean verdict about a checkout it was never handed, with every probe green
because the plant lived in the discarded parameter. No type expresses "do not
open a second tree". `gate_discipline` is a registered gate that reads the `fn
check` body of every file declaring `impl Gate for` and refuses the calls that
reach a checkout directly, scoped to those bodies so a helper in the same file
may still build its own tree. Watched: with `Tree::live()` planted into the
golden-word gate's real `check`, it names the file and the call.

**What a final review found, and both were mine.** The whole night's work was
reviewed once more before being handed over. Two defects were serious enough
that neither should have reached a maintainer, and both are worth naming
because neither was a slip of attention.

Making the re2c `%gra` lowering fallible gave three FRAGMENT entry points
something to report for the first time, and all three passed the caller's raw
sink into it. `rebase` moves the model; a diagnostic already handed to a sink is
past moving. So a `%gra` head overflow reported at byte 2 of the fragment where
the canonical backend reported it at the caller's offset. The contract test
that pins this for headers could not see it, because its `%gra` case is a
relation that emits nothing.

And `just gate` ran the gate suite at the INNER-LOOP tier, because `test-all`
depends on `test` and the tier was exported from `test`. An absent input
printed as a skip and the pre-push gate stayed green: a gate that can skip
itself, inside the mechanism written the same night to forbid it, and weaker
than CI, which sets nothing and so runs strict. The tier is a recipe ARGUMENT
now, named by the caller.

**A walk failure became something a probe can plant.** Five gates each declared
their walk-failure half unprobeable, in five sets of words, all saying that a
failing `read_dir` needs a permission change and no content overlay can express
one. That was a statement about the overlay rather than about the world: it
could already say "empty" and "gone", and the third thing a walk can do had no
coverage at all. One more set on the tree and one `TreeEdit` method closed five
record entries at once, which is the largest reduction that list has had.

**Three findings the mechanism cannot reach, stated so the next reader does not
have to find them again.** A probe's plant and its expected message are both
its author's invention, so a green suite certifies the author's understanding
of the rule, not the rule. The unproven-rule record is two hand-written lists
reconciled by a test, which catches drift between two files and is NOT the
ratchet-against-reality that `UNPROTECTED` is; deriving it needs a per-gate
vocabulary of rule identifiers that a probe and an unproven entry each name, so
the unproven set becomes `rules() - probed()`. And the mechanism is general
repository infrastructure living in a CHAT test-support crate, which is why
`talkbank-parser-re2c`'s parity gate cannot register in `ALL` and keeps a
degraded two-case copy of the shape.

**The cost, measured against the loop rather than against itself.** The probe
run was 5.3 seconds of a 13.7 second `just test`, so the mechanism that proves
the gates can fail had nearly doubled the loop it protects. Threads took the
standalone binary to 1.2 seconds, 4.5 times faster with a byte-identical report
across three runs, and took `just test` to 16.6 seconds with system time from
27 seconds to 2 minutes 30, because `cargo test` already runs the test binaries
in parallel. That was reverted, and it is the lesson worth keeping: a speedup of
the piece is not a speedup of the loop, and only timing the loop says which you
have.

A shared read cache ships instead and the loop is 12.0 seconds. Its own verdict
is honest: system time 0.57 to 0.14 seconds, wall clock 5.3 to 5.0 standalone,
so the reads were never the cost. Three micro-fixes (a conditional separator
rewrite, an allocation-free `is_under`, a borrowed blanked span) bought about
2%. The one lever left is memoizing the DERIVED per-file artifact the way the
content is now memoized, chiefly the blanked source each hygiene probe rebuilds
over about 1,100 files, which needs a key a planted file invalidates.

Commands behind the numbers in this section, on 2026-09-08: the probe and gate
counts are the last line of `cargo run -p talkbank-parser-tests --bin
audit_gate_probes`; the file count is `rg --files crates -g '*.rs' | grep -v
/generated/ | grep -v /target/ | wc -l`; the two timings are `time` around that
binary and around `cargo test -p talkbank-parser-tests --test integration
gates::every_registered_gate_passes`, both on a warm dev build.

## The thesis

1. **The spec system is the single normative source** of what CHAT is and what
   chatter must do. A claim about correctness that is not written in `spec/` is
   not a claim chatter makes.
2. **Everything else is derived from it, or is a property no example can
   express, or is deleted.** There is no fourth category. A test that is none
   of those three is volume, and volume is not evidence.
3. **Coverage is the completeness proof.** An unreached branch is either a
   missing spec example or dead code. Those are the only two verdicts, and each
   has a named consequence.
4. **Nothing depends on private data.** A successor who can read only this
   repository can run every gate, add every kind of evidence, and adjudicate
   every failure. The production corpus is read for evidence about which paths
   matter; its findings land as committed fixtures, and then it is not needed
   again.

## The answers, in one screen

**What proves what.** Six evidence classes, and nothing outside them survives:
spec examples (normative), oracles (two parsers, and CLAN CHECK), properties
(universally quantified), boundary tests (subprocess, stdio, cross-process),
algorithm units (behaviour no type can hold), and derived artifacts (which
prove nothing and are the mechanism). Section
[What proves what](#what-proves-what-six-classes-and-nothing-else) is the
table.

**Where to add evidence when you fix a bug.** A wrong verdict on CHAT input is
a `[[example]]` in `spec/errors/`. A wrong parse shape is a construct spec with
a model claim. An invariant over all inputs is a property test with a real case
budget. A bug that only appears through the CLI, the LSP or the desktop app is
a boundary test in that crate. A disagreement between the two parsers shrinks
`KNOWN_DIVERGENCES`. See
[Where evidence goes](#where-evidence-goes-when-you-fix-a-bug).

**What you may delete.** 335 orphaned snapshot files, a duplicated roundtrip
harness, a second error corpus at the repository root, a hand-mirrored CLI
command list, 137 rotted CST blocks, and roughly 210 unit tests that a type
change makes unwritable. Sizes and receipts in
[What is deleted](#what-is-deleted-with-sizes).

**How you know the suite is complete rather than green.** Three questions, three
instruments: did anything RUN the code (coverage attribution), did anything
OBSERVE what it did (mutation, scoped), and is what it does RIGHT (the oracles,
and human adjudication against the format authority). The first two are gated.
The third cannot be, and
[What the criterion cannot cover](#what-the-criterion-cannot-cover) says so
plainly.

## The one architectural fact that explains the current shape

`talkbank-model` cannot parse a CHAT file.

Its `Cargo.toml` declares no parser dependency; its dev-dependencies are
`insta`, `proptest` and `tokio`. It holds 671 of the repository's 2,868 test
attributes, 23% of the suite, and not one of those tests can turn CHAT text
into a model. Every one of them fabricates the AST it then judges. Repository
wide there are 288 `new_unchecked` call sites and 403 `Span::DUMMY`
constructions, the large majority of both inside that crate.

`Span::DUMMY` is `Span { start: 0, end: 0 }`, which is also the legal
zero-width position at the first byte of a file. The type already carries a
long comment about this, opening with `KNOWN HAZARD, for maintainers` and
conceding that "the VALUE is still overloaded, and that part is not fixed".
That comment is a receipt: prose is gated by nothing, so a paragraph explaining
why something is safe is a work item with an address, not a mitigation.

Three consequences follow from that single fact, and together they explain the
shape of everything else:

- **The spec corpus cannot reach the validation rules.** 436 spec examples are
  lowered into real `.cha` files and run through both stages, and they are the
  best evidence in the tree. But a rule that only fires on a shape the parser
  never produces cannot be demonstrated by any file, so those rules were
  demonstrated by hand-built models instead, in the crate that cannot parse.
  `spec/errors/E232.md` says this in its own words: the parser never constructs
  such a word from real input.
- **The test states the input twice.** A hand-built test writes the raw text in
  a comment or a string, and the parsed structure separately, and nothing forces
  the two to agree. A test that fabricates its own input cannot be wrong about
  the format, only about itself.
- **Volume accumulated as compensation.** 679 committed snapshot files totalling
  4.36 MB, of which 335 name a reference-corpus stem that does not exist; a roundtrip gate
  implemented twice over the same 107 files; and a second error corpus at
  `tests/error_corpus/`, 26 tracked files, whose generator walked one parent
  too many and had been writing a full corpus BESIDE the repository,
  touching nothing tracked. Confirmed by the 66 files it had left there.
  Fixed 2026-09-07; the duplication with the spec-derived corpus remains.

The redesign therefore begins with a type change, not a test cull. Make the AST
constructible only from a parse product, and the 671-test family stops
compiling. It does not get better tests; it stops being writable, and it has to
move onto spec-derived input, which is the permanent deliverable anyway.

## What proves what: six classes, and nothing else

| Class | What only this class can prove | Where | Size today | Who may add |
|---|---|---|---|---|
| **SPEC (normative)** | That a stated input violates, or does not violate, a stated rule. The only artifact a successor with no corpus can read and act on. | `spec/errors/` (223 specs, 436 examples), `spec/constructs/` (138 specs) | 436 lowered fixtures, one data-driven runner | Anyone. This is the default destination for new evidence. |
| **ORACLE** | That a defect exists in ONE implementation without a human having stated the right answer. | `talkbank-parser-re2c` differential (48 known divergences), `check_parity` (164 manifest entries, 146 fixtures) | 2 gating tests, 2 CLAN-gated | Only by shrinking a baseline, never by adding a hand-typed entry. |
| **PROPERTY** | A statement quantified over all inputs: never panics, roundtrip, span arithmetic, cleaned-text invariants. The spec format has no quantifier and never will. | `talkbank-parser-tests/tests/integration/property_tests/` | 27 files, 18 `proptest!` blocks | Anyone, at a real case budget (see below). |
| **BOUNDARY** | Behaviour of the actual seam: argv, exit codes, stdout contracts, LSP stdio lifecycle, Tauri async runtime, cross-process cache. No type of ours reaches the outside world. | `crates/chatter/tests/integration` (269 attrs, 136 spawns), `talkbank-lsp` stdio tests, `apps/chatter-desktop/src-tauri/tests` | 147 of 2,868 attrs (5.1%) reach a subprocess or runtime | Anyone. This class should GROW. |
| **ALGORITHM UNIT** | Behaviour no signature describes and no type can hold: number-to-words, POS mapping. | `talkbank-transform/src/num_words/`, `clan_ud_mapping.rs` | 65 attrs | Anyone, labelled in source with the category. |
| **DERIVED ARTIFACT** | Nothing. These are the mechanism, not the evidence: lowered fixtures, generated tests, the observation snapshot, the corpus sexp pins. | the seven registry artifacts | about 880 files written by one command | Nobody by hand. A generator owns every byte. |

There is a seventh class, and it is transitional by design:

| Class | Rule |
|---|---|
| **WILD-CORPUS ORACLE** | The `#[ignore]`d differential over the production corpus (`$TALKBANK_DATA`) is an evidence-producing PASS, not a gate. It is run deliberately; its output must land as committed spec examples and sanitized fixtures; then the ignored tests are deleted with it. Evidence produced and discarded is why the model crate still fabricates its own inputs. |

**The rule that makes this a design and not a taxonomy: a test that is not in
one of these classes is deleted.** Not deprecated, not annotated, deleted. If
deleting it feels wrong, it belongs to a class and the class is the place to say
so, in one line, in the source.

## The normative core

### What a spec example can say today

| Property | How |
|---|---|
| "this input violates rule X" | `claim = 'violates'` |
| "this input does NOT violate rule X" | `claim = 'legal'` (96 examples) |
| "this violates X but chatter reports Y today" | `claim = { subsumed_by = [...] }` (42 examples), positive and negative halves both checked |
| "the grammar produces exactly this tree" | a tree-sitter corpus case |
| "this parses with zero diagnostics" | a construct spec |
| "which stage catches this" | observed per example in `spec/observations/example-diagnostics.json` |
| "this input roundtrips byte-exact" | the snapshot's `roundtrip` field, byte-gated, so a flip fails the currency gate |

The meaning of a claim has exactly one owner, `Claim::satisfied_by`, and both
runners call it. Keep that. It is the single best structural property of the
system, alongside the artifact registry.

### What it cannot say, and what the redesign adds

It cannot state a property over all inputs, an idempotence, a preservation
invariant other than the whole-file roundtrip, a performance bound, a claim
about the parsed MODEL, a claim about the exact code SET, or anything about the
CLI, LSP, transform layer or desktop app. Most of those stay outside forever;
that is what the PROPERTY and BOUNDARY classes are for.

Three of them must move inside, because without them the spec cannot carry the
completeness criterion:

**1. The exact per-stage code set becomes claimable.** Today, extra codes always
pass: the claim is set membership, so "E316 fired instead of the rule you meant"
is a recorded observation rather than a failing state. The snapshot already
holds the exact per-stage sets. Promote them from observation to claim wherever
a human has adjudicated them, and leave the rest observed. This is the smallest
change that makes a spec example a statement about chatter's behaviour rather
than about one bit of it.

**2. A construct spec claims a MODEL, not a CST.** The authored
`## Expected CST` block is dead data and has rotted: 137 of the 138 construct
examples carry one, and 41 of them name at least one node type the grammar does
not have (25 distinct dead names, led by `initial_word_segment` in 18 files).
The generator ignores the block entirely and substitutes the parser's own
`to_sexp()`, so the committed expectation is a recording of what the parser did.

Two repairs were available and they conflict. Making the authored CST normative
is the wrong one: a CST is a statement about the grammar's internal node names,
which change legitimately under refactoring, and 41 rotted blocks are the
receipt for exactly that. The model is the published contract; it has a JSON
schema, and that schema is already gated. So:

- the `## Expected CST` block, the unread `## Metadata` section, `update_cst`
  (a library function that writes into a human-authored spec file, bypassing the
  `.human-authored` marker, one call away from being re-enabled) and the second
  format reference are DELETED;
- a construct spec gains a declared model projection, which the generated test
  asserts, replacing the 138 bodies that today parse a string and discard the
  result;
- the sexp corpus stays and is relabelled honestly as a derived regression pin.
  It catches an accidental grammar change at the node level, which nothing else
  does. It is not a specification and must stop being described as one.

**3. An example and an emit site name the same rule.** Codes are emitted from
many branches: 670 `ErrorCode::` references over 214 distinct variants, 133 of
them referenced more than once, and `ErrorCode::TreeParsingError` (E316) alone
appearing 83 times. A per-code example demonstrates that a code CAN fire, never
that a given site can. Until an emit site and an example can name the same
identifier, "every reachable branch" is checkable only by instrumentation, never
by the spec. Adding that identifier is what lets the two meet.

### Two smaller repairs that the criterion depends on

- **Derive `status` from the snapshot.** It is authored today, and the format
  reference admits it. The system already knows, per example, whether a code
  fires. Leave only the genuine adjudications (`deprecated`,
  `unreachable_from_chat`) to a human.
- **Make the drop-outs visible.** 117 examples qualify for the tree-sitter error
  corpus and 71 files exist; the 46 that fall out are announced as generation-time
  stderr and leave no committed trace. They become a committed, gated list with a
  reason per entry, in the shape `node_coverage.rs` already uses, where an entry
  that becomes covered FAILS so the list cannot rot.

## The completeness criterion

### Stated mechanically

> **For every branch in hand-written, non-generated code, the attribution
> artifact carries a row, and every row carries a verdict from a closed set. A
> row with no verdict fails the gate. A row whose verdict says it is unreachable
> and which is then covered fails the gate. The count of rows awaiting a spec
> example may only go down.**

The gate is the VERDICT, not a percentage. That distinction is the whole design.
A coverage percentage as a gate is a number to be gamed; a verdict is a
statement a human made, with a reason, that a later measurement can contradict.

### The instrument

```bash
# Branch coverage needs nightly; the pinned toolchain gives regions only.
cargo +nightly llvm-cov --branch -p <crate> --lib --json --output-path <out>.json
```

Region coverage is a good proxy for match-arm coverage, because each arm body is
its own region. It is a bad proxy for two shapes that are everywhere in a
validator: `if cond { emit(...) }` with no `else`, where the not-taken path is
not a region at all, and `a && b`, where short-circuit operands get no separate
region. "The rule never declined to fire" is precisely the state this redesign
wants to detect, so the criterion runs on `--branch` and the toolchain choice is
a measurement decision, not a CI decision.

Generated code is excluded, and the list of what is generated has ONE owner. It
does not have one today: `mutants.toml` excludes exactly one generated file with
a header explaining why (a mutant there indicts the generator), and that
reasoning applies verbatim to ten other committed generated files it does not
exclude. Derive both the coverage `--ignore-filename-regex` and
`mutants.toml`'s `exclude_globs` from the artifact registry, so a new generated
artifact cannot be excluded from one instrument and measured by the other.

The exclusion is not cosmetic. `crates/talkbank-parser/src/generated_traversal.rs`
alone is 45,284 lines, 24,549 instrumented lines and 3,924 branches, more than
double the entire hand-written parser, at 18.1% line and 19.4% branch coverage.
Including it would drag the parser's reported figure down by about ten points
and every finding in the report would be a statement about the generator.

### The row

One JSON file, one row per branch, plus a rendered page:

```json
{
  "file": "crates/talkbank-model/src/validation/utterance/spacing.rs",
  "line": 214, "column": 21,
  "region_kind": "branch",
  "function": "...::check_separator_spacing",
  "function_regions_uncovered": 1,
  "function_region_total": 31,
  "covered_by": [],
  "witnesses": [],
  "verdict": "NEEDS_SPEC_EXAMPLE",
  "verdict_detail": "no example whose separator is trailing"
}
```

`function` comes from the export's function-to-region nesting, so it is an exact
lookup rather than a line-range guess, and it resolves correctly through the
`include!`d generated test bodies. `function_regions_uncovered` over
`function_region_total` is the delete-versus-fixture discriminator, computed
rather than judged: a ratio of 1.0 is a delete candidate, one uncovered arm out
of thirty is a fixture candidate. Sorting by that ratio surfaces the deletions
first, which is the direction this redesign wants.

The verdict set is closed:

| Verdict | Meaning | Consequence |
|---|---|---|
| `REACHED_BY_SPEC` | a spec example reaches it | none, this is the goal |
| `NEEDS_SPEC_EXAMPLE` | reachable from CHAT, nothing reaches it | write the example; this count ratchets down |
| `NEEDS_PROPERTY` | quantified, no single example expresses it | write the property |
| `UNREACHABLE_FROM_CHAT` | no CHAT input reaches it | delete the code |
| `UNREACHABLE_BY_TYPE` | the type already forbids the state | delete the arm, or the type is wrong |
| `REACHED_ONLY_BY_WILD_DATA` | only the production corpus reaches it | synthesize a fixture, then re-verdict |
| `COVERED_ONLY_BY_FABRICATION` | reached, but only by a test that hand-built its input | not coverage; delete the test or convert it |
| `OUT_OF_SCOPE_GENERATED` | generated code | excluded by the registry, never hand-marked |

### Coverage has a PROVENANCE, and fabricated coverage counts as uncovered

**The maintainer, 2026-09-08: "Fake useless tests that fabricate are
particularly dangerous and could inflate coverage numbers."** This is the
sharpest constraint on the whole criterion and it was missing from it.

A test that fabricates its input still EXECUTES the code beneath it, so it
covers branches. chatter has hundreds: `talkbank-model` declares no parser
dependency, so every test in it hand-builds the AST it then judges. Coverage
bought that way is worse than no coverage, because it converts "nobody has shown
this rule firing on a real CHAT file" into a green number, and the number is the
thing a successor will trust.

So a covered region is not a fact until you know WHAT covered it:

- **Parse-backed**: reached by a spec example, a fixture or a reference file,
  through a real parse. This is coverage in the sense the criterion means.
- **Fabrication-backed**: reached only by a hand-built model. The rule ran;
  nothing showed it firing on CHAT. Counts as UNCOVERED for the criterion, and
  the test is a deletion or conversion candidate rather than an asset.

The split is measurable, and `scripts/coverage_attribution.py` measures it, but
only with THREE exports: the full suite, the fabricating tests alone, and the
suite with the fabricating tests excluded from the run but not from the report
(`--exclude-from-test`). Two exports give an upper bound only, because the full
run is a superset of the fabricating one and no subtraction between them
isolates anything. Saying so is the point: the bound is honest and the exact
figure has a price.

### Pruning is on the table, and it is half the work

**The maintainer, 2026-09-08: "Make sure that radical reorganization and
pruning of tests is on the table."** The criterion above reads as a gap-filling
machine,
and read that way it can only make the suite bigger. It is equally a DELETION
instrument, and the deletions are the cheaper half:

- A function with a ratio of 1.00 is code nothing runs. Delete the code, and its
  tests go with it.
- A region that is fabrication-backed only is a test that proves nothing about
  CHAT. Convert it to a parse, or delete it.
- A test whose parse-backed coverage is a subset of another's, and which pins no
  policy of its own, is redundant. The suite is not better for having it.
- `UNREACHABLE_FROM_CHAT` and `UNREACHABLE_BY_TYPE` are already deletion
  verdicts in the table above; they were written as consequences for CODE and
  they apply to the tests that reach that code too.

The standing rule that a test a type could obsolete should not exist is the same
instruction from the other end. Neither a count of tests nor a coverage
percentage is a goal; both go DOWN in a good week.

The repository already trusts this exact shape twice: `node_coverage.rs` splits
its exclusions into `INVALID_BY_CONSTRUCTION` and `NOT_YET_IN_CORPUS` and argues
at length that an exclusion has a KIND which implies a check a flat list could
not express; `construct_coverage.rs` does the same for parent-child pairs, where
an entry that becomes covered fails. Follow both, in both directions.

### Where it stands today, honestly

**Measured 2026-09-08 over the WHOLE SUITE, every crate instrumented, generated
files excluded, and split by what BOUGHT the coverage.** The command set is in
the appendix; the third run is what makes the split exact rather than bounded.

| Tree | Regions | Reported | **Parse-backed** | Fake |
|---|---|---|---|---|
| `talkbank-model/src/validation` | 9,455 | 89.2% | **47.7%** | 3,923 |
| `talkbank-model/src` | 38,881 | 83.8% | **43.0%** | 15,859 |
| `talkbank-parser/src` | 21,229 | 69.3% | 68.8% | 110 |
| `talkbank-transform/src` | 13,506 | 89.5% | 89.5% | 0 |

Read the third column, not the second. **The validator reports 89.2% and less
than half of it is backed by parsing CHAT**; for the model as a whole, 15,859
covered regions are reached only by tests that hand-build the AST they judge.
The parser and the transforms are almost entirely real, and the reason is
structural rather than cultural: those crates depend on a parser and
`talkbank-model` does not, so its tests cannot parse even when their authors
would prefer to.

That is the single largest fact about this repository's test suite, and it was
invisible until the split was measured: the reported number was the one being
improved.

The rows below are the earlier `--lib` figures, kept because they are what the
two-crate command produces and somebody will run it again:

| Tree | Lines | Regions | Branches | Functions |
|---|---|---|---|---|
| `talkbank-model/src/validation/` | 3121/5420 (57.6%) | 59.1% | **263/680 (38.7%)** | 292/393 (74.3%) |
| `talkbank-parser/src/`, hand-written | 4281/12295 (34.8%) | 34.2% | **478/1484 (32.2%)** | 369/691 (53.4%) |

**These are FLOORS, not the suite's coverage.** They were produced by `--lib`
runs of two crates. The spec-derived evidence (436 error fixtures, 138 construct
tests, 107 reference files) lives in `talkbank-parser-tests` integration
binaries, which were not in these runs; `talkbank-transform`, `talkbank-lsp` and
`chatter` were not measured at all. Do not quote these as chatter's coverage.
The real figure needs
`cargo +nightly llvm-cov --branch -p talkbank-parser-tests --tests`, and the
first job of the attribution harness is to produce it.

The uncovered-branch report already exists in draft form: 501 rows for the
validator and 1,078 for the parser. Their split is the useful part:

| Tree | neither side taken | false-only | true-only |
|---|---|---|---|
| validation | 401 | 51 | 49 |
| parser | 698 | 200 | 180 |

The 100 partial rows in the validator are the decidable ones, where the guard
fired but never declined or the reverse, and they are the immediate
`NEEDS_SPEC_EXAMPLE` worklist. The worst files by uncovered regions were
`validation/utterance/phon_xtier.rs` (333), `validation/retrace/rendering/bracketed.rs`
(199, 0% branch), `validation/header/structure.rs` (185),
`validation/retrace/rendering/utterance.rs` (172, 0%) and
`validation/header/checkers.rs` (161, 0%). Five functions had a ratio of 1.00
over more than 70 regions each, which means five functions nothing ran. The
two `retrace/rendering` files were a second serializer of the main tier that
E370 used to locate its marker, wrong on non-canonical spacing; on
2026-09-08 the parser started recording the marker's own span
(`Retrace::marker_span`) and the renderer was deleted.

### The precondition, and the deletion engine it unlocks

`validation_errors_detected` runs all 436 fixtures inside ONE test function. As
a class it attributes fine; per fixture it cannot, and "which spec example
reaches this branch" is the question the criterion asks. Converting it to one
`rstest` case per manifest fixture is about twenty lines, and the pattern
already exists in the same crate (`reference_corpus_parses.rs` uses
`#[rstest] #[files(...)]`).

That change unlocks the most valuable output of the whole exercise, which is not
the uncovered list at all. With per-fixture attribution you run a greedy set
cover over the 436 error fixtures and the 107 reference files and rank each by
MARGINAL branches contributed. **Every fixture contributing zero marginal
branches is a deletion candidate.** That is "volume is not evidence" made
mechanical, and it is the pressure that stops the spec corpus growing without
improving.

The six evidence classes run as six passes over ONE instrumented build:
`cargo llvm-cov show-env`, one `--no-run` build, then each class's binaries with
its own `LLVM_PROFILE_FILE` and libtest filter, then one merge and export per
class. Because every class runs the same binaries, the region tables are
identical and the six exports join on region identity. The join result is one
bitmask per branch, and that bitmask IS the attribution.

One honesty note on the property class: proptest is nondeterministic across runs
unless seeded. Run it with a pinned seed and case count and label the column
with the seed, so the claim is falsifiable rather than a lucky draw.

## What the criterion cannot cover

A claim of total coverage is the failure this exercise exists to correct, so
this section is not a caveat, it is part of the design.

**A region executed is not a region observed.** This is the sharpest limit here
and it is structural. The primary spec-derived gate over the entire validator
asserts set membership of stringified codes; nothing reads a span, a message, a
severity or a multiplicity. So across the validator's functions and all 436
fixtures, a mutation that moves a diagnostic's span from the offending word to
the whole utterance, or emits it twice, or flips Error to Warning, leaves every
branch covered and the suite green. **The parser's output is byte-pinned; the
validator's output is set-membership-pinned.** That asymmetry is where the
mutation budget goes:

```bash
cargo mutants -p talkbank-model --file 'src/validation/**' --timeout 180
```

Its survivor list is directly actionable: a mutant that makes a guard
unconditionally true turns a rule into "always fires", and it survives exactly
when the code has no `legal` example. The survivors are therefore a worklist of
codes lacking a negative example, and the fix is a spec example rather than a
test. `Claim::satisfied_by` itself is the single highest-value mutation target
in the tree, three arms judging 436 fixtures, and nothing mutates it today
because it lives in the excluded spec workspace.

**Coverage cannot see a false positive on an input nobody wrote.** `legal` is
per example. There is no statement anywhere of the form "this rule fires on no
reference-corpus file", and no coverage measurement can produce one.

**Branch coverage is condition-level, not MC/DC.** `a && b` yields two
conditions, not four combinations.

**Coverage cannot distinguish an interesting value from a degenerate one.** In a
parser this bites at length and index boundaries: a span-arithmetic branch
reached by a one-word utterance says nothing about an empty one.

**Coverage cannot see the spec being wrong.** This is the deep one, and the
[objection](#the-strongest-objection-and-the-honest-answer) below is built on
it. A defect both parsers share is invisible to the differential; a rule that
encodes a wrong understanding of CHAT is invisible to everything in this
repository. There is no gold reference anywhere: not the reference corpus, not
CLAN CHECK, not a recorded verdict. Every comparison against a human artifact
is AGREEMENT, not accuracy, and its ceiling is that artifact's own reliability.

**Coverage says nothing about the desktop or the LSP** beyond the fact that
their code was executed, which is why the boundary class exists and why it is
the one class instructed to grow.

## Type changes that delete tests

The compiler is the best failing test that exists: red before the change and
green after, at every call site including the ones no test enumerates, failing
at the mistake instead of later. Each row below is a change that removes a
possible wrong value AND deletes tests, with the count.

| # | Change | Made unrepresentable | Tests deleted | What breaks |
|---|---|---|---|---|
| 1 | `Word` gets a structured body (segments joined by compound and clitic boundaries, at most one primary stress per segment, shortenings as balanced pairs); `Word::new_unchecked` becomes `#[cfg(test)]` and the fields go private | leading and trailing `+`, `++`, misplaced stress and lengthening, secondary stress without primary, empty content, unbalanced shortening | **36** (34 in `validation/word/{tests,snapshot_tests}.rs`, 2 parser regressions) | 147 fabricating test sites; both parsers' word builders move from mutate-then-patch to one fallible constructor; 11 error codes leave the model layer and become parse diagnostics; the `WordContents` wire format changes |
| 2 | Fragment offsets become three newtypes (`DocumentOffset`, `SyntheticOffset`, `FragmentOffset`) with the wrapper owning the only conversion, and a clamp that returns `AsGiven` or `ClampedTo` | rebasing a span into the wrong space, omitting the rebase, a silent clamp | **16** (all of `context_public_api.rs`) | four public `parse_*_fragment` signatures, and their LSP and CLI callers |
| 3 | The header preamble becomes typed slots plus nested `GemScope`s built once at the boundary | duplicate single-only headers, missing required headers, wrong header order, unmatched and mismatched gems | **35** (24 + 11 in `validation/header/`) | `ChatFile::validate*` takes a preamble instead of scraping lines, which forces row 4 |
| 4 | `ChatFile` drops its four cached header-derived fields and `pub lines` | a cached `languages`/`options` drifting from the lines they came from; a JSON roundtrip that omits them silently yielding `ca_mode = false` on a CA file | few directly | 23 `ChatFile::new` sites and every consumer reading the cached fields; deliberate wire-format change |
| 5 | `GrammaticalRelation` stores `SemanticWordIndex1` and `GraHeadRef`, and `Vec<_>` becomes a validated `DependencyForest` | index 0, dangling head, cycles, multiple roots, non-sequential indices | **8-12** | 98 `GrammaticalRelation::new` sites. Receipt for the shape: the typed `index_as_semantic()` accessor is called ZERO times today while `rel.head` is read raw 29 times, which is a proof type built by its consumers and never by its producer |
| 6 | `ParseHealth` gains one `Alignment` enum with `fn tiers()` and an `AlignPermit`, replacing twenty hand-written mirrored predicates | forgetting an alignment gate; `ParseHealthState::default()` meaning "unknown" invisibly | **6** | 18 call sites |
| 7 | `ValidationContext` gets one `FieldContext` and one `DeclaredLanguages`, and the byte-duplicate `get_other_language` / `is_tertiary_language` pair is deleted | three of four field Options set and the fourth not; primary/secondary/tertiary decoded by list position; a dropped `@Options: CA` validating as non-CA | **~8** | the six helpers that rebuild the pair by hand |
| 8 | `Span` gains provenance (`Source(SourceSpan)` or `Synthesized`), with no `Default` and no numeric sentinel; `SourceLocation`'s two late-filled Options become a phase type `fn locate(SourceIndex) -> LocatedError` | a fabricated span indistinguishable from offset 0; a validator branching on `is_dummy()` and getting a wrong answer | few directly | 403 construction sites, overwhelmingly test scaffolding that row 1 removes anyway; 37 `is_dummy()` branches become decidable |
| 9 | Finish the typed traversal migration: no `node.kind()` string comparison outside the generated carriers | a forgotten node kind compiling cleanly and dropping a subtree | **87** (30 characterization and visitor files) | 262 `.kind()` sites; `tokens.rs` and its 17 tests go too if the grammar's coarsened `token(...)` rules are un-coarsened |

That is roughly 210 tests deleted with a compiler receipt, before row 9, and
about 300 with it. None of them is culled: each stops being writable.

**The counterexample that keeps this honest.** 65 tests in `num2text.rs`,
`num2chinese.rs` and `clan_ud_mapping.rs` are genuine behaviour tests of
algorithms no signature describes and no type can hold. Label them in source
with their class so the next reviewer does not spend a pass re-deciding. Not
every test is a missing type, and a design that cannot say which ones are not is
not a design.

## What is deleted, with sizes

| What | Size | Why it goes |
|---|---|---|
| Reference-corpus whole-file JSON snapshots, orphaned half | 335 files, 2,277,490 bytes | They correspond to no live corpus stem. Nothing runs `cargo insta test --unreferenced`, which is why 76% of that directory can be orphaned undetected. |
| Orphaned insta snapshots from deleted test targets | 54 files, 53 KiB, in `crates/talkbank-parser/tests/snapshots` | Four prefixes, none of which exists as a test target in that crate. No test reads or writes them. |
| Reference-corpus whole-file JSON snapshots, live half | 104 files | Replaced, not merely deleted: see the decision below. |
| Duplicate roundtrip harness | `direct_parser_roundtrip_corpus.rs`, 107 cases plus a loop test | A verbatim duplicate of `roundtrip_reference_corpus.rs` over the same 107 files with the same parser; its `DIRECT_PARSER_SKIP` list is empty and its name refers to an integration that has happened. Two harnesses proving one property. |
| The repository-root error corpus | 26 tracked files under `tests/error_corpus/`, plus `generate_error_corpus.rs` | A second error corpus at a second root. Its generator resolved one parent too many and wrote OUTSIDE the repository, verified by the 66 files found beside it; the path is fixed and now refuses a root that does not contain the manifest directory. Four spec files still declare a `source` pointing at it. The 436-fixture spec corpus supersedes it; migrate the 19 parse-error cases into `spec/errors/` and fix the four `source` lines. |
| CLI command-surface manifest | `command_surface_manifest.rs` + `SURFACE_GROUPS`, 5 tests | A hand-written second copy of a list clap already derives, compared by parsing help text. Its own comment records the failure: a wrapped description line produced a phantom command. Derive from clap's command tree; keep the per-family coverage EXPECTATION as an attribute beside the command definition. |
| Duplicate word-validation snapshots | `validation/word/snapshot_tests.rs`, 7 tests | Same helper, same fixtures, same codes as `tests.rs`, asserted through `insta` instead. A second assertion of one behaviour. |
| Orphaned golden lists at the repository root | `golden_words_featured.txt`, `golden_words_minimal.txt` | Written to the CWD by an audit binary, tracked, read by nothing, disagreeing with both the crate copies and the book page that documents their counts. Three-way drift with no owner. |
| Authored construct CST blocks | 137 blocks, 41 of them naming node types the grammar does not have, plus `## Metadata`, `update_cst`, and the second format reference | Dead data that has rotted. See the construct-claim decision above. |
| Characterization and visitor suites | 30 files, 87 tests | They pin a migration's endpoint, captured by running the pre-migration parser, and one of them opens with a paragraph arguing it is not scaffolding while the next names its migration task. Delete AFTER row 9 of the type table, never before: they are load-bearing until the last `.kind()` comparison is gone. |
| The 20-case property tests | 12 of 18 `proptest!` blocks | Not deleted outright: either restore a real budget (256+) on the roundtrip and span properties and accept the runtime, or delete those blocks and keep their committed regression seeds as ordinary examples. A property at 20 cases over a 1-8 lowercase-letter alphabet is an example test with extra machinery. |

**The one contested deletion, decided.** The 4 MB reference-snapshot population
is 92% of all committed snapshot bytes and its diffs are unreadable, so a real
regression and a field rename look identical and both get accepted. That argues
for deletion. Against it: those snapshots are what kills parser mutants, and
deleting them would leave the parser's structural output pinned by nothing while
the validator is already weakly pinned. Both are right, so the answer is to keep
the DETECTOR and drop the FORM: one derived artifact under the registry, one row
per reference file, carrying a hash of the canonical model JSON plus a
structural summary (counts by model node kind, diagnostic codes, roundtrip
byte-exactness), reviewed as a diff the way `example-diagnostics.json` already
is. Detection is unchanged, because any model change flips the hash. The honest
cost: when only the hash moves and the summary does not, seeing WHAT changed
means regenerating the full JSON against the parent commit. That is a two-command
operation, and it is the price of a review artifact a human will actually read.

**Not deleted, wired.** 39 JavaScript tests in the desktop app are invoked by no
workflow and no justfile recipe. Tests that no gate runs are worse than none,
because their presence reads as coverage. Either `npm run test:unit` joins the
gate or the files go. The Rust Tauri bridge tests stay and grow: they are the
specific hole a four-week desktop validation outage went through.

## Migration order

Each step leaves the repository working and gated. Each names what it REMOVES,
because a step that only adds is suspect.

**Step 0. Make the instrument runnable.**
Add `llvm-tools` to `rust-toolchain.toml`'s `components`, beside the existing
`targets` entry and for the same reason the comment there already gives: rustup
installs it, rather than a runbook asking a human to remember. Derive the
generated-file exclusion list from the artifact registry and consume it from both
the coverage invocation and `mutants.toml`.
*Removes:* the second, hand-maintained notion of "which files are generated"
(one file listed where eleven qualify), and one manual prerequisite from the
coverage recipe.

**Step 1. Delete the dead weight and install the hygiene that stops it
returning.**
335 orphan snapshots, the 26-file root corpus and its misdirected generator, the
duplicate roundtrip harness, the two root golden files and the audit binary that
writes to the CWD, the command-surface manifest, the duplicate word snapshots.
In the same commit: `cargo insta test --unreferenced=reject` joins `just gate`,
and a ratchet counts `new_unchecked` and `Span::DUMMY` construction in test code
and refuses an increase.
*Removes:* about 2.4 MB and roughly 470 files and cases, plus the ability for
any of it to come back. The ratchet is what makes every later step provable:
the number can only fall.

**Step 2. Attribution.**
Convert `validation_errors_detected` to one `rstest` case per manifest fixture.
Build the six-class attribution pass in `xtask`, emitting the row artifact with
every verdict empty. Publish the real coverage figure for the full suite, which
this page cannot state today.
*Removes:* the single opaque test that did 436 fixtures' work anonymously, and
the state of not knowing what covers what. This is the one step that mostly
adds, and it earns that by producing the deletion list every later step consumes:
the marginal-contribution ranking of all 543 committed fixtures.

**Step 3. Read the corpus once.**
Run the attribution pass with the `#[ignore]`d production-corpus class included.
Record the branches reachable ONLY by it. Synthesize a fixture for each, seeded
from `corpus/reference/` with the existing perturbation generator, which needs a
valid seed rather than a large corpus.
*Removes:* the 22 ignored corpus tests, the `$TALKBANK_DATA` dependency, and the
last route by which evidence is produced and discarded. After this step a
successor who cannot read the corpus inherits everything it taught us.

**Step 4. The `Word` body.**
Type table row 1. Rewrite the eleven affected specs' claims from "implemented,
unreachable by this fixture" to parse-level `violates` examples, which is the
honest version of what they already say. Re-baseline the differential.
*Removes:* 36 tests, 11 model-layer emitters, a character-by-character rescan of
raw text inside the CHAT core, and 147 fabricating call sites.

**Step 5. Coordinates, provenance and the preamble.**
Type table rows 2, 3, 4, 8. Do them together: rows 3 and 4 are one change seen
from two sides, and row 8's value is only realised once row 1 has removed the
scaffolding that constructs sentinels.
*Removes:* 51 tests, four cached fields that no longer have a source to drift
from, four `Option` fields on the validation context, one byte-duplicate function
pair, and the class of defect in which a fabricated span is indistinguishable
from a measured one.

**Step 6. Construct claims.**
Give a construct spec a model projection; make the generated test assert it;
delete the authored CST blocks, `## Metadata`, `update_cst` and the second
format reference; relabel the sexp corpus as a derived pin in the registry's own
vocabulary. Promote the exact per-stage code set to a claim where adjudicated,
and derive `status` from the observation snapshot.
*Removes:* 137 rotted blocks, one function that writes into a human-authored
directory, one duplicated format reference, one authored field the system can
observe, and 138 assertions that a string did not crash the parser.

**Step 7. Finish the traversal migration.**
Type table row 9, then delete the characterization and visitor suites.
*Removes:* 87 tests, 30 files, 262 string comparisons against node kinds, and,
if the grammar's coarsened tokens are opened up, `tokens.rs` and its 17 hand
written string parsers.

**Step 8. Close the criterion.**
Every row carries a verdict; the verdict list becomes the gate, ratcheting in
both directions. Run the scoped mutation pass over the validator against a suite
whose coverage is now attributable, and treat its survivors as the negative
example worklist.
*Removes:* the last unattributed branches, and the possibility of a green suite
nobody can account for.

**Step 9, standing.** Stop committing what a generator can lower at test time,
or exclude the lowered fixtures from review diffs. 436 committed fixtures are
noise in every diff that touches a spec. This is a judgement about review
ergonomics, not correctness, which is why it is last and optional.

## Where evidence goes when you fix a bug

| The bug is | The evidence is | Where |
|---|---|---|
| a wrong verdict on some CHAT input | a spec example with a claim | `spec/errors/E###_*.md`, then `just spec-gen`, then adjudicate the observation diff as INTENDED |
| a false positive (we reject valid CHAT) | a `legal` example, which is the negative half nothing else can state | same |
| a wrong parse SHAPE | a construct spec with a model claim | `spec/constructs/<area>/` |
| an invariant that holds for all inputs | a property, at a real case budget, seeded | `property_tests/` |
| only reachable through the CLI, LSP or desktop | a boundary test in that crate | `crates/chatter/tests/integration`, `talkbank-lsp`, `src-tauri/tests` |
| one parser disagreeing with the other | a shrunk `KNOWN_DIVERGENCES` baseline | `talkbank-parser-re2c` |
| a CLAN CHECK adjudication | a manifest row plus a fixture, and the CLAN identity it was verified against | `check_parity/manifest.json` |
| an algorithm producing the wrong output | a unit test, labelled with its class | beside the algorithm |

**Never**: a new hand-built AST in `talkbank-model`. **Never**: a new whole-file
JSON snapshot. **Never**: a second corpus, a second manifest, or a second copy
of a list a generator owns.

And one rule for reading a failure before writing anything: when `chatter`
rejects a file, the working assumption is that chatter has a defect, not that
the data does, until the construct is shown to genuinely fail to make sense.
Editing data to satisfy a validator is how a parser bug becomes permanent.

## How you know the suite is complete rather than green

Three questions, and each has exactly one instrument:

1. **Did anything RUN the code?** The attribution artifact. Gated: no row
   without a verdict, no covered row claiming to be unreachable, the
   `NEEDS_SPEC_EXAMPLE` count only falls.
2. **Did anything OBSERVE what the code did?** Mutation, scoped to the validator
   and to `Claim::satisfied_by`. Not gated per push (it is a report about the
   suite, not about a commit), run deliberately, its survivors triaged into spec
   examples.
3. **Is what the code does RIGHT?** The two oracles narrow it: an independent
   second parser catches a defect in one implementation, and the CLAN ledger
   catches drift against a recorded external verdict. Neither can catch a defect
   both sides share. Beyond them the answer is human adjudication against the
   format authority, recorded in the spec with a `source` and a `notes`, and it
   is not gateable.

A suite that answers 1 and 2 is not blind. Nothing in this repository can make
it right. Say that out loud in every report; a claim of total coverage is the
failure this architecture exists to correct.

## The strongest objection, and the honest answer

**The objection.** Coverage cannot see whether the spec is right, so a
completeness criterion built on coverage measures our own arithmetic rather than
the language. This repository could reach 100% branch coverage entirely from
spec examples that encode a wrong understanding of CHAT, and every gate would be
green while the tool is wrong about the format. Worse, the design deliberately
demotes the only three instruments that could catch that: the production corpus
is read once and then discarded, the CLAN grounding half is `#[ignore]`d and
depends on a binary a successor may not have, and human adjudication with the
format authority is explicitly outside every gate. The design therefore optimises
for a property it can measure (branches touched by committed fixtures) and
retreats from the property that matters (agreement with CHAT as it is actually
practised).

**The answer, in four parts, and the first is a concession.**

1. **Conceded, without qualification.** The criterion proves the suite is not
   blind. It never proves chatter is right. That is why the gate is a verdict
   and not a percentage, why "there is no gold" is stated in the limits section
   rather than buried, and why every comparison against a human artifact is
   called agreement rather than accuracy. A design whose strongest claim is "no
   branch is unaccounted for" is a smaller claim than "chatter is correct", and
   it is the largest claim any repository-internal gate can support.

2. **The instruments are demoted, not removed, and demotion is what makes them
   survive.** A gate that needs a private corpus is a gate that a successor
   cannot run, which means in practice it is a gate nobody runs, which is
   strictly worse than an evidence pass with a committed output. Step 3 converts
   every branch the corpus alone can reach into a committed fixture: the corpus
   is spent once and its findings are permanent. The CLAN half keeps its
   grounding test and gains a recorded CLAN identity in the manifest, so "last
   verified against" is visible rather than assumed. The differential baselines
   shrink as ratchets rather than sitting as hand-typed constants. In each case
   the instrument's OUTPUT enters the repository, which is the only form in
   which it outlives the person who ran it.

3. **A wrong belief gets an address.** The alternative on offer is not a suite
   that knows CHAT better; it is the current suite, which encodes the same
   beliefs implicitly across 2,868 tests, unaddressably, half of them over ASTs
   no parser produces. Under this design a belief about CHAT is one spec file
   with a code, a description, a rule statement, examples with claims, and a
   `source`. When it turns out to be wrong, a successor changes one file and
   watches the observation snapshot show every behavioural consequence at once.
   That is not correctness, but it is the precondition for correcting anything.

4. **What genuinely remains uncovered.** Nothing prevents a maintainer from
   writing shallow examples that touch branches and assert little, and no gate
   can detect intent. Three pressures bound it and none eliminates it: mutation
   kills a shallow example, because a fixture that touches a branch without
   observing its output lets the mutant survive; marginal-contribution ranking
   deletes fixtures that add no branches, so the corpus is pushed down as well as
   up; and the verdict is a written adjudication with a reason, which a later
   measurement can contradict in public. The residual risk is real and permanent:
   correctness against CHAT as practised is a question about the world, and the
   honest posture is to keep saying so rather than to let a green gate imply
   otherwise.

**Two runners-up, answered briefly.**

*"Forcing validation tests through the parser couples two crates, so a parser
defect can now mask a validator defect."* True, and intended. A validation rule
that can only be triggered by an AST no parser produces is not a rule about
CHAT; the specs for those rules already admit as much in their own text. The
masking risk is bounded by the differential: a parse-stage defect that hides a
validation rule shows up as a divergence unless both parsers share it. Where a
validator genuinely guards a construct the grammar does not yet produce, the
honest verdict is `UNREACHABLE_FROM_CHAT`, and the honest action is to delete the
code and re-add it with the construct.

*"Branch coverage as a gate is Goodhart bait."* It would be, as a percentage.
It is gated as a verdict per row, and the percentage is deliberately not a gate
anywhere in this design. The number appears in this document exactly once, as a
floor, with the reason it is a floor.

## Appendix A: how every number here was produced

Run from the repository root. No number in this document was written from
memory.

```bash
# Test attributes, repo-wide and per crate
rg -c '^\s*#\[(test|tokio::test)\]' $(git ls-files '*.rs') | awk -F: '{s+=$2} END{print s}'
rg -c '^\s*#\[(test|tokio::test)\]' $(git ls-files 'crates/talkbank-model') | awk -F: '{s+=$2} END{print s}'

# Fabricated-AST construction
rg -c 'new_unchecked' $(git ls-files '*.rs') | awk -F: '{s+=$2} END{print s}'
rg -c 'Span::DUMMY'   $(git ls-files '*.rs') | awk -F: '{s+=$2} END{print s}'

# Construct specs, and the corpus cases derived from them. The `cst` fence is
# written both as ```cst and as ``` cst, with a space, in 14 specs; a scan
# anchored on the first form misses those and under-counts the rot by 14, which
# is how this page briefly carried 27 instead of 41.
# The three coverage runs. The THIRD is what makes the fabrication split exact:
# it excludes the fabricating package from the RUN but not from the REPORT, so
# the regions only those tests reach are `full - without`.
cargo +nightly llvm-cov --branch --workspace --tests \
    --json --output-path /tmp/cov-all.json
cargo +nightly llvm-cov --branch -p talkbank-model -p talkbank-parser --lib \
    --json --output-path /tmp/cov-fab.json
cargo +nightly llvm-cov --branch --workspace --tests \
    --exclude-from-test talkbank-model --json --output-path /tmp/cov-nofab.json
just coverage-attribution /tmp/cov-all.json crates/talkbank-model/src/validation

git ls-files 'spec/constructs/**/*.md' | wc -l
rg -N -c '^={80}$' grammar/test/corpus/generated/ -g '*.txt' | awk -F: '{s+=$2} END{print s/2}'
rg -N -c '^={3,}$'  grammar/test/corpus/manual/    -g '*.txt' | awk -F: '{s+=$2} END{print s/2}'

# Committed snapshots
git ls-files '*.snap' | wc -l
git ls-files '*.snap' | xargs wc -c | tail -1
git ls-files '*.snap' | sed 's|/[^/]*$||' | sort | uniq -c | sort -rn

# Spec system
git ls-files 'spec/errors/*.md' | wc -l
rg -c '^\[\[example\]\]' $(git ls-files 'spec/errors/*.md') | awk -F: '{s+=$2} END{print s}'
git ls-files 'spec/constructs/**/*.md' | wc -l
git ls-files 'crates/talkbank-parser-tests/tests/error_corpus/validation_errors/*.cha' | wc -l
git ls-files 'corpus/reference/**/*.cha' | wc -l

# Error-code multiplicity (why per-code examples cannot prove per-branch coverage)
rg -o 'ErrorCode::[A-Z][A-Za-z0-9]*' crates/talkbank-model/src crates/talkbank-parser/src \
  --no-filename --glob '!*generated*' | wc -l
rg -o 'ErrorCode::[A-Z][A-Za-z0-9]*' crates/talkbank-model/src crates/talkbank-parser/src \
  --no-filename --glob '!*generated*' | sort -u | wc -l

# Generated code, for the exclusion list
rg -l --glob '*.rs' -e '@generated' -e 'DO NOT EDIT' -e 'do not edit' crates/ spec/ apps/
wc -l crates/talkbank-parser/src/generated_traversal.rs

# Branch coverage (nightly; the pinned toolchain gives regions only)
cargo +nightly llvm-cov --branch -p talkbank-model  --lib --json --output-path model.json
cargo +nightly llvm-cov --branch -p talkbank-parser --lib --json --output-path parser.json
```

The coverage figures in this document came from those two commands with
generated files excluded in post-processing, and they are floors for the reasons
given in
[Where it stands today](#where-it-stands-today-honestly). The construct-CST rot
count (41 of 137 blocks naming node types the grammar lacks) was produced by
checking each authored block's node names against `grammar/src/node-types.json`;
the snapshot-orphan count by matching
`talkbank_parser_tests__snapshot__<name>.snap` against the tracked reference
corpus stems. Both are one-off scans; re-derive them rather than trusting these
numbers after any spec or corpus change.
