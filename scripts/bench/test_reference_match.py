import tempfile
import unittest
from pathlib import Path

from reference_match import archive_interrupted, validate_row


class ReferenceMatchTests(unittest.TestCase):
    def test_global_game_identity_and_retained_pgn_are_required(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            row = dict(
                game="5",
                pair="2",
                side="1",
                score="0.5",
                result="1/2-1/2",
                termination="draw",
            )
            with self.assertRaises(ValueError):
                validate_row(row, 5, directory)
            (directory / "game-005.pgn").write_text('[Result "1/2-1/2"]\n\n1/2-1/2')
            result = validate_row(row, 5, directory)
            self.assertEqual(
                (result["game"], result["pair"], result["side"]), (5, 2, 1)
            )
            for field, value in [
                ("game", "6"),
                ("pair", "1"),
                ("side", "2"),
                ("score", "0.9"),
                ("score", "1"),
                ("result", "1-0"),
            ]:
                with self.subTest(field=field), self.assertRaises(ValueError):
                    validate_row(row | {field: value}, 5, directory)

    def test_interrupted_directories_are_preserved_without_overwriting(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / "game-001"
            for content in ["first trace", "second trace"]:
                directory.mkdir()
                (directory / "moves.tsv").write_text(content)
                archive_interrupted(directory)
            self.assertEqual(
                (Path(root) / "game-001.interrupted-1" / "moves.tsv").read_text(),
                "first trace",
            )
            self.assertEqual(
                (Path(root) / "game-001.interrupted-2" / "moves.tsv").read_text(),
                "second trace",
            )


if __name__ == "__main__":
    unittest.main()
