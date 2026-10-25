import copy
import json
import tempfile
import unittest
from pathlib import Path
from strength_history import collect, record_gains


class StrengthHistoryTests(unittest.TestCase):
    def fixture(self, folder, pairs=4):
        manifest = dict(
            ruleset="chess",
            baseline_sha256="a",
            candidate_sha256="b",
            baseline_settings=dict(time_ms=1000),
            candidate_settings=dict(time_ms=1000),
            paired_game_count_predeclared=pairs * 2,
        )
        (folder / "manifest.json").write_text(json.dumps(manifest))
        rows = ["game\tpair\tside\tscore"]
        for game in range(pairs * 2):
            rows.append(f"{game}\t{game//2}\t{game%2+1}\t1")
        (folder / "games.tsv").write_text("\n".join(rows) + "\n")
        return manifest

    def test_tiny_all_win_pilot_is_not_a_gain_and_missing_pools_are_unrated(self):
        with tempfile.TemporaryDirectory() as root:
            folder = Path(root)
            self.fixture(folder)
            report = collect([folder], [dict(ruleset="chess"), dict(ruleset="go:19")])
            row = report["pools"]["chess"][0]
            self.assertEqual(row["delta_elo"]["point"], "+inf")
            self.assertFalse(row["supported_gain"])
            self.assertEqual(report["pools"]["go:19"], [])
            self.assertEqual(
                record_gains(dict(milestones=[]), report), dict(milestones=[])
            )

    def test_full_supported_schedule_is_recorded_once_but_partial_schedule_is_not(self):
        with tempfile.TemporaryDirectory() as root:
            folder = Path(root)
            manifest = self.fixture(folder, pairs=20)
            report = collect([folder], [])
            self.assertTrue(report["pools"]["chess"][0]["supported_gain"])
            history = record_gains(dict(milestones=[]), report)
            self.assertEqual(len(record_gains(history, report)["milestones"]), 1)
            recycled = copy.deepcopy(report)
            recycled["pools"]["chess"][0]["match_sha256"] = "different-run"
            with self.assertRaises(ValueError):
                record_gains(history, recycled)
            recycled["pools"]["chess"][0]["identities"]["baseline"] = report["pools"][
                "chess"
            ][0]["identities"]["candidate"]
            recycled["pools"]["chess"][0]["identities"]["candidate"]["id"] = "c"
            self.assertEqual(
                len(record_gains(copy.deepcopy(history), recycled)["milestones"]), 2
            )
            manifest["candidate_sha256"] = manifest["baseline_sha256"]
            (folder / "manifest.json").write_text(json.dumps(manifest))
            self.assertFalse(
                collect([folder], [])["pools"]["chess"][0]["supported_gain"]
            )
            manifest["candidate_sha256"] = "b"
            manifest["paired_game_count_predeclared"] = 100
            (folder / "manifest.json").write_text(json.dumps(manifest))
            self.assertFalse(
                collect([folder], [])["pools"]["chess"][0]["supported_gain"]
            )


if __name__ == "__main__":
    unittest.main()
