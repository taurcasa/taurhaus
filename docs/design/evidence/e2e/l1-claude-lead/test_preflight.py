"""Offline lane safety tests: fabricated help only, no CLI/auth/network access."""
import unittest

from preflight import budget_prerequisite


class BudgetPrerequisiteTests(unittest.TestCase):
    def test_print_only_limit_cannot_authorize_managed_tui_or_compact(self):
        result = budget_prerequisite(
            "  --max-budget-usd <amount> Maximum dollar amount to spend on API calls (only works with --print)\n"
        )
        self.assertFalse(result["launch_allowed"])
        self.assertEqual(result["classification"], "harness")
        self.assertIn("print", result["reason"])

    def test_absent_budget_evidence_is_unavailable_not_zero_cost(self):
        result = budget_prerequisite("Usage: claude [options]\n")
        self.assertFalse(result["launch_allowed"])
        self.assertIn("unverified", result["reason"])

    def test_unqualified_help_is_not_independent_compact_enforcement_proof(self):
        result = budget_prerequisite("  --max-budget-usd <amount> Maximum dollars\n")
        self.assertFalse(result["launch_allowed"])
        self.assertIn("unverified", result["reason"])

    def test_retains_only_relevant_help_evidence(self):
        line = "  --max-budget-usd <amount> Budget (only works with --print)"
        result = budget_prerequisite("irrelevant\n" + line + "\nother\n")
        self.assertEqual(result["help_excerpt"], [line])

    def test_wrapped_help_preserves_print_only_qualification(self):
        lines = ["  --max-budget-usd <amount> Maximum dollar amount to spend on API",
                 "                            calls (only works with --print)"]
        result = budget_prerequisite("\n".join(lines) + "\n  --model <model> Model\n")
        self.assertEqual(result["help_excerpt"], lines)
        self.assertIn("only for --print", result["reason"])


if __name__ == "__main__":
    unittest.main()
