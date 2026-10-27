import copy
import json
import tempfile
import unittest
from pathlib import Path
from strength_matrix_report import verify_matrix


class StrengthMatrixReportTests(unittest.TestCase):
    def fixture(self, root):
        case = dict(id="chess", ruleset="chess", games=2, time_ms=500, max_plies=300)
        matrix = dict(
            started_at="date",
            settings=dict(
                baseline_revision="old", candidate_revision="new", opening_plies=2
            ),
            baseline_sha256="a",
            candidate_sha256="b",
            plan=[case],
            results=[dict(case=case, returncode=0)],
        )
        folder = root / "chess"
        folder.mkdir()
        manifest = dict(
            ruleset="chess",
            games=2,
            max_plies=300,
            opening_plies=2,
            baseline_sha256="a",
            candidate_sha256="b",
            baseline_revision="old",
            candidate_revision="new",
            baseline_settings=dict(depth=32, nodes=50000000, time_ms=500),
            candidate_settings=dict(depth=32, nodes=50000000, time_ms=500),
        )
        (folder / "manifest.json").write_text(json.dumps(manifest))
        (folder / "games.tsv").write_text(
            "game\tpair\tside\tscore\n1\t0\t1\t0.5\n2\t0\t2\t0.5\n"
        )
        (root / "matrix.json").write_text(json.dumps(matrix))
        return matrix, manifest

    def test_rejects_incomplete_coverage_and_mixed_binary_or_control(self):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            matrix, manifest = self.fixture(root)
            metadata, directories, completed = verify_matrix(
                root, [dict(ruleset="chess")]
            )
            self.assertEqual(directories, completed)
            self.assertEqual(
                metadata["revisions"], dict(baseline="old", candidate="new")
            )
            changed = copy.deepcopy(manifest)
            changed["candidate_sha256"] = "another"
            (root / "chess/manifest.json").write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError, "frozen binaries"):
                verify_matrix(root, [dict(ruleset="chess")])
            (root / "chess/manifest.json").write_text(json.dumps(manifest))
            matrix["results"] = []
            (root / "matrix.json").write_text(json.dumps(matrix))
            with self.assertRaisesRegex(ValueError, "Finish every"):
                verify_matrix(root, [dict(ruleset="chess")])

    def test_failed_pool_remains_visible_without_a_fabricated_rating(self):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            matrix, _ = self.fixture(root)
            matrix["results"][0].update(returncode=1, error="engine failed")
            (root / "matrix.json").write_text(json.dumps(matrix))
            (root / "chess/games.tsv").unlink()
            metadata, directories, completed = verify_matrix(
                root, [dict(ruleset="chess")]
            )
            self.assertEqual(metadata["failures"], dict(chess="engine failed"))
            self.assertEqual(directories, [])
            self.assertEqual(completed, [])


if __name__ == "__main__":
    unittest.main()
