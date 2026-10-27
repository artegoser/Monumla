# Reproducible engine experiments

Generated binaries, PGNs, traces and reports belong under ignored `target/monumla-bench/`. Keep only compact selected conclusions in [RESULTS.md](RESULTS.md) and small regression positions in `positions/`. Do not commit dated run directories or external engine binaries/networks.

## Separate correctness, speed and strength

1. Run rules/search regression tests first. A faster wrong answer is a regression.
2. For nonfunctional optimizations, compare identical fixed-depth/node searches. Require the same score, PV, node count and stop reason. Warm up, alternate baseline/candidate order, repeat on several positions, report median and spread. Fix compiler, flags, CPU, threads, cache sizes and power settings; keep the machine idle. An NPS increase alone does not establish an Elo gain.
3. For behavior changes, save two native binaries and compare paired games with the same opening and swapped sides. Report every W/D/L, termination, excluded game and pentanomial pair count. Predeclare game count; do not repeatedly inspect a fixed-N interval and stop when it turns favorable. A real SPRT needs a separately specified sequential test; this runner does not claim to implement Fishtest's GSPRT.
4. Promote a candidate only after it passes tactical regressions and enough paired comparisons. Run every affected ruleset: universal code does not guarantee equal strength gains in every game.

## Profile the current bottleneck

```sh
cargo build --release -p monumla-wasm --examples
target/release/examples/profile_search benchmarks/positions/chess/italian-queen-g4.pgn 2 100000
```

The profiler measures shared search hooks (generation, evaluation, outcomes, applying moves, hashing and quiescence). Instrumentation itself has overhead. For a system profile on Linux, build with symbols (`CARGO_PROFILE_RELEASE_DEBUG=1`) and use `perf record -g -- target/release/examples/profile_search …`; use `perf report` to attribute the cost before changing code. If hardware counters are restricted, report that rather than inventing a profile. Keep traces local.

## Compare native Monumla versions in any supported game

Build and preserve the baseline **before editing**; do the same for the candidate:

```sh
cargo build --release -p monumla-wasm --example engine_worker
mkdir -p target/monumla-bench/bin
cp target/release/examples/engine_worker target/monumla-bench/bin/baseline
# Build the candidate after the change, then:
cp target/release/examples/engine_worker target/monumla-bench/bin/candidate
python3 scripts/bench/strength.py match \
  --baseline target/monumla-bench/bin/baseline \
  --candidate target/monumla-bench/bin/candidate \
  --ruleset checkers --games 40 --time-ms 1000 \
  --out target/monumla-bench/comparison
```

Rulesets: `chess`, `checkers` (Russian), `checkers-international`, `go` (9×9 area scoring). Both hosts use the **existing browser game construction and the single RuntimeDomain**. Games contain no experiment or search adapters. Complete core turn inputs cross the JSON-lines boundary; both hosts replay every action and must agree on the full position/history key. Incompatible rules or openings abort the experiment. Binary hashes and settings are recorded. Prefixes are generated deterministically by ordinary rules; curate a broader representative opening suite before large strength tests. `--time-ms 0 --depth 2 --nodes 10000` gives deterministic fixed-work comparisons. Native JSON support is an example/dev dependency, not part of the WASM library's dependency graph.

The runner reports candidate-relative results, not a transferable absolute rating. A same-binary smoke test verifies the protocol and rules replay, not engine quality. Games unfinished at the ply cap are **excluded**, never adjudicated draws. Too many unfinished games invalidates useful strength conclusions.

## Stockfish reference pool and clocks

```sh
target/release/examples/stockfish_match \
  --stockfish /path/to/independently-installed/stockfish \
  --elo 1320 --clock-ms 120000 --increment-ms 1000 \
  --games 12 --max-plies 500 \
  --out target/monumla-bench/reference-1320
python3 scripts/bench/strength.py report \
  target/monumla-bench/reference-1320/games.tsv --reference 1320
```

UCI options and requested Elo are validated against the executable's advertised bounds. One thread, 16 MB hash, no pondering, ordinary rules validate all moves. Clock mode sends `wtime/btime/winc/binc` to the opponent and charges whole wall time, including Monumla generation/notation/application. Monumla budgets approximately remaining time / 30 plus ¾ increment, with a reserve; running out of time is a loss. Fixed `--movetime-ms` is useful for diagnostics but does not reproduce the calibrated reference time control. Record compiler/flags, binary SHA-256, hardware and source cleanliness alongside the run. Do not build or run other benchmarks during timed matches.

Stockfish documents UCI_Elo as calibrated at **120 seconds + 1 second increment**, anchored to CCRL 40/4. Its setting is not a direct FIDE or Lichess rating, and changing control, hardware, openings or the engine version changes the interpretation. If the current engine loses every game against the weakest calibrated setting, there is **no finite maximum-likelihood Elo estimate**. Report a bound and use a weaker independently calibrated pool before publishing a point estimate. Do not turn a node limit or skill level into an invented Elo.

`strength.py` converts mean expected score with `400 log10(p/(1-p))`. It uses complete swapped-color pairs as observations and a conservative 95% fixed-N KL/Chernoff bound for bounded pair scores; games inside a pair need not be independent. Independent representative pairs are an assumption, not a property proved by the script. The interval is deliberately wide for small samples and remains honest for all wins/losses. Unfinished pairs are excluded explicitly; their missing outcomes can bias results. This is not Fishtest's normalized-Elo/pentanomial GSPRT estimator, and it does not combine opponents into a calibrated pool rating.

```sh
python3 -m unittest discover -s scripts/bench -p 'test_*.py'
```

## What transfers from Stockfish and Lc0

Stockfish combines a specialized learned NNUE evaluation, efficient incremental state/evaluation work, move ordering, selective alpha-beta search and extensive empirical testing. Its development process distinguishes changes that preserve the search from changes that alter it; functional patches are tested in Fishtest. Monumla should adopt the discipline: profile shared hooks, reduce cloning/hash work, keep iteration results, test tactical failures, compare saved versions with paired games, and accept strength claims only with uncertainty.

Lc0 uses policy/value prediction and PUCT search, with neural inference batching and hardware backends. Those ideas can support many games, but an effective generic policy/value model requires a suitable universal state/action representation and training data. Copying a chess network/encoder would not produce a universal evaluator. Monumla's current rule-probed heuristic is not NNUE, a trained value model, or a calibrated win probability. A shallow generic score can still be tactically wrong; it must never be described as solved/perfect play.

Do not transplant chess-only null-move assumptions, piece tables, check/evasion shortcuts, encoders or tuned constants into the shared engine. Games continue to provide their normal rules, legal choices and outcomes. Safe shared optimizations must preserve those rules, including forced captures, multi-step turns, Go pass/scoring and history-sensitive draws.

Primary references (read 2026-10-23):

- [Stockfish developer process](https://official-stockfish.github.io/docs/stockfish-wiki/Developers.html)
- [Stockfish UCI commands and Elo calibration](https://official-stockfish.github.io/docs/stockfish-wiki/UCI-Protocol-and-Stockfish-Commands.html)
- [Stockfish regression tests](https://official-stockfish.github.io/docs/stockfish-wiki/Regression-Tests.html)
- [Fishtest mathematics: paired outcomes, GSPRT and tuning](https://official-stockfish.github.io/docs/fishtest-wiki/Fishtest-Mathematics.html)
- [NNUE concepts](https://official-stockfish.github.io/docs/nnue-pytorch-wiki/docs/nnue.html) — concepts only; no implementation copied
- [Lc0 technical overview](https://lczero.org/dev/wiki/technical-explanation-of-leela-chess-zero/)
- [Actual Lc0 paired network comparison](https://bench.lczero.org/test/1074/)
- [Lc0 backend/network compatibility](https://lczero.org/play/networks/bestnets/)

## Complete native and WASM matrix

[The case list](cases.json) covers all five games, both checkers rulesets, several Go/tic-tac-toe sizes, and Battleship setup and firing. It is a reproducible workload suite, not an exhaustive strength test. Freeze two clean revisions, build both with the same toolchain and release flags, and keep the binaries/modules unchanged during the run.

```sh
python3 scripts/bench/matrix.py \
  --baseline target/monumla-bench/bin/baseline \
  --candidate target/monumla-bench/bin/candidate \
  --observer target/monumla-bench/bin/candidate \
  --baseline-revision BASELINE_COMMIT --candidate-revision CANDIDATE_COMMIT \
  --cpu 5 --repeats 5 --out target/monumla-bench/native-matrix
# Build both revisions with wasm-pack --target web --out-name monumla.
# The exported snapshots preserve the same logical roots across runtimes.
taskset -c 4 node scripts/bench/wasm_matrix.mjs \
  --baseline target/monumla-bench/baseline-wasm \
  --candidate target/monumla-bench/candidate-wasm \
  --matrix target/monumla-bench/native-matrix --repeats 5 \
  --out target/monumla-bench/wasm-matrix
# Requires Matplotlib in the chosen Python environment.
python3 scripts/bench/matrix_report.py \
  --native target/monumla-bench/native-matrix/matrix.json \
  --wasm target/monumla-bench/wasm-matrix/matrix.json
```

`latest-matrix.json` and `performance.png` show **every case**, including regressions, failures and overlapping timing ranges. Same-source revision labels are enforced across runtimes. SHA-256 values identify the distinct native and WASM binaries; they are not expected to match each other. Native timings include the JSON worker boundary; WASM timings include callbacks but exclude snapshot loading. Compare changes within each runtime, not absolute timings across them. Warmup precedes five alternating measured repetitions. Pinning reduces scheduling noise but does not isolate shared caches, CPU boost or thermal effects.

`performance_history.py` records only improvements with identical work/results and non-overlapping observed ranges on every selected fixture. Use separate series for different workloads/runtimes and compare against that series' last accepted binary. `performance.json` is the accepted history; `performance-history.png` plots it. Historical milestones naturally have different commit labels. Missing gains remain visible in the complete matrix instead of being silently promoted into the history. Observed min/max ranges are not statistical confidence intervals.

## Live search and profiling

The browser's preferred path is one continuous WASM call in a worker. The main thread cancels it through a shared atomic flag. `pnpm start` uses the custom adapter-node server so both documents **and worker assets** receive COOP/COEP headers. If deploying behind another server, preserve those headers. Hosts without cross-origin isolation fall back to yielding chunks; root frontiers, transposition data and unfinished hidden-world batches survive those yields. Only completed root rankings drive bot decisions; unfinished bounds appear separately in analysis. Hidden-world estimates stream after complete comparable samples.

Use `wasm-pack build crates/monumla-wasm --profiling --target web --out-name monumla --out-dir /absolute/local/profile-wasm` to retain names for V8 CPU profiles. `wasm_throughput.mjs` supports `--snapshot`, `--continuous true`, and `--module`; run it under `node --cpu-prof --cpu-prof-dir=target/monumla-bench` for a local sample profile. Do not mix profiling runs with clean timing measurements. Linux perf sampling may be denied by host policy even for user-space events; shared-hook native instrumentation remains available without changing global kernel settings.

## Internal Elo pools and asymmetric budgets

```sh
python3 scripts/bench/strength.py match \
  --baseline target/monumla-bench/bin/baseline \
  --candidate target/monumla-bench/bin/candidate \
  --ruleset chess --games 40 --jobs 4 --cpus 0,1,2,3 \
  --baseline-time-ms 1000 --candidate-time-ms 10000 \
  --out target/monumla-bench/chess-budget-match
python3 scripts/bench/league.py target/monumla-bench/chess-budget-match \
  --out target/monumla-bench/league.json
```

The same runner supports `battleship`, `tictactoe`, `tictactoe:5x5:4`, `tictactoe:7x5:5`, and `go:SIZE`, in addition to chess/checkers. Entrants include both binary identity and search budget, so a single binary at 1 and 10 seconds is two entrants. Every ruleset/size has a separate pool. Online Elo starts at an explicit convention (1000, K=20 per game); it is a useful internal tracking system, not an externally calibrated rating. Import complete paired matches in chronological order. Unfinished pairs are excluded and duplicate imports rejected. Pair-based uncertainty in `strength.py` remains the evidence for strength comparisons; the online rating does not replace it.

## Strength statistics use the same evidence rules

```sh
python3 scripts/bench/strength_history.py \
  target/monumla-bench/chess-budget-match \
  --out benchmarks/latest-strength.json --image benchmarks/strength.png \
  --journal benchmarks/strength-history.json \
  --history-image benchmarks/strength-history.png --league benchmarks/elo-pools.json
```

Pass all desired match directories in chronological order. The complete strength report keeps W/D/L, complete and excluded pairs, pentanomial counts, relative Elo estimates and conservative 95% bounds, predeclared schedule size, actual binary SHA-256, budgets, and source revisions when supplied. `strength.py match --baseline-revision ... --candidate-revision ...` labels the **frozen binaries'** verified source revisions separately from the runner's revision; unknown source provenance stays unknown. New matches also record their start time. The chart includes every pool declared by `cases.json`; pools without completed comparisons explicitly remain unrated. Older pilot data is identified by binary hash because the original manifests do not establish clean source provenance or timestamps. These pilots must not be presented as a same-revision all-game strength matrix.

The full report/graph retains losses, draws, incomplete schedules and inconclusive samples. The improvement journal/graph accepts a match only after its full predeclared paired schedule finishes without excluded pairs **and** its conservative lower expected-score bound exceeds 0.5. Reimporting the same supported result cannot add another milestone. No finite estimate is fabricated for all wins/losses, and no zero is inserted for an unrated game. Do not aggregate different games, budgets, board sizes or opponents into one engine Elo. Internal online pool ratings remain separate from match-based evidence. For a revision-to-revision matrix, freeze the same two revisions, hold budgets/opening protocol fixed and run every pool; do not substitute the historical asymmetric-budget pilots for that experiment.

These are fixed-N per-comparison intervals, not an SPRT or a family-wise guarantee across repeated experiments. Predeclare the sample and acceptance criteria before a match; do not stop when the results look favorable. Elo uncertainty and incomplete coverage are evidence limitations, not quantities that can be repaired by plotting a smoother line.

Within a strength-history series, keep the pool, budgets, opening protocol, move cap and platform fixed. A later milestone must beat the last accepted entrant, identified by binary **and** settings; reusing an older baseline is rejected. An asymmetric-budget comparison is evidence for those two entrants, not a cumulative release-strength curve. Changing the protocol starts a separate series.

## Run and resume a complete strength matrix

`strength-cases.json` predeclares one schedule per pool, with the same frozen engine pair throughout. The default plan uses 500 ms per move and 12 games per pool, except the explicitly smaller Go 19×19 pilot (two games). Move caps are separate per game; reaching one excludes the game rather than adjudicating a draw. This is a short-control experiment, not a calibrated absolute rating.

```sh
python3 scripts/bench/strength_matrix.py \
  --baseline target/monumla-bench/bin/baseline \
  --candidate target/monumla-bench/bin/candidate \
  --baseline-revision BASELINE_COMMIT --candidate-revision CANDIDATE_COMMIT \
  --cpus 0,1,2,3 --out target/monumla-bench/strength-matrix
# After an interruption, use the identical arguments plus --resume.
python3 scripts/bench/strength_history.py \
  --matrix target/monumla-bench/strength-matrix
```

The matrix shares CPU slots across all pairs, so at most one pair owns each selected CPU. Atomic result-table updates preserve completed games across interruption. Resume verifies binary hashes, search budgets, opening protocol, pool plan, CPU settings and move caps; it runs only missing game numbers, including a missing counterpart of an already completed game. It archives interrupted move traces and records the restart. Completed results, including exclusions and losses, are never rerun to improve their score. Do not build over the frozen executable paths during an experiment.

Publishing a matrix verifies coverage and the common source/binary pair against each pool's own manifest. The complete report includes termination counts and explicitly labels search forfeits; a budget failure is not silently described as a checkmate or a tactical strength gain. Failed pools remain visible and do not update the league. All pools must finish processing before publication.

The online league export now continues its saved ratings instead of resetting them on each report. Imported matches are fingerprinted; regenerating a report cannot count them twice. Once fingerprinted, the saved league can continue even after older raw logs are removed. Predeclared schedules must finish before import. The standalone `league.py --previous OLD_JSON --out NEW_JSON ...` also supports continuation. Disconnected opponent networks are labeled by `rating_component`; their independently initialized ratings must not be compared. All these ratings remain provisional internal measurements.
