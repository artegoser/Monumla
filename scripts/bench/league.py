#!/usr/bin/env python3
"""Provisional Elo league from complete paired matches, separate for every ruleset.

Ratings start at an explicit convention, not a claimed FIDE/Lichess calibration.
Each swapped-color pair updates both entrants together, with K per game.
"""

import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
from strength import summarize


def entrant(manifest, side):
    settings = manifest[f"{side}_settings"]
    identity = dict(binary_sha256=manifest[f"{side}_sha256"], settings=settings)
    key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()[:16]
    return key, identity


def update_pair(a, b, score, k):
    expected = 1 / (1 + 10 ** max(-100, min(100, (b - a) / 400)))
    change = 2 * k * (score - expected)
    return a + change, b - change


def league(directories, initial=1000, k=20):
    if not math.isfinite(initial) or not 0 < k <= 100:
        raise ValueError("Use a finite initial rating and K in (0,100]")
    pools = {}
    imported = set()
    for directory in directories:
        directory = Path(directory)
        raw = (directory / "manifest.json").read_bytes()
        table = (directory / "games.tsv").read_bytes()
        fingerprint = hashlib.sha256(raw + table).hexdigest()
        if fingerprint in imported:
            raise ValueError("Do not import the same match twice")
        imported.add(fingerprint)
        manifest = json.loads(raw)
        ruleset = {"go": "go:9", "tictactoe": "tictactoe:3x3:3"}.get(
            manifest["ruleset"], manifest["ruleset"]
        )
        pool = pools.setdefault(ruleset, dict(entrants={}, matches=[]))
        base, base_identity = entrant(manifest, "baseline")
        cand, cand_identity = entrant(manifest, "candidate")
        for key, identity in [(base, base_identity), (cand, cand_identity)]:
            pool["entrants"].setdefault(
                key,
                dict(
                    **identity, rating=initial, games=0, wdl=[0, 0, 0], provisional=True
                ),
            )
        rows = list(csv.DictReader(table.decode().splitlines(), delimiter="\t"))
        pairs = {}
        for row in rows:
            pairs.setdefault(row["pair"], []).append(row)
        accepted = excluded = 0
        for pair in sorted(pairs, key=int):
            games = pairs[pair]
            if len(games) != 2 or any(r["score"] == "NA" for r in games):
                excluded += 1
                continue
            if {r["side"] for r in games} != {"1", "2"} or len(
                {r["game"] for r in games}
            ) != 2:
                raise ValueError(
                    "A pair must have two distinct games with swapped sides"
                )
            scores = [float(r["score"]) for r in games]
            if any(s not in [0, 0.5, 1] for s in scores):
                raise ValueError("Invalid game score")
            if base == cand:
                continue
            a, b = pool["entrants"][cand], pool["entrants"][base]
            a["rating"], b["rating"] = update_pair(
                a["rating"], b["rating"], sum(scores) / 2, k
            )
            for score in scores:
                a["games"] += 1
                b["games"] += 1
                a["wdl"][0 if score == 1 else 1 if score == 0.5 else 2] += 1
                b["wdl"][2 if score == 1 else 1 if score == 0.5 else 0] += 1
            accepted += 1
        pool["matches"].append(
            dict(
                source=str(directory),
                candidate=cand,
                baseline=base,
                accepted_pairs=accepted,
                excluded_pairs=excluded,
                comparison=summarize(rows),
            )
        )
    for pool in pools.values():
        for entry in pool["entrants"].values():
            entry["rating_status"] = "provisional" if entry["games"] else "unrated"
            if not entry["games"]:
                entry["rating"] = None
    return dict(
        schema=1,
        method="paired online Elo; ratings initialized by convention; K per game",
        initial_rating=initial,
        k_per_game=k,
        calibration="Internal pool only. Not FIDE, Lichess or transferable across games. Provisional ratings depend on opponents, match order and initialization.",
        pools=pools,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "matches",
        type=Path,
        nargs="+",
        help="Match directories, in chronological order",
    )
    parser.add_argument("--initial", type=float, default=1000)
    parser.add_argument("--k", type=float, default=20)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError("Preserve an existing league output")
    result = league(args.matches, args.initial, args.k)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    for ruleset, pool in result["pools"].items():
        print(ruleset)
        for identity, entry in sorted(
            pool["entrants"].items(),
            key=lambda item: -(
                item[1]["rating"] if item[1]["rating"] is not None else -math.inf
            ),
        ):
            label = (
                f"{entry['rating']:.1f} provisional"
                if entry["rating"] is not None
                else "unrated"
            )
            print(
                f"  {identity} {label} · {entry['games']} games · {entry['wdl']} W/D/L · {entry['settings']}"
            )


if __name__ == "__main__":
    main()
