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

## Budget comparison and internal Elo pilot (2026-10-25)

The same frozen Monumla binary (`fb1bec40c1db9fd76943063e15b3dbfa8e7765139f77ba2be85358c3d9932f60`, from the `49920fb` search implementation) played eight predeclared chess games at 1 versus 10 seconds per move, in four swapped-color pairs, seeds 0–3 and two ordinary opening plies. Four games ran concurrently on separate physical cores. The runner manifest records a dirty tool workspace; this was a pilot using the preserved binary, not a clean-source release strength certification.

The 10-second entrant won **8–0–0**, all by checkmate, with no excluded pairs. This is encouraging evidence that the extra work can help, but four opening pairs are a small sample. The conservative paired 95% expected-score interval is **[0.3976, 1]**, giving a relative Elo bound **[−72.1, +∞]**, with no finite maximum-likelihood point estimate. Do not publish a finite calibrated Elo from this result. The online league's 1067.5 versus 932.5 ratings merely reflect the explicitly chosen initial rating 1000 and K=20; they are internal provisional bookkeeping.

Separate 50 versus 250 ms protocol pilots used one swapped-color pair per game. Results below are for the 250 ms entrant. These tiny samples do not establish a strength ordering:

| Pool | W–D–L | Complete / excluded pairs |
|---|---|---|
| Russian checkers | 1–0–1 | 1 / 0 |
| International checkers | 1–1–0 | 1 / 0 |
| Battleship | 0–0–2 | 1 / 0 |
| Tic-tac-toe | 1–0–1 | 1 / 0 |
| Go 5×5 | 0–0–0 | 0 / 1 (both reached the move cap) |

More time is not a guarantee of stronger play in the current evaluator. Keep the Battleship losses and unfinished Go games in the record. Each variant/board size needs its own rating pool and representative paired tests.

## Reported chess failures

The supplied repetition sequence reaches threefold/fourfold repetition, not fivefold automatic termination. Threefold and 50-move claims are now ordinary legal actions, including claims by an intended move, so the universal search can consider them through the normal rules interface.

Before `50...Rxe3+??`, the supplied position is `8/4rk2/5p2/2pP1r1p/1pP2P2/4BK1P/3R4/8 b - - 4 50`. Cold depth sweeps 3–7 all selected `Rc7`, but the forced `Rxe3+` probe remained close to equal in Monumla's generic score. An independently run Stockfish 19 five-second search instead chose `h4` near equality and evaluated forced `Rxe3+` at approximately +6.52 pawns for White. **The evaluation weakness remains.** Avoiding the move in a particular run is not evidence that the engine understands its consequences. A separate generic regression verifies that a visible forced win takes precedence over a free material capture.

## Sampling and profiling

In the corrected public Battleship snapshot with damaged hull cells at F3/F5/H5, 512 legal sampled fleet completions occupied F4 in 258 cases, F2 in 179, I5 in 184 and H6 in 172. These are sampler diagnostics, not exact posterior probabilities or proof of optimal firing choices. Only the active player's redacted observation enters the sampler; enemy ship geometry is not supplied. No screenshot-specific tactical bonus was added.

Native shared-hook instrumentation and named V8/WASM CPU sampling identified legal-choice cloning and eager speculative-state hashing as substantial costs. The shared interaction driver now shares its immutable choice list; outcome generation uses cheap action grouping and compares full state only for possible aliases. Hash semantics, legal moves and evaluation were preserved. Both sides of every published speed comparison must still match depth, nodes, score, PV, lines and stop reason.

Linux `perf` and Samply sampling were attempted but denied by the host's profiling policy, including a user-space-only software event. No global kernel security setting was changed. Native hook timings and WASM CPU profiles remain available locally; profiled timings are not mixed into the fixed-work speed matrix.

## Full fixed-work matrix (2026-10-26)

[The complete matrix](latest-matrix.json) and [chart](performance.png) compare the same frozen source pair, **`44fa55c` → `07a2c49`**, across 14 cases on native release and the same 14 logical roots on WASM release (Node 24.13.1). AMD Ryzen 7 5800X, rustc 1.97.1, default workspace release settings, native CPU 5 and WASM CPU 4; one warmup followed by five alternating repetitions. No source or binary changed during the measurements. All 28 comparisons preserved completed depth, nodes, score, PV, ranked lines and stop reason within their runtime.

Native Go 19×19 speedups were 2.34× initially and 2.04× in the generated midgame; WASM was 2.15× and 1.87×. Chess ranged from 1.09–1.27× native and 1.09–1.21× WASM. The chart also retains slower/noisy results: native international checkers had a 0.85× median, WASM 3×3 tic-tac-toe had a 0.85× median, and Battleship setup stayed approximately unchanged. Short-case timing spreads overlap widely. Other user jobs were observed consuming substantial CPU during these runs; pinning does not remove cache, scheduling, thermal or frequency contention. Small ratios are provisional observations, not established effects. Raw timing samples remain local under `target/monumla-bench/verified-{native,wasm}-matrix/`.

[The performance journal](performance.json) contains only fixed-work observations with non-overlapping timing ranges; [its history graph](performance-history.png) uses the same source pair for every newly established series. The complete matrix retains every excluded/noisy/regressing case. These are throughput measurements, not evidence that playing strength improved.

[Strength statistics](latest-strength.json) and [their graph](strength.png) currently show the historical pilots above, with explicit unrated rows for missing pools. [The strength improvement journal](strength-history.json) is empty, and [its graph](strength-history.png) says so: none of these small pilots has a conservative lower bound above equal expected score. Online ratings in [elo-pools.json](elo-pools.json) are separately labeled internal/provisional. No current absolute Elo or all-game strength improvement is established.
