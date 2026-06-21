# JSON Schema

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

This repository generates JSON Schema from Rust-owned types with
[schemars](https://docs.rs/schemars) for the `ChatFile` transcript model used
by `chatter to-json`.

Keeping that schema generated from the Rust source of truth lets cross-language
integrations consume a stable contract without re-deriving the shapes by hand.

## Available schemas

| Schema | Canonical URL | Repository | Generator |
|----------|------------|------------|------------|
| `ChatFile` transcript model | `https://talkbank.org/schemas/v0.1/chat-file.json` | `schema/chat-file.schema.json` | `cargo test --test generate_schema` |

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
cargo test --test generate_schema
```

This writes the checked-in schema artifact in `schema/`. CI already checks that
generated artifacts stay in sync.

## Code references

- `schema/chat-file.schema.json`: generated schema
- `crates/talkbank-transform/src/json.rs`: schema loading and validation
- `crates/talkbank-model/src/model/`: Rust data model
- `tests/generate_schema/`: shared schema generation helpers
