#!/usr/bin/env bash
# Proves breaking-needs-changelog.sh fires, and that it stays quiet when it
# should. A guard nobody has watched fail is a guard nobody has tested, so
# every case here asserts a VERDICT rather than merely running the script.
#
# Runs against a throwaway repository, so it never inspects or depends on the
# state of the checkout it lives in.
set -euo pipefail

GUARD="$(cd "$(dirname "$0")" && pwd)/breaking-needs-changelog.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
pass=0
fail=0

check() {
    local name=$1 expected=$2 actual=$3
    if [ "$expected" = "$actual" ]; then
        pass=$((pass + 1))
    else
        fail=$((fail + 1))
        printf 'FAIL %s: expected exit %s, got %s\n' "$name" "$expected" "$actual" >&2
    fi
}

# `git commit` is never invoked: the gate reads the INDEX plus a message file,
# which is exactly what a commit-msg hook has, so driving it that way is the
# real seam rather than a simulation of it.
run_gate() {
    local msg=$1 rc=0
    printf '%s\n' "$msg" > "$work/msg"
    ( cd "$work/repo" && "$GUARD" --commit "$work/msg" ) >/dev/null 2>&1 || rc=$?
    echo "$rc"
}

git init -q "$work/repo"
cd "$work/repo"
git config user.email t@example.invalid
git config user.name t
echo "# Changelog" > CHANGELOG.md
echo "code" > src.rs
git add CHANGELOG.md src.rs
git commit -q -m "chore: seed"

# --- gate mode -----------------------------------------------------------
# Only src.rs staged: a breaking subject must be refused, an ordinary one not.
echo "changed" >> src.rs
git add src.rs
check "breaking without changelog is refused"        1 "$(run_gate 'feat(model)!: break it')"
check "breaking with scope-free marker is refused"   1 "$(run_gate 'feat!: break it')"
check "non-breaking without changelog is allowed"    0 "$(run_gate 'feat(model): add it')"
check "a bang later in the subject is not a marker"  0 "$(run_gate 'fix: it was broken!: really')"
check "a leading comment line is skipped"            1 "$(printf '#c\nfeat!: break it' > "$work/m2"; cd "$work/repo" && "$GUARD" --commit "$work/m2" >/dev/null 2>&1; echo $?)"

# With CHANGELOG.md staged too, the same breaking subject passes. This is the
# case that proves the gate reads the index rather than always refusing.
echo "- entry" >> CHANGELOG.md
git add CHANGELOG.md
check "breaking with changelog is allowed"           0 "$(run_gate 'feat(model)!: break it')"

# A LARGE staged diff must not defeat the check.
#
# `touches_changelog` was `git diff --name-only | grep -qx CHANGELOG.md` under
# `set -o pipefail`. `grep -q` exits at the FIRST match and closes the pipe; on
# a big enough file list `git` is still writing, dies on SIGPIPE (141), and
# pipefail propagates that as the pipeline's status. So the gate reported "does
# not touch CHANGELOG.md" about a diff that plainly did.
#
# It fired on exactly one commit in the project's life: the 1000-file 0.16.0
# release squash, which is the commit the gate exists to protect. Every
# ordinary commit is small enough that git finishes writing before grep exits.
# The pipe buffer is 64 KiB, so the reproduction needs the NAME LIST to exceed
# it, not merely to be long. Deep paths, as the real squash had.
deep="crates/talkbank-parser-tests/tests/error_corpus/validation_errors/fixtures"
mkdir -p "$work/repo/$deep"
for i in $(seq 1 1200); do
    printf 'x\n' > "$work/repo/$deep/a_reasonably_long_fixture_name_$i.cha"
done
( cd "$work/repo" && git add -A )
check "a breaking commit with a LARGE staged diff still sees the changelog" 0 \
    "$(run_gate 'feat(model)!: break it')"

# --- report mode ---------------------------------------------------------
git commit -q -m "feat(model)!: break it"
base=$(git rev-parse HEAD)
echo "more" >> src.rs
git add src.rs
git commit -q -m "feat(parser)!: break it again"
out=$("$GUARD" --since "$base")
case "$out" in
    *"1 breaking commit(s)"*) pass=$((pass + 1)) ;;
    *) fail=$((fail + 1)); printf 'FAIL report: got %s\n' "$out" >&2 ;;
esac

# A path that CONTAINS the changelog's name is not the changelog. The report
# used to grep a `--stat` table for a substring, so this counted as an entry
# and a non-compliant history reported clean.
base2=$(git rev-parse HEAD)
echo "note" > CHANGELOG.md.bak
git add CHANGELOG.md.bak
git commit -q -m "feat(x)!: break it, recording nothing"
out=$("$GUARD" --since "$base2")
case "$out" in
    *"1 breaking commit(s)"*) pass=$((pass + 1)) ;;
    *) fail=$((fail + 1))
       printf 'FAIL near-miss path counted as a changelog entry: %s\n' "$out" >&2 ;;
esac

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
