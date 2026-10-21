# What the GUI CLANc CHECK does that unix CHECK does not

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser-tests/clan-check-reference/gui-vs-unix-check.md)

The retained CHECK comparison uses the Unix build, not the GUI build.
The [assessment](../../../book/src/architecture/errors-and-validation/check-parity-audit.md)
owns its scope and status; neither build defines CHAT validity by itself.
This page explains the source distinction recorded in the
[parity manifest](../tests/check_parity/manifest.json), not a fresh assessment
of every current CLAN distribution.

## The mechanics

The historical source review counted 12 `#ifndef UNX` regions in `check.cpp`
(the unix build compiles
with `-DUNX`, so these regions exist only in the Mac/Windows CLANc GUI
build). Ten were cosmetic or platform infrastructure:

- the editor-vs-curses include split (`ced.h` vs `c_curses.h`);
- Mac working-directory/volume juggling (`SetNewVol`) around depfile
  opening, plus a two-path vs one-path depfile error message;
- stderr progress-line redraw suppression under output redirection;
- four regions that inject `ATTMARKER` attribute bytes around error
  spans, which the GUI editor renders as colored highlights on the
  offending text. Terminal output has no attribute channel, so the
  unix build omits them. Cosmetic only; no validation difference.

## The substantive difference: CA-character word analysis

Two reviewed regions inside `check_CheckWords`
run `uS.HandleCAChars` over every word: the Unicode
Conversation-Analysis symbol classifier (pitch shift arrows,
inhalation, creaky/whisper/singing voice, latching, hurried start,
sudden stop, vocative comma, stress marks, Hebrew/Arabic diacritics,
and the rest of the CA repertoire). Word-internal placement rules for
those symbols, including the CA paths of codes 48, 92, 93, and the
whole of 101 and 151, are enforced ONLY in the GUI build.

In that source, this classifier belongs to the GUI's text-attribute machinery
(the same `ced.h` world that renders CA symbols specially); it was
not part of the curses build. Consequence: that Mac CLANc build
gets word-level CA-symbol validation that a unix CHECK run of the
same file does not perform. These are retained source observations, not
version-independent guarantees about an installed binary. Source line numbers
and reachability notes belong to the manifest and its identified evidence.

## What this means for chatter parity

- Both 101 and 151 have status `no_obligation`, but their reasons differ:
  - **101: `gui_only`.** Its call site is excluded by `#ifndef UNX`.
  - **151: `unreachable_in_file_mode`.** Its call site survives preprocessing
    inside `check_isThereStem`; the excluded region contains that helper's only
    caller. Compiled presence does not establish runtime reachability.

  They used to be listed in a separate `dead-codes.json` beside this
  file. That was a second, hand-maintained copy of what the manifest
  already recorded, with nothing keeping the two in agreement, and by
  the time it was removed on 2026-08-11 it was missing six codes (39,
  40, 41, 58, 74, 76). The manifest is the single owner: its
  `no_obligation_reason` is a typed value, and
  `manifest_agrees_with_clan_reference` checks the source-derivable
  reasons against the generated `check-error-codes.json` in both
  directions where source-derivable. The generated
  [reference table](check-error-codes.md) and [JSON](check-error-codes.json)
  count compiled call sites, not demonstrated runtime emissions. Code 151's
  `emitted: yes` therefore does not contradict its manifest classification.
  Regenerate those artifacts through `scripts/extract_check_codes.py` after
  a reviewed source refresh; do not hand-edit them to match this prose.
- chatter's own CA validation (e.g. E230 unbalanced CA delimiters)
  is independent of this and remains: chatter may legitimately check
  CA constructs BETTER than unix CHECK; that is ordinary
  chatter-stricter modernization.
- A GUI comparison would require an identified GUI build and independently
  retained results. A Unix CHECK receipt cannot qualify GUI behavior.
