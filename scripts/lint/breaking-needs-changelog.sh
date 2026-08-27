#!/usr/bin/env bash
# breaking-needs-changelog.sh: a commit marked BREAKING must edit CHANGELOG.md.
#
# WHY THIS EXISTS
#
# Measured on 2026-08-26 over the 53 commits since v0.15.0: SEVEN carried the
# Conventional Commits breaking marker (`type(scope)!: subject`) and NONE of
# them touched CHANGELOG.md. Two were written up later in a separate docs
# commit; four were never written up at all, and were found only because
# somebody read the log by hand while preparing a release. Among the four was
# a user-visible parser fix and a public API that had been made private one day
# after the CHANGELOG told callers to use it.
#
# Deferring the entry is what loses it. The commit that makes the break is the
# only moment when the author knows what broke and why, so that is where the
# obligation belongs.
#
# TWO MODES, AND THE SPLIT IS DELIBERATE
#
#   --commit MSGFILE   GATE. Tests THIS commit: if its subject is marked
#                      breaking, CHANGELOG.md must be among its staged paths.
#                      Decidable, local, and cannot go red because of something
#                      that happened elsewhere.
#
#   --since REF        REPORT. Names every breaking commit after REF that did
#                      not touch CHANGELOG.md. Its verdict depends on history
#                      rather than on one commit, so it is deliberately NOT a
#                      gate: run it when preparing a release and read it.
#
# WHAT IT DOES NOT CHECK, stated because a guard that overclaims is worse than
# none: that the entry is CORRECT, that it describes THIS break, or that an
# unmarked commit was non-breaking. It checks that the author opened the file.
set -euo pipefail

CHANGELOG="CHANGELOG.md"

# Conventional Commits: `type` or `type(scope)` then a `!` then the colon.
# Anchored at the start so a `!` inside prose ("it was not!: really") cannot
# match, and the scope is restricted to non-paren characters so a subject
# containing parentheses later on cannot be dragged into it.
BREAKING_SUBJECT='^[a-zA-Z]+(\([^)]*\))?!:'

# Does a commit, or the staged index, touch the changelog?
#
# ONE predicate for both modes. They used to ask differently: the gate matched
# a whole path line from `--name-only`, the report matched a SUBSTRING of a
# `--stat` table. `--stat` is a display format that column-pads and abbreviates
# long paths with a leading `...`, so the report counted `docs/CHANGELOG.md.bak`
# as an entry and could miss a real one. A report whose failure mode is a false
# clean is the shape this repository's guard rules single out.
# Whether the named commit (or the index, with `--staged`) touches the
# changelog.
#
# `grep -x`, deliberately NOT `grep -qx`, and that is the whole subtlety.
#
# `-q` makes grep exit at the FIRST match and close the pipe. When the file
# list is longer than the 64 KiB pipe buffer the writer is still writing, dies
# on SIGPIPE, and `set -o pipefail` reports the pipeline as 141. The gate then
# announces "does not touch CHANGELOG.md" about a diff that plainly does.
#
# It fired exactly once: the 1000-file 0.16.0 release squash, whose name list
# is 137 KB. That is the commit this gate exists to protect, and every ordinary
# commit is small enough that the writer finishes before grep exits, so the
# defect was invisible for the gate's whole life. Buffering the list into a
# variable first does NOT fix it, because `-q` still closes the pipe on the
# `printf`; only reading the input to the end does.
touches_changelog() {
    local names
    case $1 in
        --staged) names=$(git diff --cached --name-only) ;;
        *) names=$(git show --name-only --format= "$1") ;;
    esac
    printf '%s\n' "$names" | grep -x "$CHANGELOG" >/dev/null
}

usage() {
    echo "usage: $0 --commit MSGFILE | --since REF" >&2
    exit 2
}

# Reads the first non-comment, non-blank line of a commit message file. `git`
# writes the subject first, but an editor-authored message can be preceded by
# comment lines, and a template can start with a blank one.
subject_of_msgfile() {
    grep -v '^#' "$1" | grep -m1 -v '^[[:space:]]*$' || true
}

gate_commit() {
    local msgfile=$1 subject
    [ -f "$msgfile" ] || { echo "no such message file: $msgfile" >&2; exit 2; }
    subject=$(subject_of_msgfile "$msgfile")

    if ! printf '%s' "$subject" | grep -Eq "$BREAKING_SUBJECT"; then
        return 0
    fi
    if touches_changelog --staged; then
        return 0
    fi

    cat >&2 <<MSG
BLOCKED: this commit is marked BREAKING and does not touch $CHANGELOG.

  subject: $subject

A breaking change that is written up "later" is usually not written up at all:
of the seven breaking commits after v0.15.0, four were never recorded, and one
of those made public an API the CHANGELOG had just told callers to use.

Add the entry under "## [Unreleased]" and stage $CHANGELOG, or drop the "!"
if the change is not in fact breaking. There is deliberately no bypass flag.
MSG
    exit 1
}

report_since() {
    local ref=$1 missing=0 sha subject
    git rev-parse --verify --quiet "$ref" >/dev/null \
        || { echo "no such ref: $ref" >&2; exit 2; }

    # Read the log as NUL-free lines of "<sha> <subject>"; a subject cannot
    # contain a newline, so a plain read loop is exact here.
    while read -r sha subject; do
        printf '%s' "$subject" | grep -Eq "$BREAKING_SUBJECT" || continue
        if touches_changelog "$sha"; then
            continue
        fi
        printf '  %s  %s\n' "$sha" "$subject"
        missing=$((missing + 1))
    done < <(git log --format='%h %s' "$ref"..HEAD)

    if [ "$missing" -eq 0 ]; then
        echo "every breaking commit after $ref touched $CHANGELOG"
    else
        echo "$missing breaking commit(s) after $ref did not touch $CHANGELOG (listed above)"
    fi
}

[ $# -eq 2 ] || usage
case "$1" in
    --commit) gate_commit "$2" ;;
    --since) report_since "$2" ;;
    *) usage ;;
esac
