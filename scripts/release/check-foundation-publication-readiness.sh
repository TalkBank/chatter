#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
allow_dirty=()

if [[ "${1:-}" == "--allow-dirty" ]]; then
    allow_dirty=(--allow-dirty)
    shift
fi

if [[ $# -ne 0 ]]; then
    echo "usage: $0 [--allow-dirty]" >&2
    exit 1
fi

first_wave=(
    tree-sitter-talkbank
    talkbank-derive
    talkbank-model
    talkbank-cache
    talkbank-parser
    talkbank-parser-re2c
    talkbank-transform
)

held_back=(
    send2clan
    talkbank-cli
    talkbank-lsp
)

echo "==> Checking first-wave crates.io publication metadata"
python3 - "$repo_root" "$(IFS=,; echo "${first_wave[*]}")" "$(IFS=,; echo "${held_back[*]}")" <<'PY'
import json
import pathlib
import subprocess
import sys

repo_root = pathlib.Path(sys.argv[1])
first_wave = sys.argv[2].split(",")
held_back = sys.argv[3].split(",")

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=repo_root,
        text=True,
    )
)

packages = {package["name"]: package for package in metadata["packages"]}
workspace_names = set(packages)
errors: list[str] = []


def require(condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


required_fields = ("repository", "homepage", "keywords", "categories", "readme")

for package_name in first_wave:
    package = packages.get(package_name)
    require(package is not None, f"missing first-wave package {package_name}")
    if package is None:
        continue

    require(
        package.get("publish") != [],
        f"{package_name} is marked publish = false but is in the first-wave list",
    )

    for field in required_fields:
        value = package.get(field)
        require(bool(value), f"{package_name} is missing `{field}` metadata")
        if field == "readme" and value:
            readme_path = pathlib.Path(package["manifest_path"]).parent / value
            require(readme_path.is_file(), f"{package_name} readme path does not exist: {readme_path}")

    for dependency in package["dependencies"]:
        dep_name = dependency["name"]
        dep_path = dependency.get("path")
        dep_kind = dependency.get("kind")
        if dep_path and dep_kind is None and dep_name in workspace_names:
            require(
                dep_name in first_wave,
                f"{package_name} has a runtime dependency on held-back/internal crate {dep_name}",
            )

for package_name in held_back:
    package = packages.get(package_name)
    require(package is not None, f"missing held-back package {package_name}")
    if package is None:
        continue
    require(
        package.get("publish") == [],
        f"{package_name} must stay publish = false until the Wave 1B contract is ready",
    )

first_wave_index = {name: index for index, name in enumerate(first_wave)}
for package_name in first_wave:
    package = packages[package_name]
    for dependency in package["dependencies"]:
        dep_name = dependency["name"]
        dep_path = dependency.get("path")
        dep_kind = dependency.get("kind")
        if dep_path and dep_kind is None and dep_name in first_wave_index:
            require(
                first_wave_index[dep_name] < first_wave_index[package_name],
                f"first-wave order is wrong: {package_name} appears before its runtime dependency {dep_name}",
            )

if errors:
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    raise SystemExit(1)

print("First-wave order:")
for package_name in first_wave:
    print(f"  - {package_name}")

print("\nHold-back packages locked behind publish = false:")
for package_name in held_back:
    print(f"  - {package_name}")

print(
    "\nNote: only tree-sitter-talkbank can complete `cargo publish --dry-run` before the "
    "bootstrap crates exist on crates.io. The remaining first-wave crates are validated "
    "here via metadata/readme/runtime-dependency checks; their real registry-resolution "
    "smoke test happens as the wave is published in order."
)
PY

echo
echo "==> Checking package contents for first-wave crates"
for package_name in "${first_wave[@]}"; do
    echo "  - ${package_name}"
    cargo package --list -p "${package_name}" --locked "${allow_dirty[@]}" >/dev/null
done

echo
echo "==> Running standalone crates.io dry-run for tree-sitter-talkbank"
cargo publish --dry-run -p tree-sitter-talkbank --locked "${allow_dirty[@]}"
