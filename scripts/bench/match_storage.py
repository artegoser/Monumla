"""Atomic match records and strict restart validation."""

import csv
import json


def write_json(path, value):
    pending = path.with_name("." + path.name + ".next")
    pending.write_text(json.dumps(value, indent=2) + "\n")
    pending.replace(path)


def write_games(path, rows, fields):
    pending = path.with_name("." + path.name + ".next")
    with pending.open("w") as table:
        writer = csv.DictWriter(table, fields, delimiter="\t")
        writer.writeheader()
        writer.writerows(sorted(rows, key=lambda row: int(row["game"])))
    pending.replace(path)


def prepare(out, manifest, resume):
    out.mkdir(parents=True, exist_ok=True)
    destination = out / "manifest.json"
    if not resume or not destination.exists():
        if any(out.iterdir()):
            raise ValueError(
                "Output directory must be empty; use --resume for an existing match"
            )
        write_json(destination, manifest)
        (out / "moves").mkdir()
        return manifest, []
    stored = json.loads(destination.read_text())
    for key in [
        "baseline_sha256",
        "candidate_sha256",
        "baseline_revision",
        "candidate_revision",
        "baseline_settings",
        "candidate_settings",
        "ruleset",
        "games",
        "opening_plies",
        "max_plies",
        "jobs",
        "cpus",
        "platform",
    ]:
        if stored.get(key) != manifest.get(key):
            raise ValueError(f"Resume requires the same {key}")
    rows = []
    if (out / "games.tsv").exists():
        with (out / "games.tsv").open() as table:
            rows = list(csv.DictReader(table, delimiter="\t"))
    seen = set()
    for row in rows:
        game = int(row["game"])
        if (
            game in seen
            or not 1 <= game <= manifest["games"]
            or int(row["pair"]) != (game - 1) // 2
            or int(row["side"]) != 1 + (game - 1) % 2
            or row["score"] not in ["0", "0.5", "1", "NA"]
        ):
            raise ValueError("Invalid or duplicate completed game in resume table")
        seen.add(game)
        for key in ["game", "pair", "side"]:
            row[key] = int(row[key])
    if len(rows) < manifest["games"]:
        stored.setdefault("resumes", []).append(
            {
                key: manifest[key]
                for key in [
                    "started_at",
                    "runner_revision",
                    "runner_dirty",
                    "load_average_at_start",
                ]
            }
        )
        write_json(destination, stored)
    (out / "moves").mkdir(exist_ok=True)
    return stored, rows


def preserve_interrupted(path):
    if path.exists():
        index = 1
        while path.with_suffix(f".interrupted-{index}.jsonl").exists():
            index += 1
        path.rename(path.with_suffix(f".interrupted-{index}.jsonl"))
