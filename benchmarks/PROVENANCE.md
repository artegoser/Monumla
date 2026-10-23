# Source and licensing boundaries

Monumla's workspace license remains MIT. Its universal search and experiment tools are independently implemented. No Stockfish, Lc0, Lichess, Chessground or Lidraughts source module, CSS implementation, bundled executable, trained network or copied piece artwork was added to the library.

Stockfish and Lc0 are studied as external reference programs. Benchmarking invokes a separately installed executable through ordinary UCI text commands; the Rust crates neither link to nor embed those engines. Local Lc0 build sources, dependencies and downloaded networks remain outside the repository. Running benchmarks does not add them to the distributed WASM/JS output. Their own licenses still govern their distribution.

The [FSF GPL FAQ](https://www.gnu.org/licenses/gpl-faq.html#MereAggregation) distinguishes separate programs and linked combinations and discusses both communication mechanism and semantics. This project keeps the engines separate and exchanges public game/protocol information. This is an engineering boundary, not a blanket legal conclusion about every possible integration. Do not vendor GPL/AGPL implementations or networks into MIT crates.

Public algorithm descriptions and observed behavior informed the work: iterative deepening, alpha-beta/PVS, transposition bounds, paired experiments and score-to-Elo mathematics. No engine implementation was translated or copied. [README.md](README.md) links the primary methodology references.

For the requested analysis UI review, the actual Lichess files [autoShape.ts](https://github.com/lichess-org/lila/blob/master/ui/analyse/src/autoShape.ts), Chessground [state.ts](https://github.com/lichess-org/chessground/blob/master/src/state.ts) and [svg.ts](https://github.com/lichess-org/chessground/blob/master/src/svg.ts) were read to identify visible behavior: blue best/hovered arrows, grey alternatives, lower prominence for weaker lines, and no numbered candidate circles. Monumla's existing SVG overlay was modified independently. Lichess's calibrated winning-chance conversion and GPL drawing implementation were not imported. Monumla retains three arrows on request and uses fixed rank-based fading/thickness; it has no calibrated win-probability model for Lichess's score-based cutoff formula.

Lidraughts's [flat piece set](https://github.com/RoepStoep/lidraughts/tree/master/public/piece/flat) was inspected as a visual reference. The replacement Monumla discs, rings, crown paths and flat Go stones are original SVG geometry. No Lidraughts bitmap/SVG was copied. Existing chess artwork was left unchanged.

The native worker example adds `serde_json` (MIT OR Apache-2.0) only as a dev dependency. Production workspace crates retain their existing dependency boundary; neither external chess engine is a Cargo or JavaScript dependency. Review dependency/license changes whenever adding a new evaluator or renderer.
