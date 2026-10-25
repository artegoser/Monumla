import copy
import json
import os
import tempfile
import threading
import time
import unittest
from argparse import Namespace
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from queue import SimpleQueue
from unittest.mock import patch
from strength import match
from strength_matrix import validate


class StrengthMatrixTests(unittest.TestCase):
    def test_shared_slots_bound_parallel_pairs_across_pools(self):
        cpus = sorted(os.sched_getaffinity(0))[:2]
        slots = SimpleQueue()
        for cpu in cpus:
            slots.put(cpu)
        active = set()
        guard = threading.Lock()
        maximum = [0]

        class Engine:
            def __init__(self, path, cpu):
                self.cpu = cpu

            def call(self, command, **values):
                if command == "analyze":
                    with guard:
                        if self.cpu in active:
                            raise AssertionError("CPU allocated to two active pairs")
                        active.add(self.cpu)
                        maximum[0] = max(maximum[0], len(active))
                    time.sleep(0.005)
                    with guard:
                        active.remove(self.cpu)
                    return dict(best_action="move")
                return dict(
                    key=command,
                    active=[1],
                    outcome=(
                        None
                        if command == "init"
                        else dict(winners=[1], losers=[2], key="win")
                    ),
                )

            def close(self):
                pass

        with tempfile.TemporaryDirectory() as root:
            binary = Path(root) / "binary"
            binary.write_bytes(b"frozen")

            def run(pool):
                args = Namespace(
                    baseline=str(binary),
                    candidate=str(binary),
                    ruleset=pool,
                    games=8,
                    time_ms=500,
                    depth=32,
                    nodes=1000,
                    opening_plies=0,
                    max_plies=2,
                    jobs=len(cpus),
                    cpus=",".join(map(str, cpus)),
                    out=str(Path(root) / pool),
                )
                match(args, cpu_slots=slots, emit=lambda _: None)
                return json.loads((Path(args.out) / "strength.json").read_text())

            with patch("strength.Worker", Engine), ThreadPoolExecutor(
                max_workers=2
            ) as executor:
                results = list(executor.map(run, ["one", "two"]))
            self.assertTrue(
                all(
                    row["complete_pairs"] == 4 and row["wdl"] == [4, 0, 4]
                    for row in results
                )
            )
            self.assertLessEqual(maximum[0], len(cpus))
            self.assertTrue(active == set())

    def test_every_pool_has_one_fixed_valid_schedule(self):
        cases = [
            dict(id="chess", ruleset="chess", games=12, time_ms=500, max_plies=300),
            dict(id="go", ruleset="go:19", games=2, time_ms=500, max_plies=800),
        ]
        coverage = [dict(ruleset="chess"), dict(ruleset="chess"), dict(ruleset="go:19")]
        validate(cases, coverage)
        with self.assertRaises(ValueError):
            validate(cases[:1], coverage)
        changed = copy.deepcopy(cases)
        changed[0]["games"] = 3
        with self.assertRaises(ValueError):
            validate(changed, coverage)
        with self.assertRaises(ValueError):
            validate(cases + [cases[0]], coverage)


if __name__ == "__main__":
    unittest.main()
