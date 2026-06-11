from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"
PINNED_ACTION = re.compile(r"^\s*uses:\s*[^@\s]+@[0-9a-f]{40}\s*$", re.MULTILINE)


class GithubWorkflowGovernanceTests(unittest.TestCase):
    def test_all_external_actions_are_pinned_to_commit_sha(self) -> None:
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            text = workflow.read_text()
            uses_lines = [
                line
                for line in text.splitlines()
                if line.lstrip().startswith("uses:")
            ]
            self.assertTrue(uses_lines, f"{workflow.name} must use at least one action")
            for line in uses_lines:
                self.assertRegex(
                    line,
                    PINNED_ACTION,
                    f"{workflow.name} has an unpinned action: {line.strip()}",
                )

    def test_credentialed_workflow_only_runs_from_main(self) -> None:
        text = (WORKFLOWS / "credentialed-sdk.yml").read_text()
        self.assertIn("if: github.ref == 'refs/heads/main'", text)


if __name__ == "__main__":
    unittest.main()
