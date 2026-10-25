import json
import tempfile
import unittest
from pathlib import Path
from league import entrant, league, update_pair


class LeagueTests(unittest.TestCase):
    def test_pair_update_is_zero_sum_and_color_order_independent(self):
        self.assertEqual(update_pair(1000, 1000, 0.5, 20), (1000, 1000))
        self.assertEqual(update_pair(1000, 1000, 1, 20), (1020, 980))
        a, b = update_pair(1200, 1000, 0.5, 20)
        self.assertAlmostEqual(a + b, 2200)
        self.assertLess(a, 1200)
        self.assertGreater(b, 1000)

    def test_time_budgets_distinguish_entrants_with_the_same_binary(self):
        manifest = dict(
            baseline_sha256="same",
            candidate_sha256="same",
            baseline_settings=dict(time_ms=1000),
            candidate_settings=dict(time_ms=10000),
        )
        self.assertNotEqual(
            entrant(manifest, "baseline")[0], entrant(manifest, "candidate")[0]
        )

    def test_complete_pairs_only_and_separate_rulesets(self):
        with tempfile.TemporaryDirectory() as root:
            matches = []
            for ruleset in ["chess", "go"]:
                folder = Path(root) / ruleset
                folder.mkdir()
                manifest = dict(
                    ruleset=ruleset,
                    baseline_sha256="a",
                    candidate_sha256="b",
                    baseline_settings=dict(time_ms=1000),
                    candidate_settings=dict(time_ms=10000),
                )
                (folder / "manifest.json").write_text(json.dumps(manifest))
                (folder / "games.tsv").write_text(
                    "game\tpair\tside\tscore\n0\t0\t1\t1\n1\t0\t2\t1\n2\t1\t1\t1\n3\t1\t2\tNA\n"
                )
                matches.append(folder)
            result = league(matches)
            self.assertEqual(set(result["pools"]), {"chess", "go:9"})
            for pool in result["pools"].values():
                self.assertEqual(
                    sorted(e["rating"] for e in pool["entrants"].values()), [980, 1020]
                )
                self.assertEqual(pool["matches"][0]["accepted_pairs"], 1)
                self.assertEqual(pool["matches"][0]["excluded_pairs"], 1)
            with self.assertRaises(ValueError):
                league([matches[0], matches[0]])


if __name__ == "__main__":
    unittest.main()
