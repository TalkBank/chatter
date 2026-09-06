#!/usr/bin/env bash
# Exercise the real pre-push protocol in an isolated local Git repository.
set -euo pipefail
source_root="$(git rev-parse --show-toplevel)"
fixture="$(mktemp -d)"
trap 'rm -r "$fixture"' EXIT
mkdir -p "$fixture/repo/scripts" "$fixture/empty-hooks"
cp "$source_root/scripts/tree-stamp.sh" "$fixture/repo/scripts/"
git init -q -b main "$fixture/repo"
cd "$fixture/repo"
git config user.name 'Gate Test'
git config user.email 'gate@example.invalid'
git config core.hooksPath "$fixture/empty-hooks"
printf 'old\n' > source.txt
git add -A
git commit -qm old
old="$(git rev-parse HEAD)"
printf 'fixed\n' > source.txt
bash scripts/tree-stamp.sh > .git/gate-passed
zero=0000000000000000000000000000000000000000
printf 'refs/heads/main %s refs/heads/main %s\n' "$old" "$zero" > "$fixture/refs"
if bash "$source_root/.githooks/pre-push" < "$fixture/refs" > "$fixture/output" 2>&1; then
    echo 'FAIL: uncommitted fixes authorized the older, ungated commit' >&2
    cat "$fixture/output" >&2
    exit 1
fi

git add source.txt
git commit -qm fixed
fixed="$(git rev-parse HEAD)"
printf 'refs/heads/main %s refs/heads/main %s\n' "$fixed" "$old" > "$fixture/refs"
bash "$source_root/.githooks/pre-push" < "$fixture/refs" > "$fixture/output" 2>&1
printf 'later edit\n' > source.txt
if bash "$source_root/.githooks/pre-push" < "$fixture/refs" > "$fixture/output" 2>&1; then
    echo 'FAIL: a later edit reused a stale receipt' >&2
    exit 1
fi
git restore source.txt
git tag -a tested -m tested
printf 'refs/tags/tested %s refs/tags/tested %s\n' "$(git rev-parse tested)" "$zero" > "$fixture/refs"
bash "$source_root/.githooks/pre-push" < "$fixture/refs" > "$fixture/output" 2>&1
printf 'refs/heads/older %s refs/heads/older %s\n' "$old" "$zero" >> "$fixture/refs"
if bash "$source_root/.githooks/pre-push" < "$fixture/refs" > "$fixture/output" 2>&1; then
    echo 'FAIL: a mixed push included an ungated tree' >&2
    exit 1
fi
# A changed tree during the gate cannot earn a receipt. Committing identical
# bytes is allowed; Git bookkeeping is not a source change.
# shellcheck source=scripts/gate-receipt.sh
source "$source_root/scripts/gate-receipt.sh"
gate_begin
printf 'changed during gate\n' > source.txt
if gate_finish > "$fixture/output" 2>&1; then
    echo 'FAIL: a gate recorded content changed during verification' >&2
    exit 1
fi
[[ ! -s .git/gate-passed ]]
git restore source.txt
gate_begin
gate_finish

# The optional local hook receives Git's original byte stream after validation.
export GATE_TEST_CAPTURE="$fixture/captured"
cat > .git/hooks/pre-push.local <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
cat > "$GATE_TEST_CAPTURE"
HOOK
chmod +x .git/hooks/pre-push.local
printf 'refs/heads/main %s refs/heads/main %s\n' "$fixed" "$old" > "$fixture/refs"
bash "$source_root/.githooks/pre-push" < "$fixture/refs" > "$fixture/output" 2>&1
cmp "$fixture/refs" "$fixture/captured"
echo 'gate receipts: exact push trees, tags, changed content and local-hook replay checked'
