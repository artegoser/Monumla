#!/usr/bin/env python3
"""Find a comparable Stockfish setting, then run an independent fixed-size match."""

import argparse
import csv
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

from match_storage import write_json
from reference_match import validate_row
from reference_report import publish
from strength import sha256, summarize

IDENTITY = (
    "baseline_sha256",
    "candidate_sha256",
    "baseline_revision",
    "candidate_revision",
    "candidate_settings",
    "opening_plies",
    "max_plies",
    "jobs",
    "cpus",
    "platform",
    "ruleset",
)
DEFAULT_SELECTION = dict(
    target_score_band=[0.4, 0.6],
    unbracketed_step=200,
    minimum_elo=1320,
    maximum_elo=3190,
    maximum_probe_batches=8,
)
REPORT_METADATA = ["followup_validation", "equilibrium_search", "audit_summary"]


def choose_level(reports, selection):
    """Combine repeated levels; never choose from an incomplete pair or batch."""
    totals = {}
    for report in reports:
        publish(report)
        if report["excluded_pairs"] or report["unfinished_games"]:
            raise ValueError("Resolve unfinished games before selecting another level")
        level = report["reference_setting"]
        wins, draws, losses = report["paired_wdl"]
        points, games = totals.get(level, (0, 0))
        totals[level] = (points + wins + draws / 2, games + wins + draws + losses)
    scores = {level: points / games for level, (points, games) in totals.items()}
    low, high = selection["target_score_band"]
    for report in reversed(reports):
        level = report["reference_setting"]
        if low <= scores[level] <= high:
            return level, True
    lower = [level for level, value in scores.items() if value > high]
    upper = [level for level, value in scores.items() if value < low]
    if lower and upper:
        if max(lower) >= min(upper):
            raise ValueError("Nonmonotone probe results do not bracket an equilibrium")
        # Python's round uses ties-to-even; keep the exact rule reproducible.
        level = int(25 * round((max(lower) + min(upper)) / 50))
    else:
        latest = reports[-1]["reference_setting"]
        direction = 1 if scores[latest] > high else -1
        level = latest + direction * selection["unbracketed_step"]
    return max(selection["minimum_elo"], min(selection["maximum_elo"], level)), False


def validate_manifest(manifest, template, entry):
    for key in IDENTITY:
        if manifest[key] != template[key]:
            raise ValueError(f"Calibration requires the same {key}")
    expected = dict(template["baseline_settings"], elo=entry["elo"])
    if manifest["baseline_settings"] != expected or manifest["games"] != entry["games"]:
        raise ValueError("Batch settings do not match the declared calibration plan")
    if not manifest["calibrated_time_control"]:
        raise ValueError("Calibration requires 120s+1s")


def load_completed(entry, template):
    root = Path(entry["path"])
    manifest = json.loads((root / "manifest.json").read_text())
    validate_manifest(manifest, template, entry)
    if not (root / "report.json").exists():
        return None
    stored = json.loads((root / "report.json").read_text())
    if not stored["complete_schedule"]:
        return None
    with (root / "games.tsv").open() as table:
        rows = list(csv.DictReader(table, delimiter="\t"))
    ids = [int(row["game"]) for row in rows]
    if sorted(ids) != list(range(1, entry["games"] + 1)):
        raise ValueError("Require every scheduled game exactly once")
    rows = [
        validate_row(
            row, int(row["game"]), root / "moves" / f"game-{int(row['game']):03}"
        )
        for row in rows
    ]
    report = dict(
        manifest=manifest,
        complete_schedule=True,
        completed_games=len(rows),
        planned_games=entry["games"],
        **summarize(rows, entry["elo"]),
    )
    if report["excluded_pairs"] or report["unfinished_games"]:
        raise ValueError("Unfinished pairs cannot silently enter a calibration")
    return publish(report)


def match_command(entry, template):
    root = Path(entry["path"])
    command = [
        sys.executable,
        str(Path(__file__).with_name("reference_match.py")),
        "--runner",
        template["candidate"],
        "--stockfish",
        template["baseline"],
        "--candidate-revision",
        template["candidate_revision"],
        "--games",
        str(entry["games"]),
        "--elo",
        str(entry["elo"]),
        "--clock-ms",
        "120000",
        "--increment-ms",
        "1000",
        "--max-plies",
        str(template["max_plies"]),
        "--cpus",
        template["cpus"],
        "--out",
        str(root),
    ]
    if (root / "manifest.json").exists():
        command.append("--resume")
    return command


def calibration_report(plan):
    reports = []
    for entry in plan["runs"]:
        if not (Path(entry["path"]) / "manifest.json").exists():
            break
        report = load_completed(entry, plan["template"])
        if report is None:
            break
        reports.append(dict(role=entry["role"], path=entry["path"], report=report))
    confirmation = next(
        (r["report"] for r in reports if r["role"] == "confirmation"), None
    )
    result = dict(
        schema=1,
        selection=plan["selection"],
        runs=reports,
        complete=confirmation is not None and len(reports) == len(plan["runs"]),
        primary=confirmation,
        inference="Discovery batches select the opponent. Only fresh fixed-size confirmation games determine the final rating interval.",
    )
    for key in REPORT_METADATA:
        if key in plan:
            result[key] = plan[key]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--seed", type=Path, help="Existing first probe, complete or resumable"
    )
    parser.add_argument(
        "--pilot", type=Path, help="Earlier completed pilot, used for discovery only"
    )
    parser.add_argument(
        "--selection-plan",
        type=Path,
        help="Retain an earlier predeclared selection plan",
    )
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    destination = args.out / "plan.json"
    if args.resume:
        if args.seed or args.pilot or args.selection_plan:
            parser.error("Resume uses the retained plan; do not replace its inputs")
        plan = json.loads(destination.read_text())
    else:
        if not args.seed:
            parser.error("A new calibration requires --seed")
        args.out.mkdir(parents=True, exist_ok=True)
        if any(args.out.iterdir()):
            parser.error("Output must be empty; use --resume")
        template = json.loads((args.seed / "manifest.json").read_text())
        runs = []
        for role, root in [("pilot", args.pilot), ("probe", args.seed)]:
            if root is not None:
                manifest = json.loads((root / "manifest.json").read_text())
                entry = dict(
                    role=role,
                    path=str(root.resolve()),
                    elo=manifest["baseline_settings"]["elo"],
                    games=manifest["games"],
                )
                validate_manifest(manifest, template, entry)
                runs.append(entry)
        if len({r["path"] for r in runs}) != len(runs):
            parser.error("Pilot and probe must be different runs")
        plan = dict(
            schema=1,
            started_at=datetime.now(timezone.utc).isoformat(),
            template=template,
            selection=DEFAULT_SELECTION,
            probe_games=template["games"],
            confirmation_games=48,
            runs=runs,
        )
        if args.selection_plan:
            declared = json.loads(args.selection_plan.read_text())
            if (
                any(
                    declared["selection"][key] != value
                    for key, value in DEFAULT_SELECTION.items()
                )
                or declared["confirmation"]["games"] != plan["confirmation_games"]
            ):
                raise ValueError(
                    "Earlier declaration does not match the implemented schedule"
                )
            plan["earlier_declaration"] = dict(
                path=str(args.selection_plan.resolve()),
                sha256=sha256(args.selection_plan),
                contents=declared,
            )
        write_json(destination, plan)
    template = plan["template"]
    for key in ["baseline", "candidate"]:
        if sha256(template[key]) != template[f"{key}_sha256"]:
            raise ValueError(f"Frozen {key} binary changed")
    while True:
        for entry in plan["runs"]:
            if (Path(entry["path"]) / "manifest.json").exists():
                report = load_completed(entry, template)
                if report is not None:
                    continue
            if entry["role"] == "pilot":
                raise ValueError("Imported pilot must already be complete")
            print(
                json.dumps(
                    dict(phase=entry["role"], level=entry["elo"], games=entry["games"])
                ),
                flush=True,
            )
            subprocess.run(match_command(entry, template), check=True)
            if load_completed(entry, template) is None:
                raise ValueError(
                    "Match exited without completing its declared schedule"
                )
        result = calibration_report(plan)
        write_json(args.out / "calibration.json", result)
        if result["complete"]:
            print(
                json.dumps(
                    dict(
                        phase="complete",
                        rating=result["primary"]["reference_scale_elo"],
                    )
                ),
                flush=True,
            )
            return
        reports = [r["report"] for r in result["runs"]]
        level, near = choose_level(reports, plan["selection"])
        probes = sum(r["role"] == "probe" for r in plan["runs"])
        if not near and probes >= plan["selection"]["maximum_probe_batches"]:
            raise ValueError(
                "Discovery limit reached without an equilibrium; no confirmed rating published"
            )
        role = "confirmation" if near else "probe"
        games = plan["confirmation_games"] if near else plan["probe_games"]
        path = args.out.resolve() / f"{len(plan['runs']):02}-{role}-{level}"
        if path.exists():
            raise ValueError("New batch path already exists outside the retained plan")
        plan["runs"].append(dict(role=role, path=str(path), elo=level, games=games))
        # Save the entire batch before playing; interrupted runs resume this exact entry.
        write_json(destination, plan)


if __name__ == "__main__":
    main()
