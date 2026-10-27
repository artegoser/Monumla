import csv
import json
import os
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch
from strength import match


class MatchResumeTests(unittest.TestCase):
    def test_restart_preserves_finished_game_and_rejects_changed_budget(self):
        calls = []

        class Engine:
            def __init__(self, path, cpu):
                pass

            def call(self, command, **values):
                calls.append(command)
                if len(calls) == 3:
                    raise RuntimeError("interrupted")
                return dict(
                    key="terminal",
                    active=[],
                    outcome=dict(winners=[1], losers=[2], key="win"),
                )

            def close(self):
                pass

        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            binary = root / "binary"
            binary.write_bytes(b"frozen")
            out = root / "match"
            args = Namespace(
                baseline=str(binary),
                candidate=str(binary),
                ruleset="fixture",
                games=2,
                time_ms=500,
                depth=32,
                nodes=1000,
                opening_plies=2,
                max_plies=10,
                jobs=1,
                cpus=str(min(os.sched_getaffinity(0))),
                out=str(out),
            )
            with patch("strength.Worker", Engine):
                with self.assertRaisesRegex(RuntimeError, "interrupted"):
                    match(args, emit=lambda _: None)
                first = (out / "moves/game-0001.jsonl").read_bytes()
                (out / "moves/game-0002.jsonl").write_text("partial search trace\n")
                calls.clear()
                args.resume = True
                match(args, emit=lambda _: None)
                self.assertEqual(calls, ["init", "init"])
                self.assertEqual((out / "moves/game-0001.jsonl").read_bytes(), first)
                self.assertEqual(
                    (out / "moves/game-0002.interrupted-1.jsonl").read_text(),
                    "partial search trace\n",
                )
                rows = list(csv.DictReader((out / "games.tsv").open(), delimiter="\t"))
                self.assertEqual([r["game"] for r in rows], ["1", "2"])
                self.assertEqual(
                    json.loads((out / "strength.json").read_text())["wdl"], [1, 0, 1]
                )
                manifest = (out / "manifest.json").read_bytes()
                args.time_ms = 1000
                with self.assertRaisesRegex(ValueError, "settings"):
                    match(args, emit=lambda _: None)
                self.assertEqual((out / "manifest.json").read_bytes(), manifest)


if __name__ == "__main__":
    unittest.main()
