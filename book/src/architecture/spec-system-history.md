# Why the Spec System Looks Like That

**Status:** Current
**Last modified:** 2026-09-03 19:40 EDT

[Spec System](spec-system.md) says what the spec files contain and what checks
them. This page answers the questions that page raises and does not settle, all
of which a new contributor hits in the first hour:

- Why are two thirds of the files named `_auto`?
- Why can a spec for one code carry an example that expects a different one?
- Why did eleven codes have two spec files (one still does)?
- Why do so many descriptions say "Auto-generated from corpus"?

The short answer to all four is the same, and it is worth stating plainly
because it is a defect being worked off rather than a design:

> **A large part of `spec/errors/` was written by a machine that recorded what
> chatter DID, in files whose job is to say what chatter SHOULD do.**

Everything below is measured. Reproduce any of it with
`python3 scripts/analysis/spec_system_audit/audit.py` in the workspace repo, or
with the command named beside each number.

## The numbers, as of 2026-08-15

| Fact | Value |
|---|---|
| Error spec files | 238 |
| of those, named `*_auto.md` | **152 (64%)** |
| still carrying `Review and enhance this specification as needed` | **91** |
| whose Description is the literal `Auto-generated from corpus.` | 39 |
| still carrying `[Add link to relevant CHAT manual section]` | 68 |
| codes with NO example producing the code the spec is named for | **54** |
| codes claimed by more than one spec file | 11 |

The same measurements on 2026-09-03, after Phase 5 and Phase 6 were worked
through (live `ls`/`grep` over `spec/errors/`, no tool):

| Fact | Value |
|---|---|
| Error spec files | 225 |
| of those, named `*_auto.md` | **1** (E519_auto.md) |
| still carrying `Review and enhance this specification as needed` | **1** |
| whose Description is the literal `Auto-generated from corpus.` | 1 |
| still carrying `[Add link to relevant CHAT manual section]` | 1 |
| codes claimed by more than one spec file | 1 (E519) |

The one remaining case in every row is E519, which keeps its three files
until it is ruled whether header-level and word-level ISO 639-3 membership
are one spec (see below).

## Where `_auto` came from

The tool is GONE: `corpus_to_specs` and `enhance_specs` were deleted under R5
of the spec-system redesign, and `spec/errors/` now carries a `.human-authored`
marker that every generator refuses to write into. This section is history, and
the state it describes cannot be added to.

A bootstrap tool, `corpus_to_specs`, converted a directory of error-corpus
`.cha` fixtures into spec files. For each example it wrote an
`**Expected Error Codes**` line, which is a claim about what the validator DID
on that input.

Its only source for that claim was an `expectations.json` beside the corpus.
**That file does not exist and never has** (`fd expectations.json` finds none;
`git log --diff-filter=D` shows none was ever deleted). Three silent defaults
carried the gap to the page: a missing file became an empty map, an unparseable
file became an empty map, a per-file miss became an empty code list, and the
emitter turned an empty code list into *the spec's own filename code*.

So every such line the tool ever wrote asserts, as a measurement, the answer the
filename already implied. Since 2026-08-15 the tool refuses to run rather than
fabricate, but the 152 files it produced are still there.

## Why an E202 spec can expect E316

The format allows an example to declare codes other than the spec's own, and
`ERROR_SPEC_FORMAT.md` documents it as intended, for the case where "a spec's
input triggers a different error code than the spec itself documents."

That single sentence merges two facts of completely different kinds:

- **normative**: "this input is illegal under E202", a decision about CHAT that
  the implementation must satisfy;
- **observed**: "chatter reports E316 on this input today", a fact about our
  binary that changes when we change the binary.

When they are the same field, a spec that documents a GAP is indistinguishable
from a spec that documents a RULE, so a spec can exist, carry examples, pass
every gate, and still demonstrate nothing about the rule it is named for.

Measured 2026-08-20: of the 224 codes owned by a spec, **52 are declared by no
example anywhere**, and **22 of those are `implemented`** (E001, E002, E208,
E231, E232, E253, E307, E313, E314, E315, E324, E330, E340, E361, E363, E382,
E506, E508, E510, E511, E512, E710). Reproduce it with `cargo run --bin coverage -- --errors`, which reports the
same population from the loader. The original instruction here was to collect
each spec's own `Error Code` bullet and subtract every code an
`**Expected Error Codes**` line declared; neither exists since the format moved
to frontmatter, so the stated method no longer runs.

A gate used to count the difference: `SpecSelfDemonstrationGate` ratcheted a
shrink-only 36-entry baseline of the specs in this state from 2026-08-15 until
R2 (2026-08-21) made the claim REQUIRED, at which point "demonstrates nothing"
stopped being writable and the gate was deleted with its baseline. The
population survives as the `subsumed_by` worklist that `coverage --errors`
prints. Most of
that list declares E316, "unparsable content", meaning the mined input does not
parse so the specific rule is never reached at all.

`cargo run --bin coverage -- --errors` prints the live list with what each spec
declares INSTEAD, which is what tells you which kind of problem you have: an
E316 entry is a parser gap; a specific other code is usually a wrong fixture.

**What this means for you as a reader:** a `subsumed_by` claim tells
you what chatter emits, not necessarily what the rule is. Read the spec's
Description and title for the rule, and treat a mismatch between the title code
and the example's codes as an open question rather than as a specification.

**What it means for you as an author:** do not add new examples in that shape.
If an input violates the rule your spec is about, the example should produce
that rule's code; if it does not, chatter has a gap, and the gap is the finding.

## Why some codes have two spec files

Eleven did, until 2026-09-03; one (E519) still does. In every case the pair
was one `_auto.md` plus one hand-written file:

```
E202_auto.md  +  E202_missing_form_type.md
E241_auto.md  +  E241_illegal_untranscribed_marker.md
E519_auto.md  +  E519_l1_of_language_code.md  +  E519_word_level_language_code.md
```

Somebody hit a useless machine-written spec, wrote a real one beside it, and had
no way to retire the machine's version. Nothing declares which is authoritative;
nothing forbids a third. The hand-written ones are good, and
`E522_undefined_participant.md` is the model: a real description, a `Kind`, a
`Status`, and an example that declares its own code and emits it.

**Two different things wear that shape, and they need opposite fixes.** Measured
2026-08-19 by comparing each pair's declared fields:

**Do not answer this from the metadata.** Two attempts did, and both were
wrong, because `Level` was declared per file at the time and a generator
wrote it for an unedited `_auto` stub by running the parser. Run the examples instead: take each spec's `chat` values, validate them, and
compare the DIAGNOSTICS rather than the declarations.
`scripts/analysis/adjudicate_contested_spec.sh` in the operator workspace does
this. (It read fenced blocks when this was written; the examples moved into
frontmatter in Phase 1b.) Measured that way on 2026-08-20:

- **Residue, identical diagnostics from both files**: E202 ("Missing form type
  after @" from each), E241 (`"xx" is not legal` from each), and E604 (E604 plus
  E722 from each, differing only in a double space). These get deleted.
- **Misfiled rather than duplicated**: `E243_auto.md` is filed for E243 and its
  example emits E202.
- **Different rules under one code**: E519's stub emits "disallowed
  placeholder" while its sibling emits "not in the ISO 639-3 registry"; E316,
  E342, E375 and E522 likewise pair genuinely different malformed inputs. E360
  and E502 are authored on both sides.
E519's two authored files are one rule, ISO 639-3 membership, reported once from
a header and once from an utterance. `Level` was declared once per FILE (and
`Layer` was too, until R4 deleted it in favour of the observation snapshot), so
a rule with two triggering sites could not be written as one spec. Phase 2
(2026-08-21) moved `level` onto the example, so such pairs are now mergeable;
merging them is part of R8's adjudication rather than automatic.

**Executed 2026-09-03.** The residue (E202, E241, E604 `_auto`) was deleted;
`E243_auto.md`'s example was re-filed under E202; E316, E342, E375, E522,
E360 and E502 were each merged into one bare `E###.md` with both bodies
preserved. E519 alone keeps its three files, pending the ruling above. The
merge changed every key the re2c parity baseline uses for those files, so
`KNOWN_DIVERGENCES` is regenerated from the harness's own output as part of
the same work.

For telling an unedited stub from an authored spec, 91 spec files carried
the generator's "Review and enhance this specification as needed" note on
2026-08-15, all of them `_auto` and none hand-named; on 2026-09-03 one does
(`E519_auto.md`). That is a reliable signal of ORIGIN. It is not
a verdict about the file's worth, and neither is any declared field: only
running the examples is.

Until 2026-08-19 a fifth field, `Category`, split all eleven, which made the four
look like the seven. It was a published grouping string that no generation
decision read and that mostly restated `Level`, so it was deleted rather than
normalised. Its 236 values are recoverable from the commit that removed them, and
a future taxonomy should be a closed enum seeded from those rather than another
free-text field. E202 now renders as two byte-identical index rows; the other
three residue pairs are still told apart by the `Name` column. Making the
duplication visible is the point.

The one thing that IS enforced across a pair is `Kind`: it is a property of the
code, so the `DiagnosticKind` generator refuses to run when two files disagree
about it.

## What is genuinely reliable today

Not everything is suspect, and it matters to know which parts you can lean on:

- **The code sets agree exactly.** Every `ErrorCode` variant has a spec file and
  every spec-named code has a variant, in both directions, enforced by the
  `DiagnosticKind` generator refusing to emit on divergence.
- **Enforcement status has one answer.** The enum's `#[status(planned)]` and the
  spec's `**Status**` are reconciled by a gate that fails in either direction.
  It was built after `--list-checks` was found wrong about 15 of 225 codes.
- **Every generated artifact is checked.** `just spec-check` compares all four
  byte-for-byte against what the specs produce now.
- **Every example that declares codes emits them.** That is `error_spec_codes`,
  and it is a real check; it simply cannot see whether the codes declared are
  the ones the spec is about.

## Where this is going

The direction is recorded in the workspace repo's design note
(`docs/design/2026-08-15-spec-system-redesign.md`); the parts that change what
you write are:

- an example will carry a typed **claim** (`violates` / `legal` /
  `subsumed by E###`) instead of a free-text code list, so a normative decision
  and an observation stop being the same field, and so a spec that demonstrates
  nothing becomes unwritable;
- `legal` is new capability: today a spec cannot assert that a code must NOT
  fire, which is exactly the shape of every false-positive question;
- metadata moves to frontmatter with one parser and one validation command, so
  an agent writing a spec can check it without running the suite. **SHIPPED
  2026-08-21 as Phase 1b**: all 236 specs are `+++` TOML deserialized against a
  schema that refuses an unrecognised key, and the artifacts regenerated
  byte-identical;
- the machine-written residue is worked off per code, as a count that may only
  shrink.

Until then, this page is the honest description of what you are looking at.
