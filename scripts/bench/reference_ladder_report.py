#!/usr/bin/env python3
"""Plot discovery scores and publish the separate confirmation rating."""

import argparse
import json
from pathlib import Path

from match_storage import write_json
from reference_ladder import calibration_report
from reference_report import plot, publish
from strength import interval


def discovery_points(calibration):
    levels = {}
    for run in calibration["runs"]:
        if run["role"] == "confirmation":
            continue
        report = publish(run["report"])
        if report["excluded_pairs"] or report["unfinished_games"]:
            raise ValueError("Discovery chart requires completed pairs")
        value = levels.setdefault(
            report["reference_setting"], dict(wdl=[0, 0, 0], samples=[])
        )
        value["wdl"] = [a + b for a, b in zip(value["wdl"], report["paired_wdl"])]
        for index, count in enumerate(report["pentanomial"]):
            value["samples"].extend([index / 4] * count)
    return [
        dict(
            level=level,
            wdl=value["wdl"],
            pairs=len(value["samples"]),
            score=sum(value["samples"]) / len(value["samples"]),
            interval_95=interval(value["samples"]),
        )
        for level, value in sorted(levels.items())
    ]


def final_report(calibration):
    if not calibration["complete"] or calibration["primary"] is None:
        raise ValueError(
            "Complete the fresh confirmation before replacing the published Elo"
        )
    confirmations = [r for r in calibration["runs"] if r["role"] == "confirmation"]
    if len(confirmations) != 1:
        raise ValueError("Require exactly one independent confirmation batch")
    primary = publish(calibration["primary"])
    if primary != publish(confirmations[0]["report"]):
        raise ValueError("Primary rating must come only from the confirmation")
    paths = [r["path"] for r in calibration["runs"]]
    if len(paths) != len(set(paths)):
        raise ValueError("Discovery games cannot be reused as confirmation")
    if primary["excluded_pairs"] or primary["unfinished_games"]:
        raise ValueError("Resolve unfinished confirmation games before publishing")
    result = dict(
        primary,
        calibration=dict(
            discovery=discovery_points(calibration),
            selection=calibration["selection"],
            inference=calibration["inference"],
            confirmation_games=primary["planned_games"],
            confirmation_path=confirmations[0]["path"],
        ),
    )
    for key in ["followup_validation", "equilibrium_search"]:
        if key in calibration:
            result["calibration"][key] = calibration[key]
    return result


def plot_scores(calibration, path):
    from matplotlib.figure import Figure

    points = discovery_points(calibration)
    fig = Figure(figsize=(9, 5.5), layout="constrained")
    ax = fig.subplots()
    low, high = calibration["selection"].get("target_score_band", [0.4, 0.6])
    ax.axhspan(
        low,
        high,
        color="#467238",
        alpha=0.07,
        label=f"Discovery target: {low:.0%}–{high:.0%}",
    )
    ax.axhline(0.5, color="#777777", linestyle="--", linewidth=1)
    for point in points:
        low, high = point["interval_95"]
        ax.errorbar(
            point["level"],
            point["score"],
            yerr=[[point["score"] - low], [high - point["score"]]],
            fmt="o",
            color="#467238",
            capsize=5,
        )
        w, d, loss = point["wdl"]
        ax.annotate(
            f"{w}W {d}D {loss}L",
            (point["level"], point["score"]),
            xytext=(7, 9),
            textcoords="offset points",
            fontsize=10,
        )
    if calibration["primary"] is not None:
        primary = calibration["primary"]
        w, d, loss = primary["paired_wdl"]
        at = (w + d / 2) / (w + d + loss)
        low, high = primary["score_interval_95"]
        ax.errorbar(
            primary["reference_setting"],
            at,
            yerr=[[at - low], [high - at]],
            fmt="D",
            color="#3278a4",
            capsize=6,
            label="Fresh confirmation",
        )
        ax.annotate(
            f"Confirmation: {w}W {d}D {loss}L",
            (primary["reference_setting"], at),
            xytext=(7, -18),
            textcoords="offset points",
            color="#3278a4",
            fontsize=10,
        )
    ax.set_ylim(0, 1)
    ax.margins(x=0.25)
    ax.set_xlabel("Stockfish UCI_Elo setting")
    ax.set_ylabel("Monumla score (win = 1, draw = ½)")
    ax.yaxis.set_major_formatter("{x:.0%}")
    ax.grid(axis="y", alpha=0.2)
    ax.spines[["top", "right"]].set_visible(False)
    ax.legend(loc="lower left")
    fig.suptitle("Monumla · chess reference levels", fontsize=17)
    search = calibration.get("equilibrium_search", {})
    if search.get("status") == "bracketed":
        low, high = search["observed_crossover"]
        detail = f"Observed crossover: {low}–{high} · "
    elif search.get("status") == "observed_even":
        detail = f"Observed even matchup: {search['level']} · "
    else:
        detail = ""
    ax.set_title(detail + "120s + 1s · swapped-color pairs", fontsize=11)
    fig.supxlabel(
        "Bars: conservative paired 95% bounds for each setting. Discovery selects the opponent;\nonly the separate fixed-size confirmation determines the final rating interval.",
        fontsize=9,
    )
    fig.savefig(path, dpi=150)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("benchmarks/elo.json"))
    parser.add_argument("--image", type=Path, default=Path("benchmarks/elo.png"))
    parser.add_argument(
        "--curve", type=Path, default=Path("benchmarks/elo-calibration.png")
    )
    args = parser.parse_args()
    calibration = calibration_report(json.loads((args.run / "plan.json").read_text()))
    plot_scores(calibration, args.curve)
    if calibration["complete"]:
        report = final_report(calibration)
        plot(report, args.image)
        write_json(args.out, report)
    else:
        print(
            "Discovery curve updated; confirmation incomplete, published Elo retained."
        )


if __name__ == "__main__":
    main()
