# What the GUI CLANc CHECK does that unix CHECK does not

**Status:** Reference
**Last updated:** 2026-07-08 12:48 EDT

Policy (Franklin, 2026-07-08): **unix CHECK is the authoritative parity
bar** (it is what `clan-run.sh` grounds against and what Leonid
publishes for scripting). This page documents the difference, because
"the GUI checks more" is surprising and deserves an explanation.

## The mechanics

`check.cpp` contains 12 `#ifndef UNX` regions (the unix build compiles
with `-DUNX`, so these regions exist only in the Mac/Windows CLANc GUI
build). Ten are cosmetic or platform infrastructure:

- the editor-vs-curses include split (`ced.h` vs `c_curses.h`);
- Mac working-directory/volume juggling (`SetNewVol`) around depfile
  opening, plus a two-path vs one-path depfile error message;
- stderr progress-line redraw suppression under output redirection;
- four regions that inject `ATTMARKER` attribute bytes around error
  spans, which the GUI editor renders as colored highlights on the
  offending text. Terminal output has no attribute channel, so the
  unix build omits them. Cosmetic only; no validation difference.

## The substantive difference: CA-character word analysis

Two regions (around check.cpp 2988 and 3172, inside `check_CheckWords`)
run `uS.HandleCAChars` over every word: the Unicode
Conversation-Analysis symbol classifier (pitch shift arrows,
inhalation, creaky/whisper/singing voice, latching, hurried start,
sudden stop, vocative comma, stress marks, Hebrew/Arabic diacritics,
and the rest of the CA repertoire). Word-internal placement rules for
those symbols, including the CA paths of codes 48, 92, 93, and the
whole of 101 and 151, are enforced ONLY in the GUI build.

Why: this classifier belongs to the GUI's text-attribute machinery
(the same `ced.h` world that renders CA symbols specially); it was
never ported into the curses build. Consequence: a Mac CLANc user
gets word-level CA-symbol validation that a unix CHECK run of the
same file never performs. This is a genuine validation split inside
CLAN itself, and it predates us.

## What this means for chatter parity

- Codes reachable only through these regions (101, and 151's live
  path) are recorded in `dead-codes.json` as GUI-only: not parity
  targets under the unix-authoritative policy.
- chatter's own CA validation (e.g. E230 unbalanced CA delimiters)
  is independent of this and remains: chatter may legitimately check
  CA constructs BETTER than unix CHECK; that is ordinary
  chatter-stricter modernization.
- If the policy ever changes to include the GUI bar, grounding would
  need the Mac CLANc binary (per-host versions vary; see the meta
  repo CLAUDE.md on `/Applications/CLANc/`), not `clan-run.sh`.
