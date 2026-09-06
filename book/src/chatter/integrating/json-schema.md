# JSON Schema

**Status:** Current
**Last modified:** 2026-09-05 12:03 EDT

This repository generates JSON Schema from Rust-owned types with
[schemars](https://docs.rs/schemars) for the `ChatFile` transcript model used
by `chatter to-json`.

Keeping that schema generated from the Rust source of truth lets cross-language
integrations consume a stable contract without re-deriving the shapes by hand.

## Available schemas

| Schema | Canonical URL | Repository | Generator |
|----------|------------|------------|------------|
| `ChatFile` transcript model | `https://talkbank.org/schemas/v0.1/chat-file.json` | `schema/chat-file.schema.json` | `just schema-gen` |

The generated schema declares both `$schema` (JSON Schema 2020-12) and `$id`
(the canonical URL above). External consumers that want to track the
current transcript-model version should follow the `v0.1` URL; there is
no `/latest/` alias in the generated artifacts.

## Transcript schema: `ChatFile`

`chatter to-json` converts CHAT transcripts into a structured JSON form backed
by the same `ChatFile` model used by the parser, validator, and serializer.

### How `chatter to-json` uses it

By default, `chatter to-json`:

- validates the CHAT input,
- checks dependent-tier alignment unless `--skip-alignment` is passed, and
- validates the emitted JSON against the schema unless
  `--skip-schema-validation` is passed.

These controls are independent. `--skip-schema-validation` skips only the
JSON Schema check after CHAT parsing and validation. It retains the input
filename, so filename-dependent rules such as E531 (`@Media` name mismatch)
still run for both single files and directory conversions. A directory run
prints individual parse/validation diagnostics and exits unsuccessfully if
any file fails, while retaining successfully converted siblings.

Library callers that know the transcript name can use
`chat_to_json_with_schema_policy` with `TranscriptName` and
`JsonSchemaPolicy::{Validate, Skip}`. The policy selects serialization only;
it cannot alter the parse options or discard the transcript name.

Useful flags:

```bash
chatter to-json input.cha --skip-validation
chatter to-json input.cha --skip-alignment
chatter to-json input.cha --skip-schema-validation
```

`chatter from-json` deserializes JSON back into the internal `ChatFile` model
and re-serializes it to CHAT format. The input should conform to this schema.

### Roundtrip expectations

The CHAT-to-JSON-to-CHAT pipeline is intended to preserve the `ChatFile` model:

```bash
chatter to-json input.cha -o intermediate.json
chatter from-json intermediate.json -o output.cha
diff input.cha output.cha
```

Both directions go through the same typed model. When changing the parser,
serializer, or schema generation, confirm roundtrip behavior with the existing
roundtrip test suites rather than assuming byte-for-byte identity.

### Using the schema externally

#### Validate JSON in Python

```python
import json
import jsonschema
import urllib.request

schema_url = "https://talkbank.org/schemas/v0.1/chat-file.json"
schema = json.loads(urllib.request.urlopen(schema_url).read())

with open("transcript.json") as f:
    data = json.load(f)

jsonschema.validate(data, schema)
```

#### IDE autocompletion

```json
{
  "$schema": "https://talkbank.org/schemas/v0.1/chat-file.json",
  "lines": [],
  "participants": {},
  "languages": [],
  "options": []
}
```

#### Generate types from the schema

Tools like [quicktype](https://quicktype.io),
[json-schema-to-typescript](https://github.com/bcherny/json-schema-to-typescript),
and [datamodel-code-generator](https://github.com/koxudaxi/datamodel-code-generator)
can generate typed structs or classes from the schema for TypeScript, Python,
Go, and other languages.

## Regenerating the schema

After changing transcript-model types in `talkbank-model`:

```bash
cd chatter
just schema-gen
```

This writes the checked-in schema artifact in `schema/`. CI already checks that
generated artifacts stay in sync.

## Code references

- `schema/chat-file.schema.json`: generated schema
- `crates/talkbank-transform/src/json.rs`: schema loading and validation
- `crates/talkbank-model/src/model/`: Rust data model
- `tests/integration/generate_schema/`: shared schema generation helpers

Schema generation is an explicit operation: `just schema-gen` selects the
ignored generator, and `just regen` includes it. Ordinary tests only check
currency, so they do not rewrite the schema while checking the version embedded
at compile time. Identical output preserves the file's modification time to
avoid invalidating builds that embed it. A currency mismatch reports the repair
command without dumping the entire schema into the test log.

The generator preserves schemars' Draft 2020-12 structure. In this dialect,
[`$ref` permits sibling keywords](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.3.1),
including the tag constraints for internally tagged enums. No `allOf` rewrite
is required. The former recursive workaround also traversed literal `const`
values and could change their meaning, so it has been removed. The generated
schema regression verifies both the tag and the referenced payload constraints.
