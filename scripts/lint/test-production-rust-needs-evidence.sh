#!/usr/bin/env bash
# Proves production-rust-needs-evidence.sh fires, and stays quiet when it
# should. A guard nobody has watched fail is a guard nobody has tested, so
# every case asserts a VERDICT rather than merely running the script.
#
# Runs against a throwaway repository, so it never inspects or depends on the
# state of the checkout it lives in.
set -euo pipefail

GUARD="$(cd "$(dirname "$0")" && pwd)/production-rust-needs-evidence.sh"
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

# Stage exactly the named files, having reset the index to HEAD first, so each
# case states its whole input rather than inheriting the previous one's.
stage_only() {
    ( cd "$work/repo" && git reset -q && git add "$@" )
}

git init -q "$work/repo"
cd "$work/repo"
git config user.email t@example.invalid
git config user.name t
mkdir -p crates/x/src crates/x/tests spec/errors corpus/reference
echo "# readme" > README.md
echo "fn seed() {}" > crates/x/src/lib.rs
git add -A
git commit -q -m "chore: seed"

# --- the core refusal ----------------------------------------------------
printf 'fn seed() {}\nfn added() { let _ = 1; }\n' > crates/x/src/lib.rs
stage_only crates/x/src/lib.rs
check "production rust with no evidence is refused" 1 "$(run_gate 'feat(x): add a function')"

# --- the four kinds of evidence ------------------------------------------
echo "#[test] fn t() {}" > crates/x/tests/it.rs
stage_only crates/x/src/lib.rs crates/x/tests/it.rs
check "a tests/ file is evidence" 0 "$(run_gate 'feat(x): add a function')"

printf 'fn seed() {}\nfn added() {}\n#[cfg(test)]\nmod tests { #[test] fn t() {} }\n' > crates/x/src/lib.rs
stage_only crates/x/src/lib.rs
check "an in-file #[cfg(test)] module is evidence" 0 "$(run_gate 'feat(x): add a function')"

printf 'fn seed() {}\nfn added2() {}\n' > crates/x/src/lib.rs
echo "# E999" > spec/errors/E999.md
stage_only crates/x/src/lib.rs spec/errors/E999.md
check "a spec file is evidence" 0 "$(run_gate 'feat(x): add a function')"

git rm -q --cached spec/errors/E999.md >/dev/null 2>&1 || true
echo "@UTF8" > corpus/reference/probe.cha
stage_only crates/x/src/lib.rs corpus/reference/probe.cha
check "a corpus file is evidence" 0 "$(run_gate 'feat(x): add a function')"

# --- the stated escape hatch ---------------------------------------------
printf 'fn seed() {}\nfn added3() {}\n' > crates/x/src/lib.rs
stage_only crates/x/src/lib.rs
check "a Red: trailer naming the red is accepted" 0 \
    "$(run_gate 'refactor(x): make it unrepresentable

Red: the compiler, at 14 call sites of Foo::new')"
check "an empty Red: trailer is refused"  1 "$(run_gate 'refactor(x): nope

Red:   ')"
check "Red: inside the subject is not a trailer" 1 "$(run_gate 'fix(x): Red: not here')"

# THE SUBJECT IS NOT LINE 1. An editor-authored message is preceded by git's
# own comment block, and a template can start with a blank line. A first cut
# used `tail -n +2`, so a single leading `#` shifted the SUBJECT into what the
# gate read as the body, and a subject beginning "Red:" was accepted as a
# trailer. The sibling gate has had `subject_of_msgfile` for this since it was
# written; two gates in one hook must agree where a body begins.
check "a subject beginning Red:, after a comment line, is not a trailer" 1 \
    "$(run_gate '# Please enter the commit message for your changes.
Red: this is the subject, not a trailer')"
check "a real trailer still passes with a comment block above it" 0 \
    "$(run_gate '# Please enter the commit message for your changes.
refactor(x): make it unrepresentable

Red: the compiler, at 14 call sites')"

# --- what is not production rust -----------------------------------------
echo "# more" >> README.md
stage_only README.md
check "docs alone need no evidence" 0 "$(run_gate 'docs: a paragraph')"

# COMMIT the current source first. `git diff --cached` is against HEAD, so a
# case about one file's own diff has to start from a commit; without this the
# comment-only diff still carried every earlier uncommitted edit, and the case
# passed for the wrong reason once the guard was written to be correct.
stage_only crates/x/src/lib.rs
git commit -q -m "chore: baseline for the comment-only case"
printf 'fn seed() {}\nfn added3() {}\n// a new comment\n' > crates/x/src/lib.rs
stage_only crates/x/src/lib.rs
check "a comment-only rust change needs no evidence" 0 "$(run_gate 'docs(x): explain it')"

mkdir -p crates/x/src
echo "// GENERATED" > crates/x/src/node_types.rs
stage_only crates/x/src/node_types.rs
check "a generated file is not authored production rust" 0 "$(run_gate 'chore: regenerate')"

# --- the near miss, which is what the previous gate's test taught ---------
# A path that CONTAINS "test" is not a test path. `attests.rs` is production
# code; a substring match would have excused it.
printf 'fn seed() {}\nfn attested() { let _ = 2; }\n' > crates/x/src/attests.rs
stage_only crates/x/src/attests.rs
check "a filename merely containing 'test' is production" 1 "$(run_gate 'feat(x): add it')"

# An assertion added to PRODUCTION code is not evidence of a red. The marker
# list is what a real test always brings with it; a bare `assert` was wider
# than the claim and let a `debug_assert!` in shipped code satisfy the gate.
#
# LAST on purpose: every case shares one repository, and this one rewrites
# `lib.rs`, so placing it earlier changed the baseline a later case measured
# against and broke it. Cases that mutate shared state go at the end.
printf 'fn seed() {}\nfn added4() { assert!(1 == 1); }\n' > crates/x/src/lib.rs
stage_only crates/x/src/lib.rs
check "a bare assert! in production code is not evidence" 1 "$(run_gate 'feat(x): add it')"

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
