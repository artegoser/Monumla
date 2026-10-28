import copy
import unittest
from matrix_report import compact, record_gains


class MatrixReportTests(unittest.TestCase):
    def test_only_fixed_work_nonoverlapping_gains_enter_history(self):
        native, wasm = self.matrices()
        wasm["results"][0].update(
            candidate=dict(median_ms=50, min_ms=45, max_ms=55), speedup=2
        )
        report = compact(native, wasm)
        history = record_gains(dict(schema=2, series={}), report)
        self.assertEqual(set(history["series"]), {"wasm-fixture"})
        self.assertEqual(
            history["series"]["wasm-fixture"]["points"][-1]["relative_throughput"], 2
        )
        with self.assertRaises(ValueError):
            record_gains(history, report)

    def test_keeps_failed_wasm_cases_and_does_not_journal_regressions(self):
        native, wasm = self.matrices()
        wasm["results"][0] = dict(case=wasm["results"][0]["case"], error="failure")
        report = compact(native, wasm)
        self.assertEqual(report["results"][1]["error"], "failure")
        self.assertEqual(record_gains(dict(series={}), report), dict(series={}))

    def test_new_build_identity_starts_a_separate_gain_series(self):
        native, wasm = self.matrices()
        native["settings"]["build_id"] = "stable-rust1.99.1"
        with self.assertRaisesRegex(ValueError, "build identity"):
            compact(native, wasm)
        wasm["settings"]["build-id"] = "stable-rust1.99.1"
        wasm["results"][0].update(
            candidate=dict(median_ms=50, min_ms=45, max_ms=55), speedup=2
        )
        report = compact(native, wasm)
        history = dict(schema=2, series={"wasm-fixture": dict(legacy="untouched")})
        updated = record_gains(history, report)
        self.assertEqual(updated["series"]["wasm-fixture"], dict(legacy="untouched"))
        name = "wasm-fixture-stable-rust1.99.1"
        self.assertEqual(updated["series"][name]["build_id"], "stable-rust1.99.1")
        self.assertEqual(len(updated["series"][name]["points"]), 2)
        with self.assertRaises(ValueError):
            record_gains(updated, report)

    def test_mismatched_warmup_counts_cannot_be_published(self):
        native, wasm = self.matrices()
        native["settings"]["warmups"] = 5
        with self.assertRaisesRegex(ValueError, "warmup count"):
            compact(native, wasm)
        wasm["settings"]["warmups"] = "5"
        report = compact(native, wasm)
        self.assertEqual(report["warmups"], 5)
        self.assertIn("5 warmups", report["method"])
        native["settings"]["warmup_ms"] = 1000
        with self.assertRaisesRegex(ValueError, "warmup duration"):
            compact(native, wasm)
        wasm["settings"]["warmup-ms"] = "1000"
        self.assertEqual(compact(native, wasm)["warmup_ms"], 1000)

    def matrices(self):
        case = dict(id="fixture", ruleset="chess", depth=3, nodes=100)
        result = dict(
            same_work_and_answer=True,
            baseline=dict(median_ms=100, min_ms=90, max_ms=110),
            candidate=dict(median_ms=120, min_ms=115, max_ms=125),
            speedup=100 / 120,
        )
        native = dict(
            settings=dict(baseline_revision="a", candidate_revision="b", repeats=5),
            baseline_sha256="one",
            candidate_sha256="two",
            results=[dict(case=case, summaries=[result], returncode=0)],
        )
        wasm = dict(
            source_revisions=dict(baseline="a", candidate="b"),
            wasm_sha256=dict(baseline="three", candidate="four"),
            runtime=dict(node="test"),
            settings=dict(repeats="5"),
            cpu="test",
            results=[dict(case=case, **result)],
        )
        return native, wasm

    def test_keeps_regressions_and_rejects_mixed_revisions_or_coverage(self):
        native, wasm = self.matrices()
        report = compact(native, wasm)
        self.assertEqual(len(report["results"]), 2)
        self.assertFalse(any(row["verified_gain"] for row in report["results"]))
        changed = copy.deepcopy(wasm)
        changed["source_revisions"]["candidate"] = "other"
        with self.assertRaises(ValueError):
            compact(native, changed)
        changed = copy.deepcopy(wasm)
        changed["results"] = []
        with self.assertRaises(ValueError):
            compact(native, changed)


if __name__ == "__main__":
    unittest.main()
