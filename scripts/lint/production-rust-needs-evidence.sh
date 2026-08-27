#!/usr/bin/env bash
#
# Refuse a commit that adds or changes production Rust while staging no
# evidence that the change was driven by a red.
#
# # Why this exists
#
# THE LOOP says "red first (a type change or a failing test), then green", and
# the every-touch rule says a commit must improve the types or the tests of
# every file it touches. Both were prose, and on 2026-08-26/27 three commits in
# one session each shipped a bug their own later review found. Prose loses to
# momentum; a refusal does not.
#
# # What it is not
#
# It is not a measure of test QUALITY, and it cannot be. It answers one crude
# question: did this change come with anything that could have been red? A
# commit can satisfy it and still be badly tested. The point is that a commit
# CANNOT satisfy it silently, which is what the skipped red steps were.
#
# # The four kinds of evidence, and why a spec file counts
#
#   * a file under a `tests/` or `benches/` directory
#   * a `#[test]`, `#[cfg(test)]` or similar marker ADDED by this diff, which
#     is how most of this repo's unit tests live: inside the file they test
#   * a file under `spec/`. In this repo the spec system IS the test corpus: a
#     construct or parser bug is fixed by writing the spec first, and `just
#     regen` turns it into fixtures. A spec example is a failing test.
#   * a file under `corpus/` or `grammar/test/`, the two other fixture homes
#
# # The escape hatch, which is the every-touch rule's own words
#
# That rule already says: if the edit genuinely admits neither a type nor a
# test, "say so in the commit and say why". So the way past this gate is to say
# so, in a `Red:` trailer on its own line in the message body, naming what was
# red before the change:
#
#     Red: the compiler, at 14 call sites of Word::new
#     Red: nothing. A pure deletion; it removes the only caller of X.
#
# A trailer is not a bypass flag. It is recorded in the history, it names a
# claim someone can check, and writing one is exactly the sentence the doctrine
# asks for. An EMPTY trailer is refused, because "Red:" alone asserts nothing.
#
# # Usage
#
#     production-rust-needs-evidence.sh --commit <message-file>
#
# Reads the INDEX, which is what a commit-msg hook has in hand, so it judges
# what is about to be committed rather than what is lying around in the tree.
set -euo pipefail

usage() {
    echo "usage: $0 --commit <message-file>" >&2
    exit 64
}

[ $# -eq 2 ] || usage
[ "$1" = "--commit" ] || usage
MSG_FILE="$2"
[ -f "$MSG_FILE" ] || { echo "$0: no such message file: $MSG_FILE" >&2; exit 64; }

# Generated Rust is not authored production Rust: regenerating it is the output
# of a change elsewhere, and `just regen` rewrites all of it at once. The names
# come from the DO-NOT-EDIT set listed in CLAUDE.md's danger rules.
is_generated_path() {
    local path=$1
    local base=${path##*/}
    case "$base" in
        node_types.rs|generated_traversal.rs|generated_*.rs) return 0 ;;
    esac
    case "$path" in
        */generated/*|generated/*) return 0 ;;
    esac
    return 1
}

# A path is EVIDENCE when a whole path COMPONENT says so, never when a name
# merely CONTAINS the word: `src/attests.rs` is production code, and a
# substring match would have excused it. Wrapping in slashes is what makes the
# component match whole: `/crates/x/src/attests.rs/` holds no `/tests/`.
is_evidence_path() {
    case "${1##*/}" in tests.rs | *_tests.rs | *_test.rs) return 0 ;; esac
    case "/$1/" in */tests/* | */benches/*) return 0 ;; esac
    case "$1" in spec/* | corpus/* | grammar/test/*) return 0 ;; esac
    return 1
}

# A change is SUBSTANTIVE when the staged diff for it touches a line that is
# neither blank nor a `//` comment. Retitling a doc comment is not the writing
# of production code this gate is about, and demanding a trailer for one would
# breed trailers nobody reads.
has_substantive_change() {
    local path=$1
    git diff --cached --unified=0 -- "$path" \
        | grep -E '^[+-]' \
        | grep -Ev '^(\+\+\+|---)' \
        | grep -Evq '^[+-][[:space:]]*(//.*)?$'
}

mapfile -t staged < <(git diff --cached --name-only --diff-filter=ACMR)

# EVIDENCE FIRST, because it is free. These are pure path tests with no
# subprocess, while `has_substantive_change` costs a `git diff` per file (about
# 20 ms each). The first cut ran that for every candidate and then discarded
# the list whenever evidence turned up, which is the common case in this repo:
# most commits stage a test or a spec beside the code.
evidence=no
for path in "${staged[@]}"; do
    if is_evidence_path "$path"; then
        evidence=yes
        break
    fi
done

# A test marker ADDED anywhere in this diff is evidence too, wherever it lives.
# Most of this repo's unit tests are a `mod tests` inside the file they test,
# so a per-path rule alone would refuse the commonest correct shape.
if [ "$evidence" = no ]; then
    if git diff --cached --unified=0 -- '*.rs' \
        | grep -E '^\+' \
        | grep -Ev '^\+\+\+' \
        | grep -Eq '#\[test\]|#\[cfg\(test\)\]|#\[tokio::test\]|#\[should_panic|\bmod tests\b|proptest!'
    then
        evidence=yes
    fi
fi

[ "$evidence" = no ] || exit 0

# Only now, with no evidence anywhere, is it worth asking which files are
# production Rust: this is the branch that pays a `git diff` per path.
production=()
for path in "${staged[@]}"; do
    case "$path" in
        *.rs) ;;
        *) continue ;;
    esac
    is_generated_path "$path" && continue
    has_substantive_change "$path" || continue
    production+=("$path")
done

[ "${#production[@]}" -gt 0 ] || exit 0

# The trailer must be its OWN line in the body. A subject that happens to begin
# "Red:" is a subject, not a claim about what was red, and treating it as one
# would make the hatch openable by accident.
#
# Finding the body means finding the SUBJECT first, and the subject is not
# line 1: an editor-authored message can be preceded by comment lines and a
# template can start with a blank one. A first cut used `tail -n +2`, which a
# single leading `#` defeats, and the sibling gate in this same hook has had
# `subject_of_msgfile` for exactly this reason since it was written. Two gates
# in one hook disagreeing about where a message body begins is its own defect,
# so this uses the sibling's rule.
body_of_msgfile() {
    grep -v '^#' "$1" | grep -v '^[[:space:]]*$' | tail -n +2
}

if body_of_msgfile "$MSG_FILE" | grep -Eq '^Red:[[:space:]]*[^[:space:]]'; then
    exit 0
fi

cat >&2 <<MESSAGE
[commit] REFUSED: production Rust changed with no evidence of a red.

Changed, with no test, spec, corpus or fixture staged beside it:
$(printf "    %s\n" "${production[@]}")

THE LOOP is red first, then green. Three ways forward, in preference order:

  1. Make the change a TYPE change the compiler refuses before it, and say so
     in the message:   Red: the compiler, at N call sites of X
  2. Stage the failing test or the spec example that was red. In this repo a
     spec file under spec/ IS the failing test; \`just regen\` makes it fixtures.
  3. If the edit genuinely admits neither, say so and say why, in a trailer on
     its own line in the message body:

         Red: nothing. A pure deletion; it removes the only caller of X.

This is not a bypass flag: the trailer is recorded in the history and names a
claim a reader can check.
MESSAGE
exit 1
