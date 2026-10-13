# Construct Curation Manifest

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/constructs/_CURATION_MANIFEST.md)

This checklist tracks curated construct coverage. Mark items as curated only when
there is a minimal, representative spec in `spec/constructs/`.

## Header

- [x] `@Languages`
- [x] `@Participants`
- [x] `@ID` variants
- [x] `@Media` variants
- [x] `@PID`
- [x] additional required/optional headers in current grammar

## Main Tier

- [x] simple utterance
- [x] multi-word utterance
- [x] overlap markers
- [x] fused terminator
- [x] action/comment constructs with edge punctuation
- [ ] media bullets and time markers

## Dependent Tiers

- [x] `%mor` baseline
- [x] `%gra` baseline
- [x] `%pho` baseline
- [x] `%com` baseline
- [x] `%wor` baseline
- [x] uncommon tier labels represented by grammar
- [x] alignment-sensitive edge cases
- [x] selected repeated MOR post-clitics with ordered flat/keyed features and POS subcategories (`tiers/mor_repeated_postclitics.md`)

## Word/Token Layer

- [x] plain words
- [x] representative punctuation/marker variants
- [x] overlap-enclosed token
- [ ] exhaustive clitic/compound interaction completeness (selected combinations already represented)
- [ ] exhaustive annotation-combination completeness (selected combinations already represented)

These are finite curation witnesses, not execution-coverage or validity proofs.
MOR items, expanded word chunks and GRA relations are different count spaces;
the selected combination reference and E705/E706/E711/E720 boundary specs keep
those requirements separate. Recovery outcomes require actual producer review.

## Process Gate

- [x] candidate mining report reviewed
- [x] curated specs updated from report
- [x] current spec generators run successfully
- [x] `tree-sitter test --overview-only` passes
