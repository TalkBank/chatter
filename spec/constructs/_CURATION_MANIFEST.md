# Construct Curation Manifest

**Last modified:** 2026-05-29 18:43 EDT

This checklist tracks curated construct coverage. Mark items as curated only when
there is a minimal, representative spec in `spec/constructs/`.

## Header

- [x] `@Languages`
- [x] `@Participants`
- [x] `@ID` variants
- [x] `@Media` variants
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

## Word/Token Layer

- [x] plain words
- [x] representative punctuation/marker variants
- [x] overlap-enclosed token
- [ ] clitic and compound variants not yet represented
- [ ] annotation combinations not yet represented

## Process Gate

- [x] candidate mining report reviewed
- [x] curated specs updated from report
- [x] current spec generators run successfully
- [x] `tree-sitter test --overview-only` passes
