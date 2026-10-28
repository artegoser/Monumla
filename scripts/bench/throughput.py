#!/usr/bin/env python3
"""Alternating fixed-work comparison. Preserve answers as well as timing samples."""

import argparse
import json
import os
import platform
import statistics
import time
from pathlib import Path
from strength import Worker, sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--positions", type=Path, nargs="*", default=[])
    parser.add_argument("--ruleset", default="chess")
    parser.add_argument("--prelude", type=Path)
    parser.add_argument("--opening-plies", type=int, default=0)
    parser.add_argument("--depth", type=int, default=3)
    parser.add_argument("--nodes", type=int, default=100000)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--warmup-ms", type=int, default=0)
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.repeats < 3:
        raise ValueError("Use at least three measured repetitions")
    if args.warmups < 0 or args.warmup_ms < 0:
        raise ValueError("Warmup count and duration cannot be negative")
    if args.out.exists():
        raise ValueError("Preserve previous measurements; choose a new output file")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    records = []
    summaries = []
    identity = [
        "depth",
        "nodes",
        "score",
        "principal_variation",
        "lines",
        "stop_reason",
    ]
    workers = {
        side: Worker(getattr(args, side), args.cpu)
        for side in ["baseline", "candidate"]
    }
    try:
        for fixture in args.positions or [None]:
            pgn = fixture.read_text() if fixture else None
            if pgn:
                pgn = "\n".join(
                    line for line in pgn.splitlines() if not line.startswith("[Result ")
                ).strip()
                for ending in ["1/2-1/2", "1-0", "0-1", "*"]:
                    if pgn.endswith(ending):
                        pgn = pgn[: -len(ending)]
                        break
                pgn += " *"
            answers = []
            times = {"baseline": [], "candidate": []}
            iteration, measured = 0, 0
            warmed_ms = {"baseline": 0.0, "candidate": 0.0}
            while measured < args.repeats:
                warmup = (
                    iteration < args.warmups or min(warmed_ms.values()) < args.warmup_ms
                )
                results = {}
                order = (
                    ["baseline", "candidate"]
                    if iteration % 2 == 0
                    else ["candidate", "baseline"]
                )
                for side in order:
                    worker = workers[side]
                    status = worker.call(
                        "init",
                        ruleset=args.ruleset,
                        pgn=pgn,
                        seed=1,
                        opening_plies=args.opening_plies,
                    )
                    for action in (
                        json.loads(args.prelude.read_text()) if args.prelude else []
                    ):
                        status = worker.call(
                            "apply",
                            action=dict(position_key=status["key"], action=action),
                        )
                    start = time.perf_counter()
                    result = worker.call(
                        "analyze", depth=args.depth, nodes=args.nodes, time_ms=0
                    )
                    wall_ms = (time.perf_counter() - start) * 1000
                    results[side] = {key: result[key] for key in identity}
                    records.append(
                        dict(
                            fixture=str(fixture),
                            side=side,
                            iteration=iteration,
                            warmup=warmup,
                            wall_ms=wall_ms,
                            analysis=result,
                        )
                    )
                    if warmup:
                        warmed_ms[side] += wall_ms
                    else:
                        times[side].append(wall_ms)
                answers.append(results["baseline"] == results["candidate"])
                iteration += 1
                if not warmup:
                    measured += 1

            def timing(values):
                return dict(
                    median_ms=statistics.median(values),
                    min_ms=min(values),
                    max_ms=max(values),
                )

            summary = dict(
                fixture=str(fixture),
                same_work_and_answer=all(answers),
                baseline=timing(times["baseline"]),
                candidate=timing(times["candidate"]),
            )
            summary["speedup"] = (
                summary["baseline"]["median_ms"] / summary["candidate"]["median_ms"]
            )
            summaries.append(summary)
            print(json.dumps(summary), flush=True)
    finally:
        for worker in workers.values():
            worker.close()
    args.out.write_text(
        json.dumps(
            dict(
                settings={
                    key: str(value) if isinstance(value, Path) else value
                    for key, value in vars(args).items()
                },
                baseline_sha256=sha256(args.baseline),
                candidate_sha256=sha256(args.candidate),
                platform=platform.platform(),
                affinity=sorted(os.sched_getaffinity(0)),
                summaries=summaries,
                records=records,
            ),
            indent=2,
            default=str,
        )
    )
    if not all(summary["same_work_and_answer"] for summary in summaries):
        raise SystemExit(
            "Search work or answer changed: timing alone is not a behavior-preserving speedup"
        )


if __name__ == "__main__":
    main()
