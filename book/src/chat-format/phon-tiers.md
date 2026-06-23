# Phon Tiers (%xmodsyl, %xphosyl, %xphoaln, %xphoint)

**Status:** Reference
**Last updated:** 2026-06-23 07:28 EDT

The Phon extension tiers provide syllable-level phonological annotation,
segmental alignment between target and actual IPA, and per-phone time
intervals. They are produced by the
[Phon](https://www.phon.ca/phon-manual/getting_started.html) application and
exported to CHAT via [PhonTalk](https://github.com/phon-ca/phontalk).

chatter parses and **validates all four tiers as first-class CHAT tiers**.

> **The `x` prefix.** Phon emits these tiers with a leading `x` (`%xmodsyl`,
> `%xphosyl`, `%xphoaln`, `%xphoint`) to mark them as extension tiers. The
> grammar accepts **both** the `x`-prefixed names and the historical non-`x`
> names (`%modsyl`, `%phosyl`, `%phoaln`, `%phoint`); the parser and validator
> key off the tier *kind*, not the literal prefix. The canonical serialized form
> is the `x`-prefixed name.

## The four tiers

| Tier        | Source        | Carries                                                  | Word separator |
|-------------|---------------|----------------------------------------------------------|----------------|
| `%xmodsyl`  | `%mod`        | Syllabification of the model/target transcription        | space          |
| `%xphosyl`  | `%pho`        | Syllabification of the actual transcription              | space          |
| `%xphoaln`  | `%mod`+`%pho` | Phone-by-phone alignment of model ↔ actual               | space          |
| `%xphoint`  | `%pho`        | Per-phone time intervals (`0x15` time bullets)           | ` / `          |

`%xmodsyl`, `%xphosyl`, and `%xphoaln` are word-aligned to their source tier(s)
with single ASCII spaces. `%xphoint` uses ` / ` (space-slash-space) as its word
separator because single spaces already separate the phone and bullet tokens
inside each word.

## Tier formats

### %xmodsyl / %xphosyl, syllabification

A word is one or more `phone:CODE` units concatenated with **no** internal
whitespace; words are separated by single spaces. The phone is one IPA phone
(IPA length is written with the modifier letter `ː`, U+02D0, never an ASCII
colon, so the `:` separator is unambiguous). A leading stress marker (`ˈ`
primary, `ˌ` secondary) is part of the phone it precedes.

The constituent code is one character. The legal codes are `O N C L R E A D U`:

| Code | Constituent | Notes |
|------|-------------|-------|
| `O`  | Onset | |
| `N`  | Nucleus | monophthong nucleus |
| `C`  | Coda | |
| `L`  | Left appendix | e.g. /s/ in an /s/-stop cluster |
| `R`  | Right appendix | e.g. final /z/ in a complex coda |
| `E`  | OEHS (onset of empty-headed syllable) | e.g. the stop element of an affricate |
| `A`  | Ambisyllabic | |
| `D`  | Diphthong | a nucleus member of a diphthong/triphthong; treated as a nucleus |
| `U`  | Unknown | Phon could not assign a concrete constituent; common on `%xphosyl` when the model `%xmodsyl` is fully syllabified |

The remaining Phon `SyllableConstituentType` mnemonics, `B` (boundary),
`S` (stress), `W` (word boundary), `T` (tone), are **not** emitted on these
tiers: boundary, stress, and tone need no per-phone marker.

```chat
*CHI:	I want three .
%mod:	aɪ wɑnt θri
%xmodsyl:	a:Dɪ:D w:Oɑ:Nn:Ct:C θ:Oɹ:Oi:N
%pho:	aɪ wɑn fwi
%xphosyl:	a:Dɪ:D w:Oɑ:Nn:C f:Ow:Oi:N
```

### %xphoaln, phone alignment

A word is one or more comma-separated pairs; a pair is `model↔actual` (`↔` is
U+2194). Either side may be `∅` (U+2205, empty set): `∅` on the left is an
epenthesis (a phone produced but not targeted); `∅` on the right is a deletion.
Both sides are never `∅` at once.

```chat
*CHI:	the best .
%mod:	ðə bɛst
%pho:	ðə bɛs
%xphoaln:	ð↔ð,ə↔ə b↔b,ɛ↔ɛ,s↔s,t↔∅
```

The alignment lists **segments** (phones). Suprasegmental stress (`ˈ`/`ˌ`) that
may appear on the `%mod`/`%pho` word is therefore **not** part of the alignment
pairs; the reconstruction checks below compare modulo those stress markers.

### %xphoint, per-phone intervals

`%xphoint` gives the time segmentation of each individual phone on `%pho`,
effectively phone-level bullets analogous to the word-level timing on `%wor`.
Groups (one per `%pho` word) are separated by ` / `. Within a group, each phone
is followed by a CLAN time-alignment bullet: the byte `0x15` (NAK), the interval
`start_end`, then `0x15`.

```chat
*CHI:	I want . •0_500•
%pho:	aɪ wɑnt
%xphoint:	aɪ •0_250• / w •250_320• ɑ •320_400• n •400_460• t •460_500•
```

(Bullets are shown as `•` above; in the file they are the `0x15` byte.)

## Validation

**These checks run by default.** Pass `--suppress xphon` to silence the entire
Phon `%x` validation surface, or suppress an individual code. (The historical
`--check-xphon` opt-in flag is now a deprecated no-op: the checks it used to
gate are on by default.)

**Word-count cross-checks** (each `%x` tier has the same number of words as the
tier(s) it depends on):

- `%xmodsyl` ↔ `%mod`: **E725**
- `%xphosyl` ↔ `%pho`: **E726**
- `%xphoaln` ↔ `%mod`: **E727**, ↔ `%pho`: **E728**

**Content checks:**

| Code | Tier | Rule |
|------|------|------|
| E735 | xmodsyl/xphosyl | a unit is not a well-formed `phone:CODE` (no `:`, empty phone, or empty code) |
| E736 | xmodsyl/xphosyl | a constituent code is not one of `O N C L R E A D U` |
| E737 | xmodsyl | stripping codes and concatenating phones does not reproduce the `%mod` word |
| E738 | xphosyl | stripping codes and concatenating phones does not reproduce the `%pho` word |
| E739 | xphoaln | a pair is malformed (not exactly one `↔`, an empty side, or `∅↔∅`) |
| E740 | xphoaln | concatenating the model sides (skipping `∅`, modulo stress) does not reproduce the `%mod` word |
| E741 | xphoaln | concatenating the actual sides (skipping `∅`, modulo stress) does not reproduce the `%pho` word |
| E742 | xphoint | a bullet has `start >= end` |
| E743 | xphoint | interval start times are not non-decreasing across the tier |
| E744 | xphoint | the first start / last end falls outside the record's media bullet (1 ms tolerance) |
| E745 | xphoint | a group's phones do not reproduce the `%pho` word |
| E746 | xphoint | the number of groups does not equal the `%pho` word count |

See [Alignment Architecture](../architecture/alignment.md#phon-tier-to-tier-alignment)
for the word-count implementation.

## Parsing strategy

- **%xmodsyl / %xphosyl**: stored as flat word strings
  (`talkbank-model::dependent_tier::phon::SylTier`), consistent with how `%pho`
  and `%mod` store flat phone words. The validator tokenizes each word into typed
  `phone:CODE` units (`PositionCode`) to apply the content rules above; the IPA
  characters themselves stay verbatim for exact round-trip.
- **%xphoaln**: each word is parsed into a `Vec<AlignmentPair>`, where
  `AlignmentPair { source, target }` carries one `model↔actual` mapping (`None`
  is `∅`).
- **%xphoint**: parsed into typed groups of `(phone, bullet)` pairs
  (`XphointTier` / `XphointGroup` / `PhoneInterval`), reusing the same `0x15`
  bullet machinery as `%wor`.

Deep phonological analysis is Phon's domain; chatter parses the structure that
validation needs and keeps the IPA content verbatim.

## Phon XML source format

In Phon's native XML format, phonological data is stored as structured elements:

```xml
<ipaTarget>
  <pho>
    <pw>
      <ph scType="onset"><base>θ</base></ph>
      <ph scType="nucleus"><base>ɹ</base></ph>
      <ph scType="nucleus"><base>i</base></ph>
    </pw>
  </pho>
</ipaTarget>
```

Each `<pw>` (phonological word) element contains `<ph>` elements with syllable
constituent types (`scType`). The `<alignment>` element provides phone-level
mappings between target and actual using index-based `<pm>` (phone map) entries.

## Data quality notes

A small percentage of Phon corpus XML records have an orthography↔IPA word-count
mismatch: the number of `<pw>` elements in `<ipaTarget>` / `<ipaActual>` differs
from the number of `<w>` elements in `<orthography>`. This is expected in child
phonology data: children may produce extra syllables, partial words, or
over-productions relative to the target.

For current counts on a local CHILDES/TalkBank data tree, run:

```bash
python3 scripts/analysis/scan_phon_mismatches.py /path/to/data
```

The PhonTalk CHAT export handles this discrepancy inconsistently:

1. `%mod`/`%pho` are written through a `OneToOne` alignment path that maps IPA
   words to orthography words; extras are silently dropped.
2. `%xmodsyl`/`%xphosyl`/`%xphoaln` are written directly from the raw
   `IPATranscript`; all IPA words are included.

This produces CHAT files where `%xmodsyl` may have more words than `%mod`,
triggering the E725-E728 word-count errors. This is being investigated in
collaboration with the Phon team.
