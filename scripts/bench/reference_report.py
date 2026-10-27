#!/usr/bin/env python3
"""Publish a completed Stockfish-calibrated result, including unbounded estimates."""

import argparse
import json
import math
from pathlib import Path

from match_storage import write_json


def finite(value):
    return isinstance(value, (int, float)) and math.isfinite(value)


def publish(report):
    if (
        not report["complete_schedule"]
        or report["completed_games"] != report["planned_games"]
    ):
        raise ValueError(
            "Finish the entire predeclared schedule before publishing a rating"
        )
    if not report["manifest"]["calibrated_time_control"]:
        raise ValueError(
            "Rated reference publication requires the documented 120s+1s control"
        )
    result = dict(report)
    result["rating_label"] = "Stockfish UCI_Elo reference scale"
    result["rating_status"] = (
        "unrated"
        if not report["complete_pairs"]
        else (
            "estimate"
            if finite(report["reference_scale_elo"]["point"])
            else (
                "upper_bound"
                if report["reference_scale_elo"]["point"] == "-inf"
                else "lower_bound"
            )
        )
    )
    return result


def plot(report, path):
    from matplotlib.figure import Figure

    fig = Figure(figsize=(9, 5.5), layout="constrained")
    ax = fig.subplots()
    rating = report["reference_scale_elo"]
    reference = report["reference_setting"]
    values = [float(value) for value in rating.values() if finite(value)] + [reference]
    bottom, top = min(values) - 250, max(values) + 250
    ax.set_ylim(bottom, top)
    ax.set_xlim(-0.7, 0.7)
    ax.axhline(reference, color="#777777", linestyle="--", linewidth=1)
    ax.text(
        0.68,
        reference,
        f"Stockfish setting {reference}",
        ha="right",
        va="bottom",
        color="#666666",
    )
    if report["rating_status"] == "unrated":
        ax.text(0, (bottom + top) / 2, "No completed rated pairs", ha="center")
    else:
        low = rating["low"] if finite(rating["low"]) else bottom + 30
        high = rating["high"] if finite(rating["high"]) else top - 30
        ax.vlines(0, low, high, color="#467238", linewidth=2)
        if finite(rating["low"]):
            ax.hlines(low, -0.07, 0.07, color="#467238", linewidth=2)
        else:
            ax.plot(0, low, marker="v", color="#467238")
        if finite(rating["high"]):
            ax.hlines(high, -0.07, 0.07, color="#467238", linewidth=2)
        else:
            ax.plot(0, high, marker="^", color="#467238")
        if finite(rating["point"]):
            ax.plot(0, rating["point"], marker="o", color="#467238", markersize=9)
            text, at = f"{rating['point']:.0f}", rating["point"]
        elif report["rating_status"] == "upper_bound":
            text, at = f"≤ {rating['high']:.0f} (95% upper bound)", high
        else:
            text, at = f"≥ {rating['low']:.0f} (95% lower bound)", low
        ax.annotate(
            text, (0, at), xytext=(15, 8), textcoords="offset points", weight="bold"
        )
    source = report["manifest"]["candidate_revision"][:7]
    ax.set_xticks([0], [f"Monumla · {source}"])
    ax.set_ylabel("Elo on the Stockfish UCI_Elo reference scale")
    ax.spines[["top", "right"]].set_visible(False)
    ax.grid(axis="y", alpha=0.2)
    wins, draws, losses = report["wdl"]
    fig.suptitle("Monumla · calibrated chess strength", fontsize=17)
    ax.set_title(
        f"{wins} wins · {draws} draws · {losses} losses · {report['unfinished_games']} unfinished\n120s + 1s · paired colors · {report['manifest']['baseline_revision']}",
        fontsize=11,
    )
    fig.supxlabel(
        "95% conservative paired score bound; reference-setting model, not a FIDE or Lichess rating.\nArrows mean an unbounded interval; no finite rating is invented from all losses or wins.",
        fontsize=9,
    )
    fig.savefig(path, dpi=150)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("benchmarks/elo.json"))
    parser.add_argument("--image", type=Path, default=Path("benchmarks/elo.png"))
    args = parser.parse_args()
    report = publish(json.loads((args.run / "report.json").read_text()))
    plot(report, args.image)
    write_json(args.out, report)


if __name__ == "__main__":
    main()
