#!/usr/bin/env python3
"""Complete strength evidence and a gain-only history; no invented absolute Elo."""

import argparse
import csv
import hashlib
import json
import math
from datetime import datetime, timezone
from pathlib import Path
from collections import Counter
from league import canonical_ruleset as pool_name, entrant, league
from match_storage import write_json
from strength import summarize


def collect(directories, cases):
    pools = {pool_name(case["ruleset"]): [] for case in cases}
    imported = set()
    for directory in directories:
        directory = Path(directory)
        raw_manifest = (directory / "manifest.json").read_bytes()
        raw_games = (directory / "games.tsv").read_bytes()
        fingerprint = hashlib.sha256(raw_manifest + raw_games).hexdigest()
        if fingerprint in imported:
            raise ValueError("Duplicate match import")
        imported.add(fingerprint)
        manifest = json.loads(raw_manifest)
        rows = list(csv.DictReader(raw_games.decode().splitlines(), delimiter="\t"))
        pairs = {}
        for row in rows:
            pairs.setdefault(row["pair"], []).append(row)
        for pair in pairs.values():
            if len(pair) > 2 or (
                len(pair) == 2
                and (
                    {r["side"] for r in pair} != {"1", "2"}
                    or len({r["game"] for r in pair}) != 2
                )
            ):
                raise ValueError(
                    "Match pairs must contain distinct games with swapped sides"
                )
        summary = summarize(rows)
        predeclared = manifest.get("paired_game_count_predeclared")
        complete_plan = (
            predeclared is not None
            and predeclared == len(rows)
            and summary["excluded_pairs"] == 0
            and summary["complete_pairs"] * 2 == len(rows)
        )
        # A point rating rise, all wins in a tiny sample, or an unfinished
        # schedule is insufficient for promotion to the improvement history.
        supported_gain = complete_plan and summary["score_interval_95"][0] > 0.5
        identities = {}
        for side in ["baseline", "candidate"]:
            key, identity = entrant(manifest, side)
            identities[side] = dict(
                id=key, **identity, revision=manifest.get(f"{side}_revision")
            )
        row = dict(
            match_sha256=fingerprint,
            measured_at=manifest.get("started_at"),
            identities=identities,
            opening_plies=manifest.get("opening_plies"),
            max_plies=manifest.get("max_plies"),
            platform=manifest.get("platform"),
            predeclared_games=predeclared,
            plan_complete=complete_plan,
            runner_revision=manifest.get("runner_revision"),
            runner_dirty=manifest.get("runner_dirty"),
            resumes=manifest.get("resumes", []),
            terminations=dict(
                Counter(row.get("termination", "unspecified") for row in rows)
            ),
            forfeits=sum("forfeit" in row.get("termination", "") for row in rows),
            supported_gain=supported_gain
            and identities["baseline"]["id"] != identities["candidate"]["id"],
            **summary,
        )
        pools.setdefault(pool_name(manifest["ruleset"]), []).append(row)
    return dict(
        schema=1,
        generated_at=datetime.now(timezone.utc).isoformat(),
        method="Relative Elo from complete swapped-side pairs, conservative fixed-N 95% bounds. Missing pools are unrated. Gain history requires the full predeclared schedule and lower expected-score bound > 0.5. Not an absolute FIDE/Lichess rating or a sequential test.",
        pools=pools,
    )


def record_gains(database, report):
    known = {row["match_sha256"] for row in database["milestones"]}

    def series(pool, row):
        return json.dumps(
            dict(
                pool=pool,
                budgets={
                    side: row["identities"][side]["settings"]
                    for side in ["baseline", "candidate"]
                },
                opening_plies=row["opening_plies"],
                max_plies=row["max_plies"],
                platform=row.get("platform"),
            ),
            sort_keys=True,
        )

    latest = {series(row["pool"], row): row for row in database["milestones"]}
    for pool, rows in report["pools"].items():
        for row in rows:
            if row["supported_gain"] and row["match_sha256"] not in known:
                key = series(pool, row)
                previous = latest.get(key)
                if (
                    previous
                    and previous["identities"]["candidate"]["id"]
                    != row["identities"]["baseline"]["id"]
                ):
                    raise ValueError(
                        "Compare against the last accepted entrant with the same pool and protocol; do not recycle an older baseline"
                    )
                database["milestones"].append(dict(pool=pool, **row))
                latest[key] = row
                known.add(row["match_sha256"])
    return database


def plot(report, destination, title="Monumla · strength evidence by pool"):
    from matplotlib.figure import Figure

    fig = Figure(
        figsize=(14, max(5, len(report["pools"]) * 0.7 + 1.5)), layout="constrained"
    )
    ax = fig.subplots()
    ax.axvline(0, color="#888888", linewidth=1)
    labels = []
    index = 0
    cap = 800

    def number(value):
        return float(value) if value is not None else None

    for pool, rows in report["pools"].items():
        for row in rows or [None]:
            labels.append(pool)
            if row is None or not row["complete_pairs"]:
                ax.text(
                    -cap + 20,
                    index,
                    "Unrated · no complete pairs"
                    + (f" · excluded {row['excluded_pairs']}" if row else ""),
                    va="center",
                    color="#777777",
                )
            else:
                low, high = [number(row["delta_elo"][key]) for key in ["low", "high"]]
                point = number(row["delta_elo"]["point"])
                clipped = lambda value: max(-cap, min(cap, value))
                color = "#39733c" if row["supported_gain"] else "#687787"
                ax.plot(
                    [clipped(low), clipped(high)],
                    [index, index],
                    color=color,
                    linewidth=2,
                )
                if math.isfinite(point):
                    ax.plot(clipped(point), index, marker="o", color=color)
                else:
                    ax.plot(
                        clipped(point),
                        index,
                        marker=">" if point > 0 else "<",
                        color=color,
                    )
                for endpoint in [low, high]:
                    if abs(endpoint) > cap:
                        ax.plot(
                            clipped(endpoint),
                            index,
                            marker=">" if endpoint > 0 else "<",
                            color=color,
                        )
                ids = row["identities"]
                label = (
                    lambda side: f"{(ids[side]['revision'] or ids[side]['binary_sha256'])[:7]} / {ids[side]['settings']['time_ms']}ms"
                )
                point_text = (
                    f"Δ {point:+.1f}" if math.isfinite(point) else "no finite estimate"
                )
                ax.text(
                    cap + 35,
                    index,
                    f"{label('baseline')} → {label('candidate')}\n{row['wdl']} W/D/L · {row['complete_pairs']} pairs · {point_text}"
                    + (f"\nForfeits: {row['forfeits']}" if row.get("forfeits") else "")
                    + (
                        f" · excluded pairs: {row['excluded_pairs']}"
                        if row["excluded_pairs"]
                        else ""
                    ),
                    va="center",
                    fontsize=8,
                )
            index += 1
    ax.set_yticks(range(index), labels)
    ax.set_ylim(index - 0.5, -0.5)
    ax.set_xlim(-cap - 30, cap * 2.1)
    ax.set_xticks([-800, -400, 0, 400, 800], ["−800", "−400", "0", "+400", "+800"])
    ax.set_xlabel(
        "Candidate relative Elo · 95% pair-based bounds · arrows indicate bounds/estimates beyond plotted range"
    )
    ax.spines[["top", "right"]].set_visible(False)
    if report.get("matrix"):
        revisions = report["matrix"]["revisions"]
        title += f" · {revisions['baseline'][:7]} → {revisions['candidate'][:7]}"
    fig.suptitle(title, fontsize=17)
    fig.supxlabel(
        "All results retained. Missing/unfinished pools remain unrated. Budgets identify distinct entrants; unknown source revisions are labeled by binary hash.",
        fontsize=9,
    )
    fig.savefig(destination, dpi=150)


def plot_history(history, destination):
    if history["milestones"]:
        pools = {}
        for row in history["milestones"]:
            pools.setdefault(row["pool"], []).append(row)
        plot(
            dict(pools=pools),
            destination,
            "Monumla · supported relative Elo improvements",
        )
    else:
        from matplotlib.figure import Figure

        fig = Figure(figsize=(10, 3), layout="constrained")
        ax = fig.subplots()
        ax.axis("off")
        ax.text(
            0.5,
            0.6,
            "No statistically supported strength gains recorded yet",
            ha="center",
            fontsize=16,
        )
        ax.text(
            0.5,
            0.35,
            "No complete predeclared comparison has established a positive lower bound.\nMissing pools stay unrated; complete evidence is shown in strength.png.",
            ha="center",
            fontsize=11,
        )
        fig.savefig(destination, dpi=150)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("matches", nargs="*", type=Path)
    parser.add_argument(
        "--matrix", type=Path, help="Verify and publish a complete all-pool tournament"
    )
    parser.add_argument("--cases", type=Path, default=Path("benchmarks/cases.json"))
    parser.add_argument(
        "--out", type=Path, default=Path("benchmarks/latest-strength.json")
    )
    parser.add_argument("--image", type=Path, default=Path("benchmarks/strength.png"))
    parser.add_argument(
        "--journal", type=Path, default=Path("benchmarks/strength-history.json")
    )
    parser.add_argument(
        "--history-image", type=Path, default=Path("benchmarks/strength-history.png")
    )
    parser.add_argument(
        "--league", type=Path, default=Path("benchmarks/elo-pools.json")
    )
    args = parser.parse_args()
    if bool(args.matches) == bool(args.matrix):
        parser.error("Supply match directories or --matrix, exclusively")
    coverage = json.loads(args.cases.read_text())
    if args.matrix:
        from strength_matrix_report import verify_matrix

        metadata, directories, completed = verify_matrix(args.matrix, coverage)
        report = collect(directories, coverage)
        report["matrix"] = metadata
        for pool, error in metadata["failures"].items():
            for row in report["pools"][pool]:
                row.update(run_error=error, supported_gain=False, plan_complete=False)
    else:
        directories = completed = args.matches
        report = collect(directories, coverage)
    history = (
        json.loads(args.journal.read_text())
        if args.journal.exists()
        else dict(schema=1, milestones=[])
    )
    history = record_gains(history, report)
    previous_league = (
        json.loads(args.league.read_text()) if args.league.exists() else None
    )
    updated_league = league(completed, previous=previous_league)
    write_json(args.out, report)
    write_json(args.journal, history)
    write_json(args.league, updated_league)
    plot(report, args.image)
    plot_history(history, args.history_image)


if __name__ == "__main__":
    main()
