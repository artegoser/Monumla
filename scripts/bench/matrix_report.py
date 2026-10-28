#!/usr/bin/env python3
"""Publish complete matrix coverage, including noisy or slower cases."""

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path


def compact(native, wasm):
    build_id = native["settings"].get("build_id")
    if build_id != wasm["settings"].get("build-id"):
        raise ValueError("Both runtimes must use the same compiler/build identity")
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
    warmups = native["settings"].get("warmups", 1)
    if warmups != int(wasm["settings"].get("warmups", 1)):
        raise ValueError("Both runtimes must use the same warmup count")
    warmup_ms = native["settings"].get("warmup_ms", 0)
    if warmup_ms != float(wasm["settings"].get("warmup-ms", 0)):
        raise ValueError("Both runtimes must use the same warmup duration")
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
                    "error", f"native process exit {source.get('returncode')}"
                )
            row["verified_gain"] = (
                not row.get("error")
                and row.get("same_work_and_answer", False)
                and row["candidate"]["max_ms"] < row["baseline"]["min_ms"]
            )
            rows.append(row)
    report = dict(
        schema=1,
        measured_at=datetime.now(timezone.utc).isoformat(),
        revisions=expected,
        native_binary_sha256={side: native[f"{side}_sha256"] for side in expected},
        wasm_binary_sha256=wasm["wasm_sha256"],
        node=wasm["runtime"]["node"],
        cpu=wasm["cpu"],
        repetitions=repeats,
        warmups=warmups,
        warmup_ms=warmup_ms,
        method=f"At least {warmups} warmups and {warmup_ms} ms warmup work per binary, then {repeats} alternating measured repetitions, pinned CPUs; same depth, nodes, score, PV, lines and stop reason required within each runtime. Observed ranges are not confidence intervals. Native and WASM timings are separate measurements.",
        results=rows,
    )
    if build_id:
        report["build_id"] = build_id
    return report


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
            label = f"{value:.3f}×" if abs(value - 1) < 0.01 else f"{value:.2f}×"
            ax.text(max(high, value) + 0.04, index, label, va="center", fontsize=9)
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
            )
            + (f"\n{report['build_id']}" if report.get("build_id") else ""),
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


def record_gains(database, report):
    """Append only clear, fixed-work gains; never erase a slower matrix row."""
    for row in report["results"]:
        if not row["verified_gain"]:
            continue
        runtime, case = row["runtime"], row["case"]
        name = f"{runtime}-{case['id']}"
        if report.get("build_id"):
            name += f"-{report['build_id']}"
        fingerprints = {
            key: hashlib.sha256(Path(case[key]).read_bytes()).hexdigest()
            for key in ["pgn", "prelude"]
            if key in case
        }
        suite = dict(**case, lines=1, seed=1, fixture_sha256=fingerprints)
        if report.get("build_id"):
            suite["warmups"] = report["warmups"]
            suite["warmup_ms"] = report["warmup_ms"]
        binaries = report[f"{runtime}_binary_sha256"]
        history = database["series"].get(name)
        if history and (
            history["suite"] != suite
            or history["points"][-1]["binary_sha256"] != binaries["baseline"]
        ):
            raise ValueError(
                f"{name}: compare against the last accepted binary with the same workload"
            )

        def point(side, relative):
            return dict(
                revision=report["revisions"][side],
                binary_sha256=binaries[side],
                measured_at=report["measured_at"],
                timings={case["id"]: row[side]},
                relative_throughput=relative,
            )

        if not history:
            history = dict(
                runtime=runtime, suite=suite, points=[point("baseline", 1.0)]
            )
            if report.get("build_id"):
                history["build_id"] = report["build_id"]
            database["series"][name] = history
        history["points"].append(
            point(
                "candidate",
                history["points"][-1]["relative_throughput"] * row["speedup"],
            )
        )
    return database


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
    parser.add_argument("--journal", type=Path)
    args = parser.parse_args()
    report = compact(
        json.loads(args.native.read_text()), json.loads(args.wasm.read_text())
    )
    args.out.write_text(json.dumps(report, indent=2) + "\n")
    plot(report, args.image)
    if args.journal:
        database = (
            json.loads(args.journal.read_text())
            if args.journal.exists()
            else dict(schema=2, series={})
        )
        args.journal.write_text(
            json.dumps(record_gains(database, report), indent=2) + "\n"
        )


if __name__ == "__main__":
    main()
