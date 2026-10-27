#!/usr/bin/env python3
"""Predeclared, resumable chess calibration against a frozen Stockfish setting."""

import argparse
import csv
import json
import os
import platform
import signal
import subprocess
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path
from queue import SimpleQueue

from match_storage import prepare, write_games, write_json
from strength import sha256, summarize

FIELDS = ["pair", "game", "side", "score", "result", "termination"]


def terminate(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=10)
    except ProcessLookupError:
        pass
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)


def validate_row(row, game, directory):
    row = dict(row)
    for key in ["game", "pair", "side"]:
        row[key] = int(row[key])
    if (
        row["game"] != game
        or row["pair"] != (game - 1) // 2
        or row["side"] != 1 + (game - 1) % 2
        or row["score"] not in ["0", "0.5", "1", "NA"]
    ):
        raise ValueError("Game identity, side or score did not match the declared plan")
    result = row["result"]
    if result not in ["*", "1/2-1/2", "1-0", "0-1"]:
        raise ValueError("Invalid game result")
    expected = (
        "NA"
        if result == "*"
        else (
            "0.5"
            if result == "1/2-1/2"
            else "1" if result == ("1-0" if row["side"] == 1 else "0-1") else "0"
        )
    )
    if row["score"] != expected:
        raise ValueError(
            "Candidate score does not agree with the game result and color"
        )
    pgn = directory / f"game-{game:03}.pgn"
    if not pgn.is_file():
        raise ValueError("Completed games must retain their PGNs")
    if f'[Result "{result}"]' not in pgn.read_text():
        raise ValueError("PGN result does not agree with the score table")
    return row


def archive_interrupted(path):
    if path.exists():
        index = 1
        while path.with_name(f"{path.name}.interrupted-{index}").exists():
            index += 1
        path.rename(path.with_name(f"{path.name}.interrupted-{index}"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", type=Path, required=True)
    parser.add_argument("--stockfish", type=Path, required=True)
    parser.add_argument("--games", type=int, default=12)
    parser.add_argument("--elo", type=int, default=1320)
    parser.add_argument("--clock-ms", type=int, default=120000)
    parser.add_argument("--increment-ms", type=int, default=1000)
    parser.add_argument("--max-plies", type=int, default=500)
    parser.add_argument("--cpus", default="0,1,2,3")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--candidate-revision")
    args = parser.parse_args()
    cpus = [int(cpu) for cpu in args.cpus.split(",")]
    if args.games < 2 or args.games % 2 or args.games > 1000:
        parser.error("Predeclare an even game count between 2 and 1000")
    if (
        not cpus
        or len(set(cpus)) != len(cpus)
        or not set(cpus) <= os.sched_getaffinity(0)
    ):
        parser.error("Use distinct available CPUs")
    version = subprocess.run(
        [str(args.stockfish.resolve())],
        input="uci\nquit\n",
        text=True,
        capture_output=True,
        check=True,
        timeout=30,
    ).stdout
    name = next(
        line.removeprefix("id name ")
        for line in version.splitlines()
        if line.startswith("id name ")
    )
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    source_revision = args.candidate_revision or revision
    if (
        args.resume
        and not args.candidate_revision
        and (args.out / "manifest.json").exists()
    ):
        source_revision = json.loads((args.out / "manifest.json").read_text())[
            "candidate_revision"
        ]
    manifest = dict(
        mode="stockfish_reference",
        ruleset="chess",
        games=args.games,
        max_plies=args.max_plies,
        opening_plies="three built-in paired prefixes",
        baseline=str(args.stockfish.resolve()),
        candidate=str(args.runner.resolve()),
        baseline_revision=name,
        candidate_revision=source_revision,
        baseline_sha256=sha256(args.stockfish),
        candidate_sha256=sha256(args.runner),
        baseline_settings=dict(
            elo=args.elo,
            clock_ms=args.clock_ms,
            increment_ms=args.increment_ms,
            threads=1,
            hash_mb=16,
        ),
        candidate_settings=dict(
            clock_ms=args.clock_ms,
            increment_ms=args.increment_ms,
            depth=32,
            nodes=50000000,
            transposition_entries=16384,
            play=True,
        ),
        jobs=len(cpus),
        cpus=args.cpus,
        platform=platform.platform(),
        runner_revision=revision,
        runner_dirty=bool(subprocess.check_output(["git", "status", "--porcelain"])),
        started_at=datetime.now(timezone.utc).isoformat(),
        load_average_at_start=os.getloadavg(),
        opponent_uci=version,
        reference_scale="Stockfish UCI_Elo; documented calibration is 120 seconds + 1 second increment",
        calibrated_time_control=args.clock_ms == 120000 and args.increment_ms == 1000,
        paired_game_count_predeclared=args.games,
    )
    manifest, rows = prepare(args.out, manifest, args.resume)
    done = {row["game"] for row in rows}
    available = SimpleQueue()
    for cpu in cpus:
        available.put(cpu)
    active, lock, stopping = set(), threading.Lock(), threading.Event()

    def run_game(game):
        cpu = available.get()
        directory = args.out / "moves" / f"game-{game:03}"
        process = None
        try:
            if (directory / "completed.json").exists():
                return validate_row(
                    json.loads((directory / "completed.json").read_text()),
                    game,
                    directory,
                )
            archive_interrupted(directory)
            command = [
                "taskset",
                "-c",
                str(cpu),
                str(args.runner.resolve()),
                "--stockfish",
                str(args.stockfish.resolve()),
                "--games",
                "1",
                "--game-offset",
                str(game - 1),
                "--elo",
                str(args.elo),
                "--clock-ms",
                str(args.clock_ms),
                "--increment-ms",
                str(args.increment_ms),
                "--max-plies",
                str(args.max_plies),
                "--out",
                str(directory),
            ]
            with lock:
                if stopping.is_set():
                    raise RuntimeError("Match interrupted")
                process = subprocess.Popen(
                    command,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    start_new_session=True,
                )
                active.add(process)
            timeout = (
                2 * args.clock_ms + args.max_plies * args.increment_ms
            ) / 1000 + 120
            try:
                output, _ = process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired:
                terminate(process)
                output, _ = process.communicate(timeout=10)
                raise TimeoutError(
                    f"Game {game} exceeded the whole-game timeout: {output[-2000:]}"
                )
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "runner.log").write_text(output)
            if process.returncode:
                raise RuntimeError(f"Game {game} failed: {output[-2000:]}")
            with (directory / "games.tsv").open() as table:
                result = list(csv.DictReader(table, delimiter="\t"))
            if len(result) != 1:
                raise ValueError("Require one complete result from each game process")
            row = validate_row(result[0], game, directory)
            write_json(directory / "completed.json", row)
            return row
        finally:
            if process is not None:
                with lock:
                    active.discard(process)
                if process.poll() is None:
                    terminate(process)
            available.put(cpu)

    def save():
        write_games(args.out / "games.tsv", rows, FIELDS)
        report = dict(
            manifest=manifest,
            complete_schedule=len(rows) == args.games,
            completed_games=len(rows),
            planned_games=args.games,
            **summarize(rows, args.elo),
        )
        write_json(args.out / "report.json", report)

    executor = ThreadPoolExecutor(max_workers=len(cpus))
    try:
        futures = [
            executor.submit(run_game, game)
            for game in range(1, args.games + 1)
            if game not in done
        ]
        for future in as_completed(futures):
            row = future.result()
            rows.append(row)
            save()
            print(
                json.dumps(dict(completed=len(rows), total=args.games, **row)),
                flush=True,
            )
        save()
    except BaseException:
        stopping.set()
        executor.shutdown(wait=False, cancel_futures=True)
        with lock:
            for process in active:
                if process.poll() is None:
                    terminate(process)
        raise
    finally:
        executor.shutdown(wait=True, cancel_futures=True)


if __name__ == "__main__":
    main()
