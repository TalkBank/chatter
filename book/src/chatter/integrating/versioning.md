# What a Version Bump Promises

**Status:** Current
**Last modified:** 2026-08-21 13:42 EDT

If you depend on chatter, two different things can move under you and they move
independently:

- the **Rust API**, which decides whether your code still compiles;
- the **validation verdict**, which decides whether files you already have
  still pass.

A release can be perfectly source-compatible and still change which CHAT files
`validate` accepts. That is not a defect; it is the point of the project. But
it means the version number alone cannot tell you whether a bump is safe for
your corpus, so this page says what each half promises.

## The Rust API follows SemVer

Ordinary Cargo expectations, with one qualifier: chatter is pre-1.0, so a minor
bump may break the API. Pin exactly (`=0.10.0`) if you need stability, or track
the minor and read the changelog.

## The validation verdict follows a different rule

**Any release may change which files validate, including a patch release.**
Validation is not a stable interface and will not become one before 1.0. The
project exists to move the boundary of what counts as valid CHAT, and freezing
verdicts would freeze that.

What you get instead is a **guarantee that the change is announced**:

> Every release whose validation verdicts move opens its `CHANGELOG.md` entry
> with a bold **Validation behaviour** note, naming the codes that were added,
> removed, retired or made stricter, and what kind of file is affected.

If a release has no such note, its verdicts did not move. If it has one, read
it before upgrading a pipeline that gates on `validate`.

## Which direction a change can go

Both, and they are not symmetric:

- **Stricter** (a new code, or an existing one reaching more inputs) means
  files that used to pass now fail. This is the common direction, and the
  failing files are usually genuinely wrong; the corpus they came from has
  simply not been cleaned yet.
- **Looser** (a code retired, or a false positive fixed) means files that used
  to fail now pass. Retired codes are never reused for a different rule.

Neither direction is a breaking change in the SemVer sense, because neither
touches the API.

### Retiring a code is not a clean removal

A code names a rule at a moment in time, and downstream records cite it: an
adjudication log, a repair ledger, a review note all say "this edit was made
because E754 fired", and that remains true after the rule is withdrawn. So a
retired code keeps two obligations. It is never reused, as above. And a
consumer holding historical citations should NOT validate them against the
current code set, because doing so makes a correct old record un-loadable over
a rule that was withdrawn for reasons that have nothing to do with that record.

If you keep such citations, validate them as a closed list that includes the
retirements you know about, so a typo still fails and a NEW retirement fails
loudly until someone records why. That is the check worth having; checking
against today's live set is not.

Reported by an external consumer in August 2026, whose ledger cited E754
(`LetterFormMultipleLetters`, retired 2026-08-11) for a repair that is still
correct: a digit zero typed for the letter `o` in `0@l`. The rule went away
because it counted characters and a digraph is one letter written with two;
the repair it surfaced was right either way.

## What to do about it

If you gate on `validate` in CI over a fixed corpus, treat a chatter upgrade
the way you would treat a linter upgrade: pin it, upgrade deliberately, and
diff the verdicts over your own files rather than assuming. Chatter's own
release process does exactly this against a large real corpus before shipping,
comparing per-code counts and roundtrip results against the previously released
binary; a new code or a count increase is adjudicated one instance at a time,
never waved through and never automatically treated as a regression.

If you only consume the parsed model and never call `validate`, only the SemVer
half applies to you.

## Why this page exists

An integrator pinning chatter found that the two halves were not distinguished
anywhere, and hit the case that makes the distinction concrete: a release that
was perfectly source-compatible (their adapter compiled unchanged, their whole
suite passed) while validation moved in both directions at once, one file
newly rejected in a `bad` corpus and one newly rejected in a `good` one. The
practice of announcing verdict changes already existed by then and had been
followed for several releases; it was simply not written down anywhere a
consumer would look.
