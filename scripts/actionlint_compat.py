"""Lint native workflows, bridging actionlint issue #711 without ignoring errors.

cargo-dist 0.34 emits GitHub's same-repository ``$/`` workflow calls. Until
actionlint understands them, recheck affected files with a same-length ``./``
projection on stdin. The native checker still validates callees and inputs;
neither generated workflows nor actionlint's diagnostic policy are changed.
"""

from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from pydantic import BaseModel, ConfigDict, TypeAdapter


class Diagnostic(BaseModel):
    model_config = ConfigDict(strict=True, frozen=True)

    message: str
    filepath: str
    line: int
    column: int
    kind: str

    def render(self) -> str:
        return f"{self.filepath}:{self.line}:{self.column}: {self.message} [{self.kind}]"


@dataclass(frozen=True)
class WorkflowSnapshot:
    path: Path
    source: str

    @classmethod
    def read(cls, path: Path) -> WorkflowSnapshot:
        return cls(path, path.read_bytes().decode("utf-8"))

    def verify_unchanged(self) -> None:
        if self.path.read_bytes() != self.source.encode("utf-8"):
            raise ValueError(f"workflow changed during lint: {self.path}")


@dataclass(frozen=True)
class ProjectedWorkflow:
    original: WorkflowSnapshot
    source: str

    @classmethod
    def admit(
        cls, snapshot: WorkflowSnapshot, diagnostics: list[Diagnostic]
    ) -> ProjectedWorkflow:
        lines = snapshot.source.splitlines(keepends=True)
        replaced: set[int] = set()
        for diagnostic in diagnostics:
            if diagnostic.kind != "workflow-call" or not 1 <= diagnostic.line <= len(lines):
                continue
            # Only a native diagnostic at the start of a plain job-level uses
            # scalar authorizes this projection. Never rewrite run text/comments.
            index = diagnostic.line - 1
            match = re.fullmatch(
                r"(    uses: )(\$/\.github/workflows/[A-Za-z0-9_.-]+\.ya?ml)"
                r"([ \t]*(?:#.*)?\r?\n?)",
                lines[index],
            )
            if match is None or diagnostic.column != len(match[1]) + 1:
                continue
            expected = (
                f'reusable workflow call "{match[2]}" at "uses" is not following '
                'the format "owner/repo/path/to/workflow.yml@ref" nor "./path/to/workflow.yml". '
                "see https://docs.github.com/en/actions/learn-github-actions/reusing-workflows "
                "for more details"
            )
            if diagnostic.message != expected:
                continue
            lines[index] = match[1] + "." + match[2][1:] + match[3]
            replaced.add(index)
        if not replaced:
            raise ValueError("no unsupported same-repository workflow calls admitted")
        return cls(snapshot, "".join(lines))


def native(root: Path, *args: str, source: str | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["actionlint", *args], cwd=root, input=source, text=True,
        capture_output=True, check=False,
    )


def lint(root: Path) -> int:
    snapshots = {
        path.relative_to(root).as_posix(): WorkflowSnapshot.read(path)
        for path in sorted((root / ".github/workflows").glob("*"))
        if path.suffix in {".yml", ".yaml"} and path.is_file()
    }
    result = native(root, "-format", "{{json .}}")
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    failures: list[str] = []
    match result.returncode:
        case 0:
            pass
        case 1:
            diagnostics = TypeAdapter(list[Diagnostic]).validate_json(result.stdout)
            if not diagnostics:
                raise ValueError("actionlint failed without diagnostics")
            grouped: dict[str, list[Diagnostic]] = {}
            for diagnostic in diagnostics:
                grouped.setdefault(diagnostic.filepath, []).append(diagnostic)
            for path, errors in grouped.items():
                if path not in snapshots:
                    failures.extend(error.render() for error in errors)
                    continue
                try:
                    projection = ProjectedWorkflow.admit(snapshots[path], errors)
                except ValueError:
                    failures.extend(error.render() for error in errors)
                    continue
                projection.original.verify_unchanged()
                recheck = native(root, "-stdin-filename", path, "-", source=projection.source)
                if recheck.returncode != 0:
                    failures.append(recheck.stdout + recheck.stderr)
                print(f"actionlint: checked {path} using the read-only $/ compatibility projection")
        case _:
            failures.append(result.stdout or f"actionlint exited {result.returncode}")
    for snapshot in snapshots.values():
        snapshot.verify_unchanged()
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    return 0


def main() -> int:
    try:
        return lint(Path(__file__).resolve().parent.parent)
    except (OSError, ValueError) as error:
        print(f"actionlint compatibility check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
