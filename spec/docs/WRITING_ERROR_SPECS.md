# Writing Error Specs, Quick Reference

**Last modified:** 2026-08-21 11:03 EDT

See [ERROR_SPEC_FORMAT.md](ERROR_SPEC_FORMAT.md) for the complete format
reference. This page covers the practical workflow.

## Adding a New Error Spec

1. **Create the file**: `spec/errors/E{NNN}.md` or `E{NNN}_{suffix}.md`

2. **Write the spec**: `+++` TOML frontmatter for everything declared, then
   markdown prose. The complete field list, with types and requiredness, is
   `talkbank_spec_vocabulary::frontmatter`; an unrecognised key is a load
   error, so the schema tells you when you get it wrong.

   ````markdown
   +++
   code = 'E999'
   name = 'ErrorName'
   kind = 'Invalidity'
   status = 'not_implemented'
   level = 'utterance'

   [[example]]
   claim = 'violates'
   chat = '''
   @UTF8
   @Begin
   @Languages:	eng
   @Participants:	CHI Child
   @ID:	eng|corpus|CHI|||||Child|||
   *CHI:	the bad input here .
   @End
   '''
   +++

   ## Description

   What this error means.

   ## Notes

   - Any implementation notes.
   ````

   The claim is required: `violates` asserts the spec's own code fires,
   `legal` asserts it does not, `subsumed_by <code(s)>` asserts the named
   codes fire and the spec's own does not.

3. **There is no layer to choose.** Which stage catches the rule is observed
   when the artifacts regenerate, recorded per example in
   `spec/observations/`, and corpus membership follows the observation.

4. **Verify the example triggers the right code**:
   ```bash
   chatter validate /tmp/test.cha --force
   ```

5. **Regenerate tests**:
   ```bash
   just spec-gen      # every artifact derived from spec/
   just spec-check    # or: is the committed copy current?
   ```

6. **Run tests**:
   ```bash
   cargo test -p talkbank-parser-tests --release
   ```

## Common Mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Example triggers a different code | Test fails naming the code it actually emitted | `claim = { subsumed_by = 'E{actual}' }` (the honest worklist entry), or fix the example |
| The rule is not wired up yet | Test runs and fails | `status = 'not_implemented'`, which defers the example |
| A misspelled or invented key | The spec does not load, naming the key | The schema lists every key: unrecognised ones are refused, not ignored |

Three mistakes that were in this table and are no longer possible: a
declaration written below its fence (there is no fence), a wrong fence info
string (there is no info string), and a missing `status` producing a test that
runs anyway (a spec without one does not load).

## Validating Specs

```bash
# Check all spec format/layer correctness
cargo run --bin validate_error_specs --manifest-path spec/runtime-tools/Cargo.toml

# Check coverage (all error codes have specs)
just spec-coverage
```

---
