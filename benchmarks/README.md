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
