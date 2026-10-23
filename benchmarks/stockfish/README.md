# Playing-strength benchmark

Chess strength is measured by games against Stockfish. Monumla plays through `monumla-search::runtime::RuntimeDomain`, the same shared search used in the browser and other games. The runner contains chess protocol/notation handling only; it adds no chess search or evaluation.

Use an official Stockfish binary, record its version and checksum, and run a release build:

```sh
cargo run --release -p monumla-wasm --example stockfish_match -- \
  --stockfish /path/to/stockfish \
  --games 6 --movetime-ms 1000 --max-plies 160 \
  --out benchmarks/stockfish/my-run
```

Games come in pairs with the same opening and opposite Monumla colors. The small built-in suite cycles through the starting position, an Italian opening, and a Queen's Gambit opening. Stockfish uses one thread, 16 MB hash, skill 20, full strength, one PV, and no pondering. Both engines receive the same nominal search time per move. Moves from both engines are checked by ordinary chess rules, including castling, en passant, and promotions. Full move history is supplied to both engines.

The runner writes `report.md`, `moves.tsv` (depth, nodes, actual elapsed time, stop reason), and one PGN per game. Checkmates and automatic draws are recognized by the rules. A search that cannot produce a completed iteration loses by forfeit; there is no arbitrary-move fallback. Games reaching the ply cap remain unfinished and are not counted as draws. Protocol errors fail the run. No evaluation-based win/draw adjudication is used.

The deadline is checked between nodes. Rule generation, move validation, and notation can exceed the nominal time; inspect actual times in the log. This is a development match harness rather than a tournament clock manager. A small result set is a baseline, not an Elo estimate. Strength improvements need larger matched runs under the same settings and machine conditions; avoid concurrent browser/search work during measurement.

Search changes belong in `monumla-search`. Keep the abstract minimax, draughts, Go, and browser regressions passing. Chess gains do not by themselves establish Go or draughts strength; their branching factors and strategic demands differ.

Protocol and release sources: [Stockfish UCI documentation](https://official-stockfish.github.io/docs/stockfish-wiki/UCI-Protocol-and-Stockfish-Commands.html), [official releases](https://github.com/official-stockfish/Stockfish/releases).

## Recorded runs

Both runs use the same machine, Stockfish binary, starting-position color pair, and 1000 ms nominal budget.

| Search revision | Result for Monumla | Termination |
|---|---|---|
| [Initial baseline](baseline-2026-10-23/report.md), `9db54e3` | 0–2 | Two checkmates, 38/21 plies |
| [After probe fix](after-probe-fix-2026-10-23/report.md), `89e1bf6` | 0–2 | Two forfeits: no complete iteration, 18/13 plies |

The match exposed capability probes becoming terminal after entities were removed, which undervalued some movement capabilities. The shared fix now passes knight-for-pawn and draughts-promotion regressions. It does **not** establish a strength gain: the updated run still loses and exposes the cost of generation/evaluation before the first completed iteration. The browser retries with longer budgets; this benchmark enforces its stated budget and records failure. The engine is not competitive with Stockfish at this setting.

## Profiling a search deadline

```sh
cargo run --release -p monumla-wasm --example profile_search -- \
  benchmarks/stockfish/after-probe-fix-2026-10-23/game-002.pgn 1 10000
cargo run --release -p monumla-wasm --example profile_search -- - 2 10000
```

The profiler reports calls and time by shared search hook, on the final board in a PGN (ignoring a recorded result); `-` selects the starting position. It runs a fixed-depth/node search without a wall-clock deadline, so the cost of completing an iteration can be measured. It delegates every search operation to the normal runtime domain and does not substitute an evaluator or generator.
