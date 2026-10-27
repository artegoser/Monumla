import unittest

from reference_report import publish
from strength import summarize


class ReferenceReportTests(unittest.TestCase):
    def report(self, score):
        rows = [dict(pair=game // 2, score=score) for game in range(12)]
        return dict(
            manifest=dict(calibrated_time_control=True),
            complete_schedule=True,
            completed_games=12,
            planned_games=12,
            **summarize(rows, 1320),
        )

    def test_all_losses_publish_an_upper_bound_without_inventing_a_finite_rating(self):
        result = publish(self.report("0"))
        self.assertEqual(result["rating_status"], "upper_bound")
        self.assertEqual(result["reference_scale_elo"]["point"], "-inf")
        self.assertLess(result["reference_scale_elo"]["high"], 1320)

    def test_all_wins_and_interior_scores_are_distinguished(self):
        result = publish(self.report("1"))
        self.assertEqual(result["rating_status"], "lower_bound")
        self.assertEqual(result["reference_scale_elo"]["point"], "+inf")
        result = publish(self.report("0.5"))
        self.assertEqual(result["rating_status"], "estimate")
        self.assertEqual(result["reference_scale_elo"]["point"], 1320)

    def test_partial_schedules_and_uncalibrated_clocks_cannot_be_published(self):
        report = self.report("0.5")
        with self.assertRaises(ValueError):
            publish(report | dict(complete_schedule=False))
        with self.assertRaises(ValueError):
            publish(report | dict(completed_games=10))
        with self.assertRaises(ValueError):
            publish(report | dict(manifest=dict(calibrated_time_control=False)))


if __name__ == "__main__":
    unittest.main()
