"""Verify pool coverage and frozen provenance before publishing strength data."""

import csv
import json
from pathlib import Path
from league import canonical_ruleset


def verify_matrix(root, coverage):
    root = Path(root)
    matrix = json.loads((root / "matrix.json").read_text())
    plan = {case["id"]: case for case in matrix["plan"]}
    rows = {row["case"]["id"]: row for row in matrix["results"]}
    if (
        len(plan) != len(matrix["plan"])
        or len(rows) != len(matrix["results"])
        or set(rows) != set(plan)
    ):
        raise ValueError(
            "Finish every declared pool before publishing the strength matrix"
        )
    if {canonical_ruleset(case["ruleset"]) for case in plan.values()} != {
        canonical_ruleset(case["ruleset"]) for case in coverage
    }:
        raise ValueError("Strength matrix coverage differs from the declared games")
    directories, completed, failures = [], [], {}
    for name, case in plan.items():
        row = rows[name]
        if row["case"] != case:
            raise ValueError("A pool's recorded plan changed")
        folder = root / name
        if row["returncode"]:
            failures[canonical_ruleset(case["ruleset"])] = row.get(
                "error", "match failed"
            )
        if not (folder / "manifest.json").exists():
            if row["returncode"]:
                continue
            raise ValueError("Completed pool has no provenance manifest")
        manifest = json.loads((folder / "manifest.json").read_text())
        for side in ["baseline", "candidate"]:
            if (
                manifest[f"{side}_sha256"] != matrix[f"{side}_sha256"]
                or manifest.get(f"{side}_revision")
                != matrix["settings"][f"{side}_revision"]
            ):
                raise ValueError(
                    "Every pool must compare the same frozen binaries and revisions"
                )
            if manifest[f"{side}_settings"] != dict(
                depth=32, nodes=50000000, time_ms=case["time_ms"]
            ):
                raise ValueError("Pool search budgets changed")
        for key in ["ruleset", "games", "max_plies"]:
            if manifest[key] != case[key]:
                raise ValueError(f"Pool {key} changed")
        if manifest["opening_plies"] != matrix["settings"]["opening_plies"]:
            raise ValueError("Pool opening protocol changed")
        if (folder / "games.tsv").exists():
            directories.append(folder)
            if not row["returncode"]:
                with (folder / "games.tsv").open() as table:
                    if (
                        len(list(csv.DictReader(table, delimiter="\t")))
                        != case["games"]
                    ):
                        raise ValueError("A successful pool has an incomplete schedule")
                completed.append(folder)
        elif not row["returncode"]:
            raise ValueError("Completed pool has no games")
    metadata = dict(
        revisions={
            side: matrix["settings"][f"{side}_revision"]
            for side in ["baseline", "candidate"]
        },
        binary_sha256={
            side: matrix[f"{side}_sha256"] for side in ["baseline", "candidate"]
        },
        started_at=matrix["started_at"],
        resumes=matrix.get("resumes", []),
        plan=matrix["plan"],
        failures=failures,
    )
    return metadata, directories, completed
