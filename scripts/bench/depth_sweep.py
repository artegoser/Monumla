#!/usr/bin/env python3
"""Inspect a saved position as requested depth increases; preserve incomplete runs."""

import argparse
import json
from pathlib import Path
from strength import Worker, sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True)
    parser.add_argument("--pgn", type=Path, required=True)
    parser.add_argument("--first", type=int, default=3)
    parser.add_argument("--last", type=int, default=7)
    parser.add_argument(
        "--candidate-move",
        help="Also run a restricted-root search of this legal move label",
    )
    parser.add_argument(
        "--resume",
        action="store_true",
        help="Reuse completed iterations between depths (default: fresh session each depth)",
    )
    parser.add_argument("--nodes", type=int, default=2_000_000)
    parser.add_argument("--time-ms", type=int, default=60_000)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.first < 1 or args.last < args.first:
        raise ValueError("Use increasing positive depths")
    if args.out.exists():
        raise ValueError("Preserve existing experiment output")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    worker = Worker(args.engine)
    reports = []
    try:
        source = args.pgn.read_text()
        worker.call("init", ruleset="chess", pgn=source)
        for depth in range(args.first, args.last + 1):
            if not args.resume:
                worker.call("init", ruleset="chess", pgn=source)
            report = worker.call(
                "analyze", depth=depth, nodes=args.nodes, time_ms=args.time_ms
            )
            probe = (
                worker.call(
                    "probe",
                    label=args.candidate_move,
                    depth=depth,
                    nodes=args.nodes,
                    time_ms=args.time_ms,
                )
                if args.candidate_move
                else None
            )
            reports.append(
                {"requested_depth": depth, "analysis": report, "candidate": probe}
            )
            args.out.write_text(
                json.dumps(
                    {
                        "engine_sha256": sha256(args.engine),
                        "position": str(args.pgn),
                        "position_sha256": sha256(args.pgn),
                        "resume": args.resume,
                        "candidate_move": args.candidate_move,
                        "nodes_per_call": args.nodes,
                        "time_ms_per_call": args.time_ms,
                        "reports": reports,
                    },
                    indent=2,
                )
            )
            print(
                json.dumps(
                    {
                        "requested": depth,
                        "completed": report["depth"],
                        "nodes": report["nodes"],
                        "score": report["score"],
                        "pv": report["principal_variation"],
                        "stop": report["stop_reason"],
                        "candidate": probe,
                    }
                ),
                flush=True,
            )
    finally:
        worker.close()


if __name__ == "__main__":
    main()
