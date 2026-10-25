import copy
import unittest
from matrix_report import compact


class MatrixReportTests(unittest.TestCase):
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
