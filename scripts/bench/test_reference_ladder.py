import unittest

from reference_ladder import DEFAULT_SELECTION, choose_level, validate_manifest
from strength import summarize


def report(level, scores):
    return dict(
        manifest=dict(calibrated_time_control=True),
        complete_schedule=True,
        completed_games=len(scores),
        planned_games=len(scores),
        **summarize(
            [dict(pair=i // 2, score=str(s)) for i, s in enumerate(scores)], level
        ),
    )


class ReferenceLadderTests(unittest.TestCase):
    def test_equal_match_selects_confirmation_level(self):
        self.assertEqual(
            choose_level([report(1600, [1, 0] * 6)], DEFAULT_SELECTION), (1600, True)
        )

    def test_search_steps_and_brackets_follow_completed_scores(self):
        pilot = report(1320, [1] * 10 + [0] * 2)
        self.assertEqual(choose_level([pilot], DEFAULT_SELECTION), (1520, False))
        strong = report(1600, [1] * 8 + [0] * 4)
        self.assertEqual(
            choose_level([pilot, strong], DEFAULT_SELECTION), (1800, False)
        )
        weak = report(1600, [1] * 4 + [0] * 8)
        self.assertEqual(choose_level([pilot, weak], DEFAULT_SELECTION), (1450, False))

    def test_repeated_levels_are_combined_instead_of_picking_a_favorable_batch(self):
        reports = [report(1600, [1] * 12), report(1600, [0] * 12)]
        self.assertEqual(choose_level(reports, DEFAULT_SELECTION), (1600, True))

    def test_boundaries_do_not_invent_an_out_of_range_stockfish_setting(self):
        self.assertEqual(
            choose_level([report(1320, [0] * 12)], DEFAULT_SELECTION), (1320, False)
        )
        self.assertEqual(
            choose_level([report(3190, [1] * 12)], DEFAULT_SELECTION), (3190, False)
        )

    def test_unfinished_or_partial_batches_do_not_select_a_level(self):
        for value in [
            report(1600, ["NA"] * 12),
            report(1600, [0.5] * 12) | dict(complete_schedule=False),
        ]:
            with self.assertRaises(ValueError):
                choose_level([value], DEFAULT_SELECTION)

    def test_nonmonotone_bracket_does_not_get_silently_fitted(self):
        with self.assertRaisesRegex(ValueError, "Nonmonotone"):
            choose_level(
                [report(1400, [0] * 12), report(1800, [1] * 12)], DEFAULT_SELECTION
            )

    def test_level_may_change_but_engine_and_controls_must_match(self):
        from reference_ladder import IDENTITY

        template = {key: key for key in IDENTITY}
        template.update(
            baseline_settings=dict(elo=1320, clock_ms=120000, increment_ms=1000),
            games=12,
            calibrated_time_control=True,
        )
        manifest = template | dict(
            baseline_settings=dict(template["baseline_settings"], elo=1600)
        )
        entry = dict(elo=1600, games=12)
        validate_manifest(manifest, template, entry)
        for changed in [
            manifest | dict(candidate_sha256="different"),
            manifest | dict(games=48),
            manifest | dict(baseline_settings=dict(elo=1600, clock_ms=1000)),
            manifest | dict(calibrated_time_control=False),
        ]:
            with self.assertRaises(ValueError):
                validate_manifest(changed, template, entry)


class ReferenceLadderLifecycleTests(unittest.TestCase):
    def test_fresh_fixed_confirmation_and_resume_never_replay_completed_games(self):
        import json
        import tempfile
        from pathlib import Path
        from unittest.mock import patch
        from match_storage import write_games, write_json
        from reference_ladder import IDENTITY, main
        from reference_match import FIELDS
        from strength import sha256

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "engine"
            binary.write_text("frozen binary fixture")
            template = {key: key for key in IDENTITY}
            template.update(
                baseline=str(binary),
                candidate=str(binary),
                baseline_sha256=sha256(binary),
                candidate_sha256=sha256(binary),
                baseline_settings=dict(elo=1600, clock_ms=120000, increment_ms=1000),
                games=12,
                calibrated_time_control=True,
                cpus="0",
            )

            def batch(path, level, scores):
                path.mkdir(parents=True)
                manifest = template | dict(
                    games=len(scores),
                    baseline_settings=dict(template["baseline_settings"], elo=level),
                )
                write_json(path / "manifest.json", manifest)
                rows = []
                for index, value in enumerate(scores):
                    game = index + 1
                    side = 1 + index % 2
                    winner = side if value == 1 else 3 - side
                    result = "1-0" if winner == 1 else "0-1"
                    moves = path / "moves" / f"game-{game:03}"
                    moves.mkdir(parents=True)
                    (moves / f"game-{game:03}.pgn").write_text(
                        f'[Result "{result}"]\n\n{result}'
                    )
                    rows.append(
                        dict(
                            game=game,
                            pair=index // 2,
                            side=side,
                            score=str(value),
                            result=result,
                            termination="chess.checkmate",
                        )
                    )
                write_games(path / "games.tsv", rows, FIELDS)
                write_json(path / "report.json", dict(complete_schedule=True))

            seed = root / "seed"
            batch(seed, 1600, [1, 0] * 6)
            out = root / "ladder"
            calls = []

            def play(command, check):
                calls.append(command)
                self.assertTrue(check)
                count = int(command[command.index("--games") + 1])
                level = int(command[command.index("--elo") + 1])
                self.assertEqual((count, level), (48, 1600))
                # Even an extreme confirmation result must complete as declared.
                batch(Path(command[command.index("--out") + 1]), level, [0] * count)

            with patch(
                "sys.argv", ["ladder", "--seed", str(seed), "--out", str(out)]
            ), patch("reference_ladder.subprocess.run", side_effect=play):
                main()
            plan = json.loads((out / "plan.json").read_text())
            self.assertEqual(
                [entry["role"] for entry in plan["runs"]], ["probe", "confirmation"]
            )
            result = json.loads((out / "calibration.json").read_text())
            self.assertEqual(result["primary"]["wdl"], [0, 0, 48])
            self.assertEqual(result["primary"]["reference_scale_elo"]["point"], "-inf")
            with patch("sys.argv", ["ladder", "--out", str(out), "--resume"]), patch(
                "reference_ladder.subprocess.run",
                side_effect=AssertionError("Completed games were replayed"),
            ):
                main()
            self.assertEqual(len(calls), 1)
            # A later equilibrium check is a new batch, never a replacement
            # for an unfavorable confirmation or a reason to replay it.
            from reference_ladder import calibration_report
            from reference_ladder_report import final_report

            validation = root / "validation"
            plan["runs"].append(
                dict(role="validation", path=str(validation), elo=1450, games=12)
            )
            write_json(out / "plan.json", plan)
            pending = calibration_report(plan)
            self.assertFalse(pending["complete"])
            with self.assertRaises(ValueError):
                final_report(pending)

            def validate(command, check):
                self.assertEqual(command[command.index("--games") + 1], "12")
                self.assertEqual(command[command.index("--elo") + 1], "1450")
                batch(validation, 1450, [1] * 12)

            with patch("sys.argv", ["ladder", "--out", str(out), "--resume"]), patch(
                "reference_ladder.subprocess.run", side_effect=validate
            ) as played:
                main()
                self.assertEqual(played.call_count, 1)
            result = final_report(calibration_report(plan))
            self.assertEqual(result["wdl"], [0, 0, 48])
            self.assertEqual(result["reference_scale_elo"]["point"], "-inf")


if __name__ == "__main__":
    unittest.main()
