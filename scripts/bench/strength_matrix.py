#!/usr/bin/env python3
"""Predeclare and run all strength pools against one frozen revision pair."""

import argparse
import json
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path
from queue import SimpleQueue
from strength import match, sha256
from strength_history import pool_name


def validate(cases, coverage):
    if not cases or len({case["id"] for case in cases}) != len(cases):
        raise ValueError("Require unique, nonempty case IDs")
    pools = [pool_name(case["ruleset"]) for case in cases]
    if len(set(pools)) != len(pools) or set(pools) != {
        pool_name(case["ruleset"]) for case in coverage
    }:
        raise ValueError(
            "Strength cases must cover each declared game pool exactly once"
        )
    for case in cases:
        if (
            not case["id"].replace("-", "").isalnum()
            or case["games"] < 2
            or case["games"] % 2
            or case["time_ms"] < 1
            or case["max_plies"] < 1
        ):
            raise ValueError(
                "Use safe IDs, even game counts, positive time budgets and move caps"
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in [
        "baseline",
        "candidate",
        "baseline-revision",
        "candidate-revision",
        "out",
    ]:
        parser.add_argument("--" + name, required=True)
    parser.add_argument(
        "--cases", type=Path, default=Path("benchmarks/strength-cases.json")
    )
    parser.add_argument("--coverage", type=Path, default=Path("benchmarks/cases.json"))
    parser.add_argument("--cpus", default="0,1,2,3")
    parser.add_argument("--opening-plies", type=int, default=2)
    args = parser.parse_args()
    cases = json.loads(args.cases.read_text())
    validate(cases, json.loads(args.coverage.read_text()))
    cpus = [int(cpu) for cpu in args.cpus.split(",")]
    if (
        not cpus
        or len(set(cpus)) != len(cpus)
        or not set(cpus).issubset(os.sched_getaffinity(0))
        or args.opening_plies < 0
    ):
        raise ValueError("Require distinct allowed CPUs and nonnegative opening plies")
    out = Path(args.out)
    out.mkdir(parents=True)  # Preserve previous runs.
    manifest = dict(
        schema=1,
        started_at=datetime.now(timezone.utc).isoformat(),
        settings=vars(args)
        | {"cases": str(args.cases), "coverage": str(args.coverage)},
        runner_revision=subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip(),
        runner_dirty=bool(
            subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()
        ),
        baseline_sha256=sha256(args.baseline),
        candidate_sha256=sha256(args.candidate),
        cases_sha256=sha256(args.cases),
        plan=cases,
        results=[],
    )
    destination = out / "matrix.json"
    destination.write_text(json.dumps(manifest, indent=2) + "\n")

    # Share the CPU slots across all pools. A slow remaining pool can occupy
    # every free slot; inactive pools do not reserve a physical core.
    slots = SimpleQueue()
    for cpu in cpus:
        slots.put(cpu)

    def run_pool(case):
        settings = argparse.Namespace(
            mode="match",
            baseline=args.baseline,
            candidate=args.candidate,
            baseline_revision=args.baseline_revision,
            candidate_revision=args.candidate_revision,
            ruleset=case["ruleset"],
            games=case["games"],
            time_ms=case["time_ms"],
            max_plies=case["max_plies"],
            opening_plies=args.opening_plies,
            depth=32,
            nodes=50000000,
            jobs=min(len(cpus), case["games"] // 2),
            cpus=args.cpus,
            out=str(out / case["id"]),
        )
        row = dict(case=case, directory=settings.out, returncode=0)
        with (out / f"{case['id']}.log").open("w") as log:
            try:
                match(
                    settings,
                    cpu_slots=slots,
                    emit=lambda message: print(message, file=log, flush=True),
                )
            except Exception as error:
                row.update(returncode=1, error=str(error))
                print(repr(error), file=log, flush=True)
        report = out / case["id"] / "strength.json"
        if report.exists():
            row["summary"] = json.loads(report.read_text())
        print(json.dumps(row), flush=True)
        return row

    with ThreadPoolExecutor(max_workers=len(cases)) as executor:
        futures = [executor.submit(run_pool, case) for case in cases]
        for future in as_completed(futures):
            manifest["results"].append(future.result())
            destination.write_text(json.dumps(manifest, indent=2) + "\n")
    if (
        sha256(args.baseline) != manifest["baseline_sha256"]
        or sha256(args.candidate) != manifest["candidate_sha256"]
    ):
        raise RuntimeError("A frozen binary changed during the experiment")
    if any(row["returncode"] for row in manifest["results"]):
        raise SystemExit("Some pools failed; retain and inspect the complete matrix")


if __name__ == "__main__":
    main()
