# Playing-strength benchmark

Chess strength is measured by games against Stockfish. Monumla plays through `monumla-search::runtime::RuntimeDomain`, the same shared search used in the browser and other games. The runner contains chess protocol/notation handling only; it adds no chess search or evaluation.

Use an official Stockfish binary, record its version and checksum, and run a release build:

```sh
cargo run --release -p monumla-wasm --example stockfish_match -- \
  --stockfish /path/to/stockfish \
  --games 6 --movetime-ms 1000 --max-plies 160 \
  --out target/monumla-bench/stockfish/my-run
```

Games come in pairs with the same opening and opposite Monumla colors. The small built-in suite cycles through the starting position, an Italian opening, and a Queen's Gambit opening. Stockfish uses one thread, 16 MB hash, skill 20, full strength, one PV, and no pondering. Both engines receive the same nominal search time per move. Moves from both engines are checked by ordinary chess rules, including castling, en passant, and promotions. Full move history is supplied to both engines.

The runner writes `report.md`, `moves.tsv` (depth, nodes, actual elapsed time, stop reason), and one PGN per game. It retains the runtime domain and search memory across each game, matching normal bot play. If the first iteration cannot finish, play may retain a legal unscored decision, logged as `legal_fallback`; it is never a completed evaluation. Checkmates and automatic draws are recognized by the rules. Games reaching the ply cap remain unfinished and are not counted as draws. Protocol errors fail the run. No evaluation-based win/draw adjudication is used. Output directories must be empty.

## Calibrated reference and Elo graph

```sh
python3 scripts/bench/reference_match.py \
  --runner /path/to/frozen/stockfish_match \
  --stockfish /path/to/frozen/stockfish \
  --games 12 --elo 1320 --clock-ms 120000 --increment-ms 1000 \
  --max-plies 500 --cpus 0,1,2,3 \
  --out target/monumla-bench/reference-1320
python3 scripts/bench/reference_report.py \
  --run target/monumla-bench/reference-1320
```

The coordinator predeclares the full paired schedule, records binary identities, charges whole move time and saves each completed game atomically. `--resume` preserves completed games and archives interrupted traces; binaries and controls must match. The underlying runner accepts `--games 1 --game-offset N` for independently scheduled games, while the coordinator requires a complete even schedule. Each opening is played with both colors.

Only a finished schedule at the documented 120s+1s calibration control can publish `benchmarks/elo.json` and `benchmarks/elo.png`. The graph shows Monumla's estimate and a conservative paired confidence interval on the **Stockfish UCI_Elo reference scale**. All losses/wins yield a bound with an arrow, without a fabricated finite rating. This calibration is distinct from FIDE/Lichess ratings and from version-to-version Elo changes. External Stockfish binaries remain under ignored `target/`; no Stockfish code or weights are linked into Monumla.

To find a comparable opponent automatically, import a first probe and optionally an earlier completed pilot:

```sh
python3 scripts/bench/reference_ladder.py \
  --pilot target/monumla-bench/reference-1320 \
  --seed target/monumla-bench/reference-1600 \
  --out target/monumla-bench/chess-calibration
# After an interruption:
python3 scripts/bench/reference_ladder.py \
  --out target/monumla-bench/chess-calibration --resume
python3 scripts/bench/reference_ladder_report.py \
  --run target/monumla-bench/chess-calibration
```

Discovery completes each full batch before changing levels. It steps by 200 Elo until bracketed, then tests the midpoint rounded to 25 Elo (ties to even). A combined score of 40–60% at a setting selects that opponent for a **fresh, fixed 48-game confirmation**. Repeated discovery batches at one setting are combined; discovery stops with an explicit error after eight probe batches if no equilibrium is found. Every new batch is recorded before play, and resume retains its original games, colors, binaries and controls. The driver rejects incomplete/excluded pairs instead of silently dropping them from calibration.

The discovery curve is `benchmarks/elo-calibration.png`. Its points measure the same frozen Monumla against different settings; they are not strength improvements over time. Only the independent confirmation determines the final Elo estimate and interval in `elo.json`/`elo.png`. While confirmation is incomplete, plotting discovery leaves the previously published rating intact. Choosing an opponent adaptively does not justify pooling those discovery games into a fixed-sample confidence interval.

If the larger confirmation shows the opponent was uneven, further predeclared level checks can refine the equilibrium. Retain the original confirmation as the primary result regardless of those later scores. The run plan may include separate `validation` batches; resume completes them without replaying confirmation. Their selection policy is retained in the published calibration metadata, and they never enter the primary rating interval.

Audit completed PGNs against a frozen WASM build of the same rules before accepting a result:

```sh
node scripts/bench/reference_audit.mjs /path/to/frozen/wasm \
  target/monumla-bench/reference-1600
```

The audit checks actual clock/level settings, replays each PGN, verifies its rules outcome against the result table, and inspects every position for claimable draws. Draw claims need explicit review because UCI has no claim-draw command. The audit records its WASM checksum and findings under the ignored run directory.

The deadline is checked between nodes. Rule generation, move validation, and notation can exceed the nominal time; inspect actual times in the log. This is a development match harness rather than a tournament clock manager. A small result set is a baseline, not an Elo estimate. Strength improvements need larger matched runs under the same settings and machine conditions; avoid concurrent browser/search work during measurement.

Search changes belong in `monumla-search`. Keep the abstract minimax, draughts, Go, and browser regressions passing. Chess gains do not by themselves establish Go or draughts strength; their branching factors and strategic demands differ.

Protocol and release sources: [Stockfish UCI documentation](https://official-stockfish.github.io/docs/stockfish-wiki/UCI-Protocol-and-Stockfish-Commands.html), [official releases](https://github.com/official-stockfish/Stockfish/releases).

## Recorded results

[The compact results summary](../RESULTS.md) records previous observations. Generated reports, PGNs, profiles, models, and per-run logs live under ignored `target/monumla-bench/`; they are not committed. Keep only small, intentionally selected regression positions in `benchmarks/positions/`.

## Profiling a search deadline

```sh
cargo run --release -p monumla-wasm --example profile_search -- \
  benchmarks/positions/chess/central-knights.pgn 1 10000
cargo run --release -p monumla-wasm --example profile_search -- - 2 10000
```

The profiler reports calls and time by shared search hook, on the final board in a PGN (ignoring a recorded result); `-` selects the starting position. It runs a fixed-depth/node search without a wall-clock deadline, so the cost of completing an iteration can be measured. It delegates every search operation to the normal runtime domain and does not substitute an evaluator or generator.
