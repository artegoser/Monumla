#!/usr/bin/env python3
"""Publish complete matrix coverage, including noisy or slower cases."""

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path


def compact(native, wasm):
    expected = {
        name: native["settings"][f"{name}_revision"]
        for name in ["baseline", "candidate"]
    }
    if wasm["source_revisions"] != expected:
        raise ValueError("Both runtimes must compare the same source revisions")
    if [r["case"] for r in native["results"]] != [r["case"] for r in wasm["results"]]:
        raise ValueError("Both runtimes must cover the same cases")
    repeats = native["settings"]["repeats"]
    if repeats != int(wasm["settings"].get("repeats", 5)):
        raise ValueError("Both runtimes must use the same repeat count")
    rows = []
    for runtime, matrix in [("native", native), ("wasm", wasm)]:
        for source in matrix["results"]:
            row = {"runtime": runtime, "case": source["case"]}
            summary = source.get("summaries", [source])[0]
            row.update(
                {
                    key: summary[key]
                    for key in [
                        "same_work_and_answer",
                        "baseline",
                        "candidate",
                        "speedup",
                    ]
                    if key in summary
                }
            )
            if source.get("error") or source.get("returncode"):
                row["error"] = source.get(
                    "error", f"native process exit {source['returncode']}"
                )
            row["verified_gain"] = (
                not row.get("error")
                and row.get("same_work_and_answer", False)
                and row["candidate"]["max_ms"] < row["baseline"]["min_ms"]
            )
            rows.append(row)
    return dict(
        schema=1,
        measured_at=datetime.now(timezone.utc).isoformat(),
        revisions=expected,
        native_binary_sha256={side: native[f"{side}_sha256"] for side in expected},
        wasm_binary_sha256=wasm["wasm_sha256"],
        node=wasm["runtime"]["node"],
        cpu=wasm["cpu"],
        repetitions=repeats,
        method=f"One warmup, {repeats} alternating repetitions, pinned CPUs; same depth, nodes, score, PV, lines and stop reason required within each runtime. Observed ranges are not confidence intervals. Native and WASM timings are separate measurements.",
        results=rows,
    )


def plot(report, destination):
    from matplotlib.figure import Figure

    fig = Figure(figsize=(14, 9), layout="constrained")
    axes = fig.subplots(1, 2)
    for ax, runtime in zip(axes, ["native", "wasm"]):
        rows = [row for row in report["results"] if row["runtime"] == runtime]
        ax.axvline(1, color="#888888", linewidth=1)
        for index, row in enumerate(rows):
            if row.get("error") or not row.get("same_work_and_answer"):
                ax.text(0.05, index, "FAILED / CHANGED WORK", color="#b43a36")
                continue
            value = row["speedup"]
            color = (
                "#39733c"
                if row["verified_gain"]
                else "#b43a36" if value < 1 else "#858585"
            )
            ax.barh(index, value, color=color, height=0.6)
            low = row["baseline"]["min_ms"] / row["candidate"]["max_ms"]
            high = row["baseline"]["max_ms"] / row["candidate"]["min_ms"]
            ax.errorbar(
                value,
                index,
                xerr=[[max(0, value - low)], [max(0, high - value)]],
                color="#333333",
                capsize=3,
                fmt="none",
            )
            ax.text(
                max(high, value) + 0.04, index, f"{value:.2f}×", va="center", fontsize=9
            )
        ax.set_yticks(
            range(len(rows)),
            [f"{r['case']['id']} · d{r['case']['depth']}" for r in rows],
            fontsize=9,
        )
        ax.invert_yaxis()
        ax.set_xlim(
            0,
            max(
                [
                    r.get("baseline", {}).get("max_ms", 1)
                    / r.get("candidate", {}).get("min_ms", 1)
                    for r in rows
                ]
            )
            * 1.18,
        )
        ax.set_title(
            (
                "Native release"
                if runtime == "native"
                else f"WASM release · Node {report['node']}"
            ),
            loc="left",
        )
        ax.set_xlabel("Baseline median time / candidate median time (higher is faster)")
        ax.spines[["top", "right"]].set_visible(False)
    fig.suptitle(
        f"Monumla · all-game comparison · {report['revisions']['baseline'][:7]} → {report['revisions']['candidate'][:7]}",
        fontsize=17,
    )
    fig.supxlabel(
        "Green: non-overlapping observed timing ranges. Gray: noisy gain. Red: slower median. Whiskers: observed extremes, not confidence intervals.",
        fontsize=9,
    )
    fig.savefig(destination, dpi=150)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--wasm", type=Path, required=True)
    parser.add_argument(
        "--out", type=Path, default=Path("benchmarks/latest-matrix.json")
    )
    parser.add_argument(
        "--image", type=Path, default=Path("benchmarks/performance.png")
    )
    args = parser.parse_args()
    report = compact(
        json.loads(args.native.read_text()), json.loads(args.wasm.read_text())
    )
    args.out.write_text(json.dumps(report, indent=2) + "\n")
    plot(report, args.image)


if __name__ == "__main__":
    main()
