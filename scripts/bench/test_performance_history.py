import copy
import unittest
from performance_history import accept, throughput_range


class HistoryTests(unittest.TestCase):
    def test_chained_point_spread_uses_its_own_timing_scale(self):
        # A later run can have a different clock/load than the first baseline.
        # Its normalized 2x point must not get whiskers around an absolute 10x.
        low, high = throughput_range(
            dict(
                relative_throughput=2.0,
                timings={"fixture": dict(median_ms=100, min_ms=90, max_ms=110)},
            )
        )
        self.assertAlmostEqual(low, 2 * 100 / 110)
        self.assertAlmostEqual(high, 2 * 100 / 90)
        self.assertLessEqual(low, 2)
        self.assertGreaterEqual(high, 2)

    def report(self):
        return dict(
            settings=dict(ruleset="chess", depth=3, nodes=100000, cpu=4),
            platform="fixture",
            baseline_sha256="old",
            candidate_sha256="new",
            summaries=[
                dict(
                    fixture="benchmarks/positions/chess/central-knights.pgn",
                    same_work_and_answer=True,
                    baseline=dict(median_ms=100, min_ms=95, max_ms=105),
                    candidate=dict(median_ms=50, min_ms=45, max_ms=55),
                )
            ],
        )

    def test_accepts_improvement_and_rejects_regression_or_changed_work(self):
        report = self.report()
        history = accept(None, report, "baseline", "candidate", "2026-10-24")
        self.assertEqual(history["points"][-1]["relative_throughput"], 2)
        changed = copy.deepcopy(report)
        changed["summaries"][0]["same_work_and_answer"] = False
        with self.assertRaises(ValueError):
            accept(None, changed, "a", "b", "date")
        changed = copy.deepcopy(report)
        changed["summaries"][0]["candidate"]["max_ms"] = 96
        with self.assertRaises(ValueError):
            accept(None, changed, "a", "b", "date")
        with self.assertRaises(ValueError):
            accept(history, report, "a", "b", "date")


if __name__ == "__main__":
    unittest.main()
