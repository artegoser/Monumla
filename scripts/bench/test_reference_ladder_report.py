import copy
import unittest

from reference_ladder_report import discovery_points, final_report
from reference_report import publish
from test_reference_ladder import report


class ReferenceLadderReportTests(unittest.TestCase):
    def calibration(self):
        probe = publish(report(1320, [1] * 10 + [0] * 2))
        confirm = publish(report(1600, [0.5] * 48))
        return dict(
            complete=True,
            primary=confirm,
            selection={},
            inference="fresh only",
            runs=[
                dict(role="probe", path="probe", report=probe),
                dict(role="confirmation", path="confirmation", report=confirm),
            ],
        )

    def test_final_rating_uses_only_confirmation_not_discovery_wins(self):
        result = final_report(self.calibration())
        self.assertEqual(result["reference_scale_elo"]["point"], 1600)
        self.assertEqual(result["wdl"], [0, 48, 0])
        self.assertEqual(result["complete_pairs"], 24)
        self.assertEqual(result["calibration"]["discovery"][0]["wdl"], [10, 0, 2])

    def test_confirmation_is_not_pooled_into_discovery_curve(self):
        calibration = self.calibration()
        calibration["runs"].insert(
            1, dict(role="probe", path="probe2", report=publish(report(1320, [0] * 12)))
        )
        points = discovery_points(calibration)
        self.assertEqual(len(points), 1)
        self.assertEqual(points[0]["wdl"], [10, 0, 14])
        self.assertEqual(points[0]["pairs"], 12)
        self.assertAlmostEqual(points[0]["score"], 10 / 24)

    def test_partial_duplicate_or_substituted_confirmation_cannot_publish(self):
        original = self.calibration()
        changed = []
        changed.append(original | dict(complete=False))
        duplicate = copy.deepcopy(original)
        duplicate["runs"][1]["path"] = "probe"
        changed.append(duplicate)
        substituted = copy.deepcopy(original)
        substituted["primary"] = substituted["runs"][0]["report"]
        changed.append(substituted)
        twice = copy.deepcopy(original)
        twice["runs"].append(twice["runs"][1])
        changed.append(twice)
        for calibration in changed:
            with self.subTest(calibration=calibration), self.assertRaises(ValueError):
                final_report(calibration)


if __name__ == "__main__":
    unittest.main()
