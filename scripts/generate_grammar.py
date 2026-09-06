#!/usr/bin/env python3
"""Generate grammar artifacts in staging, then check or publish changed files."""

import argparse
import filecmp
from pathlib import Path
import subprocess
import sys
import tempfile

from generate_if_changed import publish

GRAMMAR = Path(__file__).resolve().parents[1] / "grammar"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="report staleness without writing")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(dir=GRAMMAR, prefix=".generated-") as directory:
        staged = Path(directory)
        result = subprocess.run(
            ["tree-sitter", "generate", "--output", str(staged)], cwd=GRAMMAR,
            check=False,
        )
        if result.returncode:
            return result.returncode if result.returncode > 0 else 128 - result.returncode
        for required in ("parser.c", "grammar.json", "node-types.json"):
            if not (staged / required).is_file():
                print(f"generator omitted {required}", file=sys.stderr)
                return 1
        stale = False
        for source in sorted(path for path in staged.rglob("*") if path.is_file()):
            relative = source.relative_to(staged)
            target = GRAMMAR / "src" / relative
            if args.check:
                if not target.is_file() or not filecmp.cmp(source, target, shallow=False):
                    print(f"stale grammar/src/{relative}; run just regen", file=sys.stderr)
                    stale = True
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                publish(source, target)
        return int(stale)


if __name__ == "__main__":
    raise SystemExit(main())
