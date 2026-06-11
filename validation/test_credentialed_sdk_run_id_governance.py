from __future__ import annotations

import ast
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VALIDATION = ROOT / "validation"


class CredentialedSdkRunIdGovernanceTests(unittest.TestCase):
    def test_all_cli_arguments_require_explicit_credentialed_sdk_run_id(self) -> None:
        declarations: list[tuple[Path, ast.Call]] = []
        for path in sorted(VALIDATION.glob("*.py")):
            tree = ast.parse(path.read_text(), filename=str(path))
            for node in ast.walk(tree):
                if not isinstance(node, ast.Call):
                    continue
                if not any(
                    isinstance(arg, ast.Constant)
                    and arg.value == "--credentialed-sdk-run-id"
                    for arg in node.args
                ):
                    continue
                declarations.append((path, node))

        self.assertTrue(declarations, "credentialed SDK CLI declarations must exist")
        for path, declaration in declarations:
            keywords = {keyword.arg: keyword.value for keyword in declaration.keywords}
            required = keywords.get("required")
            self.assertIsInstance(
                required,
                ast.Constant,
                f"{path.name} must declare required=True",
            )
            self.assertIs(
                required.value,
                True,
                f"{path.name} must require an explicit credentialed SDK run id",
            )
            self.assertNotIn(
                "default",
                keywords,
                f"{path.name} must not carry a stale credentialed SDK run id default",
            )


if __name__ == "__main__":
    unittest.main()
