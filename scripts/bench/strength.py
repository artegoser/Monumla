#!/usr/bin/env python3
"""Independent paired-match statistics and version comparison. Python stdlib only."""

import argparse
import csv
import hashlib
import json
import math
from datetime import datetime, timezone
from pathlib import Path
import subprocess
import select
import time
import os
import platform
import threading
from queue import SimpleQueue
from concurrent.futures import ThreadPoolExecutor, as_completed


def elo(score):
    if score <= 0:
        return -math.inf
    if score >= 1:
        return math.inf
    return 400 * math.log10(score / (1 - score))


def interval(samples, alpha=0.05):
    """Conservative fixed-N KL/Chernoff interval for independent [0,1] pair scores.

    Within-pair games may be correlated. No normal approximation, draw model,
    pseudocount, or finite Elo estimate is invented for all-loss/all-win samples.
    Bound: P(mean deviates at least d) <= exp(-N * binary_KL(mean, true_mean)).
    """
    if not samples:
        return 0.0, 1.0
    mean = sum(samples) / len(samples)
    threshold = math.log(2 / alpha) / len(samples)

    def divergence(p):
        value = 0.0
        if mean:
            value += mean * math.log(mean / max(p, 1e-300))
        if mean < 1:
            value += (1 - mean) * math.log((1 - mean) / max(1 - p, 1e-300))
        return value

    lo, hi = 0.0, mean
    for _ in range(80):
        mid = (lo + hi) / 2
        if divergence(mid) > threshold:
            lo = mid
        else:
            hi = mid
    lower = (lo + hi) / 2 if mean else 0.0
    lo, hi = mean, 1.0
    for _ in range(80):
        mid = (lo + hi) / 2
        if divergence(mid) > threshold:
            hi = mid
        else:
            lo = mid
    return lower, (lo + hi) / 2 if mean < 1 else 1.0


def summarize(rows, reference=None):
    pairs = {}
    wdl = [0, 0, 0]
    for row in rows:
        score = row["score"]
        if score != "NA":
            score = float(score)
            wdl[0 if score == 1 else 1 if score == 0.5 else 2] += 1
        pairs.setdefault(row["pair"], []).append(score)
    complete = [sum(v) / 2 for v in pairs.values() if len(v) == 2 and "NA" not in v]
    penta = [sum(value == i / 4 for value in complete) for i in range(5)]
    low, high = interval(complete)
    point = elo(sum(complete) / len(complete)) if complete else None
    delta = {"point": point, "low": elo(low), "high": elo(high)}

    def printable(value):
        if value is None:
            return None
        return (
            round(value, 1)
            if math.isfinite(value)
            else ("+inf" if value > 0 else "-inf")
        )

    result = {
        "wdl": wdl,
        "complete_pairs": len(complete),
        "excluded_pairs": len(pairs) - len(complete),
        "pentanomial": penta,
        "score_interval_95": [low, high],
        "delta_elo": {k: printable(v) for k, v in delta.items()},
        "method": "fixed-N conservative paired KL/Chernoff bound; independent pair assumption",
    }
    if reference is not None:
        result["reference_setting"] = reference
        result["reference_scale_elo"] = {
            k: printable(None if v is None else v + reference) for k, v in delta.items()
        }
        result["rating_caveat"] = (
            "Opponent-setting reference scale, not FIDE/CCRL rating; calibration depends on time control and pool."
        )
    return result


class Worker:
    def __init__(self, path, cpu=None):
        command = [str(Path(path).resolve())]
        if cpu is not None:
            command = ["taskset", "-c", str(cpu)] + command
        self.process = subprocess.Popen(
            command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1
        )

    def call(self, command, **values):
        self.process.stdin.write(json.dumps({"command": command, **values}) + "\n")
        self.process.stdin.flush()
        if not select.select(
            [self.process.stdout],
            [],
            [],
            max(180, values.get("time_ms", 0) / 1000 + 30),
        )[0]:
            raise TimeoutError("Engine did not reply within the experiment timeout")
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f"Engine exited: {self.process.poll()}")
        reply = json.loads(line)
        if "error" in reply:
            raise RuntimeError(reply["error"])
        return reply["ok"]

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def settings_for(args, candidate):
    prefix = "candidate" if candidate else "baseline"
    return {
        name: (
            getattr(args, f"{prefix}_{name}", None)
            if getattr(args, f"{prefix}_{name}", None) is not None
            else getattr(args, name)
        )
        for name in ["depth", "nodes", "time_ms"]
    }


def match(args, cpu_slots=None, emit=None):
    if emit is None:
        emit = lambda message: print(message, flush=True)
    if args.games < 2 or args.games % 2 or args.opening_plies < 0 or args.jobs < 1:
        raise ValueError(
            "Use an even number of games, positive jobs and nonnegative opening plies"
        )
    cpus = [int(cpu) for cpu in args.cpus.split(",")] if args.cpus else []
    if cpus and (
        len(cpus) < args.jobs
        or len(set(cpus)) != len(cpus)
        or not set(cpus).issubset(os.sched_getaffinity(0))
    ):
        raise ValueError("Supply one allowed CPU per job")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    if any(out.iterdir()):
        raise ValueError("Output directory must be empty (preserve previous runs)")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()
    manifest = {
        **vars(args),
        "runner_revision": revision,
        "runner_dirty": bool(dirty),
        "started_at": datetime.now(timezone.utc).isoformat(),
        "baseline_sha256": sha256(args.baseline),
        "candidate_sha256": sha256(args.candidate),
        "baseline_settings": settings_for(args, False),
        "candidate_settings": settings_for(args, True),
        "platform": platform.platform(),
        "affinity": sorted(os.sched_getaffinity(0)),
        "load_average_at_start": os.getloadavg(),
        "paired_game_count_predeclared": args.games,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2))
    (out / "moves").mkdir()
    rows = []
    lock = threading.Lock()
    stop = threading.Event()
    slots = cpu_slots if cpu_slots is not None else SimpleQueue()
    if cpu_slots is None:
        for cpu in (cpus[: args.jobs] if cpus else [None] * args.jobs):
            slots.put(cpu)

    def pair(pair_index):
        cpu = slots.get()
        workers = []
        pair_rows = []
        try:
            workers.append(Worker(args.baseline, cpu))
            workers.append(Worker(args.candidate, cpu))
            for color in range(2):
                game = 2 * pair_index + color
                candidate_side = 1 + color
                status = [
                    worker.call(
                        "init",
                        ruleset=args.ruleset,
                        seed=pair_index,
                        opening_plies=args.opening_plies,
                    )
                    for worker in workers
                ]
                if status[0] != status[1]:
                    raise RuntimeError(
                        "Version rules or opening differ; positions must match"
                    )
                score, reason = "NA", "ply limit (unfinished, excluded)"
                with (out / "moves" / f"game-{game+1:04d}.jsonl").open("w") as log:
                    for ply in range(args.max_plies + 1):
                        if stop.is_set():
                            raise RuntimeError(
                                "Experiment aborted after a peer failure"
                            )
                        state = status[0]
                        if state["outcome"]:
                            outcome = state["outcome"]
                            score = (
                                "1"
                                if candidate_side in outcome["winners"]
                                else (
                                    "0"
                                    if candidate_side in outcome["losers"]
                                    else "0.5"
                                )
                            )
                            reason = outcome["key"]
                            break
                        if ply == args.max_plies:
                            break
                        if len(state["active"]) != 1:
                            raise RuntimeError(
                                "Match runner requires a single active player"
                            )
                        actor = state["active"][0]
                        index = int(actor == candidate_side)
                        settings = settings_for(args, bool(index))
                        started = time.monotonic()
                        result = workers[index].call("analyze", **settings)
                        log.write(
                            json.dumps(
                                {
                                    "game": game + 1,
                                    "ply": ply,
                                    "actor": actor,
                                    "cpu": cpu,
                                    "candidate": bool(index),
                                    "wall_ms": (time.monotonic() - started) * 1000,
                                    "settings": settings,
                                    "analysis": result,
                                }
                            )
                            + "\n"
                        )
                        log.flush()
                        action = result["best_action"]
                        if not action:
                            score = "0" if index else "1"
                            reason = "no completed search iteration forfeit"
                            break
                        status = [
                            worker.call("apply", action=action) for worker in workers
                        ]
                        if status[0] != status[1]:
                            raise RuntimeError(
                                "Peer state diverged after action replay"
                            )
                row = dict(
                    pair=pair_index,
                    game=game + 1,
                    side=candidate_side,
                    score=score,
                    result="candidate score",
                    termination=reason,
                )
                pair_rows.append(row)
                with lock:
                    rows.append(row)
                    with (out / "games.tsv").open("w") as table:
                        writer = csv.DictWriter(table, fields, delimiter="\t")
                        writer.writeheader()
                        writer.writerows(sorted(rows, key=lambda r: r["game"]))
                    emit(json.dumps(row))
            return pair_rows
        except Exception:
            stop.set()
            raise
        finally:
            for worker in workers:
                worker.close()
            slots.put(cpu)

    fields = ["pair", "game", "side", "score", "result", "termination"]
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = [executor.submit(pair, i) for i in range(args.games // 2)]
        for future in as_completed(futures):
            future.result()
    report = summarize(rows)
    (out / "strength.json").write_text(json.dumps(report, indent=2))
    emit(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    report = sub.add_parser("report")
    report.add_argument("games", type=Path)
    report.add_argument("--reference", type=float)
    run = sub.add_parser("match")
    run.add_argument("--baseline", required=True)
    run.add_argument("--candidate", required=True)
    run.add_argument("--ruleset", default="chess")
    run.add_argument("--jobs", type=int, default=1)
    run.add_argument("--cpus", help="Comma-separated allowed CPUs, one per job")
    for side in ["baseline", "candidate"]:
        run.add_argument(
            f"--{side}-revision",
            help="Source revision of the frozen binary, if verified; distinct from runner_revision",
        )
        run.add_argument(f"--{side}-time-ms", type=int)
        run.add_argument(f"--{side}-depth", type=int)
        run.add_argument(f"--{side}-nodes", type=int)
    run.add_argument("--games", type=int, default=8)
    run.add_argument("--depth", type=int, default=32)
    run.add_argument("--nodes", type=int, default=50000000)
    run.add_argument(
        "--time-ms",
        type=int,
        default=1000,
        help="0: fixed nodes/depth, no wall deadline",
    )
    run.add_argument("--opening-plies", type=int, default=2)
    run.add_argument("--max-plies", type=int, default=300)
    run.add_argument("--out", required=True)
    args = parser.parse_args()
    if args.mode == "match":
        match(args)
    else:
        with args.games.open() as source:
            print(
                json.dumps(
                    summarize(
                        list(csv.DictReader(source, delimiter="\t")), args.reference
                    ),
                    indent=2,
                )
            )


if __name__ == "__main__":
    main()
