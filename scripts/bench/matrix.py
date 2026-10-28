#!/usr/bin/env python3
"""Run every declared native case against the same two frozen binaries."""

import argparse
import json
import subprocess
from pathlib import Path
from strength import Worker, sha256


def source_pgn(case):
    if not case.get("pgn"):
        return None
    source = "\n".join(
        line
        for line in Path(case["pgn"]).read_text().splitlines()
        if not line.startswith("[Result ")
    ).strip()
    for ending in ["1/2-1/2", "1-0", "0-1", "*"]:
        if source.endswith(ending):
            source = source[: -len(ending)]
            break
    return source + " *"


def prepare(worker, case):
    status = worker.call(
        "init",
        ruleset=case["ruleset"],
        pgn=source_pgn(case),
        seed=1,
        opening_plies=case.get("opening_plies", 0),
    )
    for action in (
        json.loads(Path(case["prelude"]).read_text()) if case.get("prelude") else []
    ):
        status = worker.call(
            "apply", action=dict(position_key=status["key"], action=action)
        )
    return status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in [
        "baseline",
        "candidate",
        "observer",
        "baseline-revision",
        "candidate-revision",
        "out",
    ]:
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--cases", type=Path, default=Path("benchmarks/cases.json"))
    parser.add_argument("--cpu", type=int, default=5)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--warmup-ms", type=int, default=0)
    parser.add_argument(
        "--build-id", help="Shared compiler/build identity for both binaries"
    )
    args = parser.parse_args()
    out = Path(args.out)
    if out.exists():
        raise ValueError("Preserve existing matrix runs")
    out.mkdir(parents=True)
    (out / "snapshots").mkdir()
    cases = json.loads(args.cases.read_text())
    results = []
    observer = Worker(args.observer, args.cpu)
    manifest = dict(
        settings=vars(args) | {"cases": str(args.cases)},
        baseline_sha256=sha256(args.baseline),
        candidate_sha256=sha256(args.candidate),
        cases_sha256=sha256(args.cases),
        results=results,
    )
    try:
        for case in cases:
            prepare(observer, case)
            snapshot = dict(
                ruleset=case["ruleset"].split(":")[0],
                timeline=observer.call("observation"),
                initial_fen=(
                    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
                    if case["ruleset"] == "chess"
                    else ""
                ),
                undo_floor=0,
                pgn_tags={},
            )
            (out / "snapshots" / f"{case['id']}.json").write_text(json.dumps(snapshot))
            report = out / f"{case['id']}.json"
            command = [
                "python3",
                "scripts/bench/throughput.py",
                "--baseline",
                args.baseline,
                "--candidate",
                args.candidate,
                "--ruleset",
                case["ruleset"],
                "--depth",
                str(case["depth"]),
                "--nodes",
                str(case["nodes"]),
                "--cpu",
                str(args.cpu),
                "--repeats",
                str(args.repeats),
                "--warmups",
                str(args.warmups),
                "--warmup-ms",
                str(args.warmup_ms),
                "--opening-plies",
                str(case.get("opening_plies", 0)),
                "--out",
                str(report),
            ]
            if case.get("pgn"):
                command += ["--positions", case["pgn"]]
            if case.get("prelude"):
                command += ["--prelude", case["prelude"]]
            with (out / f"{case['id']}.log").open("w") as log:
                result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
            row = dict(case=case, returncode=result.returncode, report=str(report))
            if report.exists():
                row["summaries"] = json.loads(report.read_text())["summaries"]
            results.append(row)
            (out / "matrix.json").write_text(json.dumps(manifest, indent=2))
            print(json.dumps(row), flush=True)
    finally:
        observer.close()
    if any(row["returncode"] for row in results):
        raise SystemExit(
            "Some cases changed work/answers or failed; inspect every matrix row"
        )


if __name__ == "__main__":
    main()
