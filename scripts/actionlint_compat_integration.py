"""Real actionlint acceptance checks, run by ``just actionlint``.

Kept outside the generic pytest population: these require the actionlint
executable, just like the recipe that owns them.
"""

from __future__ import annotations

import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path

from actionlint_compat import Diagnostic, ProjectedWorkflow, WorkflowSnapshot, lint


CALLER = """name: Caller
on: push
jobs:
  call:
    uses: $/.github/workflows/callee.yml
    with:
      greeting: hello
"""
CALLEE = """name: Callee
on:
  workflow_call:
    inputs:
      greeting:
        type: string
        required: true
jobs:
  run:
    runs-on: ubuntu-latest
    steps:
      - run: echo '${{ inputs.greeting }}'
"""


class ActionlintCompatibilityTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        self.workflows = self.root / ".github/workflows"
        self.workflows.mkdir(parents=True)
        self.caller = self.workflows / "caller.yml"
        self.callee = self.workflows / "callee.yml"
        self.caller.write_text(CALLER)
        self.callee.write_text(CALLEE)

    def check(self) -> tuple[int, str]:
        before = {path: path.read_bytes() for path in self.workflows.iterdir()}
        output = io.StringIO()
        with contextlib.redirect_stderr(output), contextlib.redirect_stdout(output):
            result = lint(self.root)
        self.assertEqual(before, {path: path.read_bytes() for path in self.workflows.iterdir()})
        return result, output.getvalue()

    def test_valid_call_and_normal_workflow_pass_without_writes(self) -> None:
        self.assertEqual(self.check()[0], 0)
        self.caller.write_text(CALLER.replace("$/", "./"))
        self.assertEqual(self.check()[0], 0)

    def test_missing_callee_is_still_an_error(self) -> None:
        self.callee.unlink()
        result, output = self.check()
        self.assertEqual(result, 1)
        self.assertIn("callee.yml", output)

    def test_required_and_unknown_inputs_are_still_checked(self) -> None:
        for inputs in ("", "    with:\n      typo: hello\n"):
            with self.subTest(inputs=inputs):
                self.caller.write_text(CALLER.split("    with:")[0] + inputs)
                result, output = self.check()
                self.assertEqual(result, 1)
                self.assertIn("greeting", output)

    def test_unrelated_errors_in_projected_and_other_files_fail(self) -> None:
        self.caller.write_text(CALLER.replace("name: Caller", "unknown_key: Caller"))
        self.assertEqual(self.check()[0], 1)
        self.caller.write_text(CALLER)
        self.callee.write_text(CALLEE.replace("ubuntu-latest", "${{ absent.value }}"))
        self.assertEqual(self.check()[0], 1)

    def test_run_text_cannot_be_rewritten_into_success(self) -> None:
        self.callee.write_text(CALLEE.replace(
            "      - run: echo '${{ inputs.greeting }}'",
            "      - run: |\n          # uses: $/.github/workflows/callee.yml\n"
            "          echo '${{ absent.value }}'",
        ))
        self.assertEqual(self.check()[0], 1)

    def test_ref_and_traversal_are_not_projected(self) -> None:
        original = "$/.github/workflows/callee.yml"
        diagnostic = Diagnostic(
            message=(
                f'reusable workflow call "{original}" at "uses" is not following '
                'the format "owner/repo/path/to/workflow.yml@ref" nor "./path/to/workflow.yml". '
                "see https://docs.github.com/en/actions/learn-github-actions/reusing-workflows "
                "for more details"
            ),
            filepath=".github/workflows/caller.yml", line=5, column=11, kind="workflow-call",
        )
        # Admit the valid control first: a malformed diagnostic must not make
        # the refusal cases pass before reaching the reference boundary.
        ProjectedWorkflow.admit(WorkflowSnapshot.read(self.caller), [diagnostic])
        for reference in (
            "$/.github/workflows/callee.yml@main",
            "$/.github/workflows/../workflows/callee.yml",
        ):
            with self.subTest(reference=reference):
                self.caller.write_text(CALLER.replace(original, reference))
                changed = diagnostic.model_copy(update={
                    "message": diagnostic.message.replace(original, reference),
                })
                with self.assertRaises(ValueError):
                    ProjectedWorkflow.admit(WorkflowSnapshot.read(self.caller), [changed])


if __name__ == "__main__":
    unittest.main()
