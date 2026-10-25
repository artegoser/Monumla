#!/usr/bin/env python3
"""Record verified fixed-work improvements; render a compact, reproducible PNG."""

import argparse
import hashlib
import json
import math
from datetime import datetime, timezone
from pathlib import Path


def geometric(values):
    values = list(values)
    return math.exp(sum(math.log(v) for v in values) / len(values))


def accept(history, report, baseline_revision, candidate_revision, measured_at):
    rows = report["summaries"]
    if not rows or not all(r["same_work_and_answer"] for r in rows):
        raise ValueError(
            "Require identical completed depth, score, PV, nodes and stop reason"
        )
    if not all(r["candidate"]["max_ms"] < r["baseline"]["min_ms"] for r in rows):
        raise ValueError(
            "No clear improvement on every fixture: observed timing ranges overlap or regress"
        )
    signature = {k: report["settings"][k] for k in ["ruleset", "depth", "nodes", "cpu"]}
    signature["opening_plies"] = report["settings"].get("opening_plies", 0)
    signature["lines"] = report["settings"].get("lines", 1)
    if report["settings"].get("prelude"):
        signature["prelude_sha256"] = hashlib.sha256(
            Path(report["settings"]["prelude"]).read_bytes()
        ).hexdigest()
    signature["fixtures"] = {
        r["fixture"]: hashlib.sha256(
            Path(r["fixture"]).read_bytes()
            if Path(r["fixture"]).is_file()
            else json.dumps(
                {
                    "ruleset": signature["ruleset"],
                    "opening_plies": signature["opening_plies"],
                    "seed": 1,
                },
                sort_keys=True,
            ).encode()
        ).hexdigest()
        for r in rows
    }
    point = dict(
        revision=candidate_revision,
        binary_sha256=report["candidate_sha256"],
        measured_at=measured_at,
        timings={r["fixture"]: r["candidate"] for r in rows},
    )
    if not history:
        baseline = dict(
            revision=baseline_revision,
            binary_sha256=report["baseline_sha256"],
            measured_at=measured_at,
            timings={r["fixture"]: r["baseline"] for r in rows},
            relative_throughput=1.0,
        )
        history = dict(
            schema=1,
            runtime=report.get("runtime", "native release"),
            platform=report["platform"],
            suite=signature,
            points=[baseline],
        )
    elif history["suite"] != signature or history["platform"] != report["platform"]:
        raise ValueError(
            "Benchmark suite, CPU or platform changed; start a separate series"
        )
    elif history["points"][-1]["binary_sha256"] != report["baseline_sha256"]:
        raise ValueError(
            "Compare against the last accepted binary, not a cherry-picked older baseline"
        )
    if point["binary_sha256"] == history["points"][-1]["binary_sha256"]:
        raise ValueError("The candidate is the same binary")
    reference = history["points"][0]["timings"]
    point["relative_throughput"] = geometric(
        reference[name]["median_ms"] / value["median_ms"]
        for name, value in point["timings"].items()
    )
    if point["relative_throughput"] <= history["points"][-1]["relative_throughput"]:
        raise ValueError("Aggregate performance did not improve")
    history["points"].append(point)
    return history


def plot(history, path):
    from matplotlib.figure import Figure

    series = list(history.get("series", {"single": history}).values())
    columns = min(3, len(series))
    fig = Figure(
        figsize=(6.4 * columns, 4.5 * math.ceil(len(series) / columns)),
        layout="constrained",
    )
    axes = fig.subplots(math.ceil(len(series) / columns), columns, squeeze=False).flat
    axes = list(axes)
    for ax, entry in zip(axes, series):
        points = entry["points"]
        reference = points[0]["timings"]
        y = [p["relative_throughput"] for p in points]
        lo = [
            geometric(
                reference[n]["median_ms"] / v["max_ms"] for n, v in p["timings"].items()
            )
            for p in points
        ]
        hi = [
            geometric(
                reference[n]["median_ms"] / v["min_ms"] for n, v in p["timings"].items()
            )
            for p in points
        ]
        ax.errorbar(
            range(len(points)),
            y,
            yerr=[[v - l for v, l in zip(y, lo)], [h - v for v, h in zip(y, hi)]],
            color="#4b7634",
            marker="o",
            linewidth=2,
            capsize=5,
        )
        for i, value in enumerate(y):
            ax.annotate(
                f"{value:.2f}×",
                (i, value),
                xytext=(0, 9),
                textcoords="offset points",
                ha="center",
                weight="bold",
            )
        ax.set_xticks(
            range(len(points)),
            [f"{p['measured_at'][:10]}\n{p['revision'][:7]}" for p in points],
        )
        ax.set_ylim(0, max(hi) * 1.2)
        ax.set_ylabel("Fixed-depth throughput / suite baseline")
        ax.set_title(
            f"{entry['runtime']} · {entry['suite'].get('id', entry['suite']['ruleset'])} · depth {entry['suite']['depth']}",
            loc="left",
            fontsize=11,
            pad=18,
        )
        ax.spines[["top", "right"]].set_visible(False)
        ax.grid(axis="y", alpha=0.2)
    for ax in list(axes)[len(series) :]:
        ax.set_visible(False)
    fig.suptitle("Monumla · verified performance milestones", fontsize=17)
    fig.supxlabel(
        "Separate suites; baselines are not comparable across panels. Bars: observed spread, not confidence intervals.",
        fontsize=9,
    )
    fig.savefig(path, dpi=150)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--series", default="native-chess-d3")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument(
        "--journal", type=Path, default=Path("benchmarks/performance.json")
    )
    parser.add_argument(
        "--image", type=Path, default=Path("benchmarks/performance.png")
    )
    parser.add_argument("--baseline-revision", required=True)
    parser.add_argument("--candidate-revision", required=True)
    parser.add_argument("--measured-at", default=datetime.now(timezone.utc).isoformat())
    args = parser.parse_args()
    database = (
        json.loads(args.journal.read_text())
        if args.journal.exists()
        else {"schema": 2, "series": {}}
    )
    if "series" not in database:
        database["suite"].setdefault("opening_plies", 0)
        database["suite"].setdefault("lines", 1)
        database = {"schema": 2, "series": {"native-chess-d3": database}}
    result = accept(
        database["series"].get(args.series),
        json.loads(args.report.read_text()),
        args.baseline_revision,
        args.candidate_revision,
        args.measured_at,
    )
    database["series"][args.series] = result
    plot(database, args.image)
    args.journal.write_text(json.dumps(database, indent=2) + "\n")
    print(
        f"Recorded {result['points'][-1]['relative_throughput']:.2f}× suite baseline; {args.image}"
    )


if __name__ == "__main__":
    main()
