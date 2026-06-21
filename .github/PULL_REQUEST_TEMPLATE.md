<!--
Thanks for contributing to chatter. Please fill in the sections below.
See CONTRIBUTING.md for the development setup and coding conventions.
-->

## What does this change?

<!-- A short description of the change and why. Link any related issue. -->

Closes #

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Refactor / internal
- [ ] Build / CI

## Checklist

- [ ] I followed red/green TDD: a failing test at the right boundary
      first, then the fix (see CONTRIBUTING.md).
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy` (the two-pass CI lint) passes.
- [ ] `cargo nextest run --workspace` passes (or I describe what cannot
      run locally and why).
- [ ] For grammar/parser/model changes: the parser-equivalence,
      roundtrip, and reference-corpus gates pass.
- [ ] Docs updated (book and/or rustdoc) for any user-facing change.
- [ ] No `unwrap()`/`expect()`/`panic!` added to long-lived code paths.

## Notes for reviewers

<!-- Anything reviewers should focus on, trade-offs, or follow-ups. -->
