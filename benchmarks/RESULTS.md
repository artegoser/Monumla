# Engine experiment summary

Raw artifacts are generated under ignored `target/monumla-bench/`. This file is a compact record of selected observations, not an append-only dump of runs. None of these match pairs establishes an Elo rating.

## Initial Stockfish reference (2026-10-23)

AMD Ryzen 7 5800X; release build with rustc 1.97.1. Stockfish 19, one thread, Hash 16 MB, skill 20, unrestricted strength; nominal 1000 ms/move for both engines. One starting-position pair with swapped colors per revision.

| Search revision | Monumla W–D–L | Termination |
|---|---|---|
| `9db54e3` | 0–0–2 | Checkmates at 38/21 plies |
| `89e1bf6` | 0–0–2 | No completed iteration within budget, 18/13 plies |
| `9792e70` | 0–0–2 | Checkmates at 38/31 plies; no forfeits |

Stockfish's binary SHA-256 was `0f83d24cc46d2c66c60f16001af5444873bc112b7d028594513426894c12da19`, from its official `sf_19` Linux universal release. Previous raw runs were preserved locally under `target/monumla-bench/archive/`.

The generic capability-probe fix prevents isolated terminal positions from undervaluing movement. Reading cached effects without cloning states and using lazy quiet witnesses reduced a fixed search on `positions/chess/central-knights.pgn` from 992.94 to 607.73 ms (128 nodes, same score −157). These were single diagnostic runs. The browser midgame test subsequently took about 8 seconds versus about 32 seconds previously. Timing is hardware/workload dependent; no strength gain follows from these measurements alone.

The next strength baseline must use a defined rating pool/time control and uncertainty, with matched Monumla version comparisons for candidate changes. Two losses to an unrestricted top engine cannot determine Monumla's rating.
