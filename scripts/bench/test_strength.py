import math
import unittest
from strength import elo, interval, summarize, settings_for


class StrengthTests(unittest.TestCase):
    def test_logistic_scale(self):
        self.assertEqual(elo(0.5), 0)
        self.assertAlmostEqual(elo(10 / 11), 400)
        self.assertEqual(elo(0), -math.inf)
        self.assertEqual(elo(1), math.inf)

    def test_extreme_bound_has_no_fake_pseudocount(self):
        low, high = interval([0] * 20)
        self.assertEqual(low, 0)
        self.assertAlmostEqual(high, 1 - 0.025 ** (1 / 20))
        self.assertEqual(interval([]), (0, 1))

    def test_pairing_and_unfinished_exclusion(self):
        rows = [
            {"pair": i // 2, "score": s}
            for i, s in enumerate(["1", "0", "0.5", "0.5", "0", "0", "1", "NA"])
        ]
        result = summarize(rows, 1320)
        self.assertEqual(result["pentanomial"], [1, 0, 2, 0, 0])
        self.assertEqual(result["complete_pairs"], 3)
        self.assertEqual(result["excluded_pairs"], 1)
        self.assertEqual(result["wdl"], [2, 2, 3])

    def test_interval_symmetry_and_more_data(self):
        a, b = interval([0.5] * 10)
        self.assertAlmostEqual(a, 1 - b)
        c, d = interval([0.5] * 100)
        self.assertGreater(c, a)
        self.assertLess(d, b)


if __name__ == "__main__":
    unittest.main()


class BudgetTests(unittest.TestCase):
    def test_asymmetric_budgets_are_explicit(self):
        from argparse import Namespace

        args = Namespace(
            depth=32,
            nodes=50000000,
            time_ms=1000,
            baseline_depth=None,
            baseline_nodes=None,
            baseline_time_ms=1000,
            candidate_depth=None,
            candidate_nodes=None,
            candidate_time_ms=10000,
        )
        self.assertEqual(
            settings_for(args, False), dict(depth=32, nodes=50000000, time_ms=1000)
        )
        self.assertEqual(
            settings_for(args, True), dict(depth=32, nodes=50000000, time_ms=10000)
        )
