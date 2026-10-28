# Engine experiment summary

Raw artifacts are generated under ignored `target/monumla-bench/`. This file is a compact record of selected observations, not an append-only dump of runs. Historical tiny matches are not calibrated ratings; the completed reference calibration is recorded below.

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

The earlier complete comparison used the same frozen source pair, **`44fa55c` → `07a2c49`**, across 14 cases on native release and the same 14 logical roots on WASM release (Node 24.13.1). AMD Ryzen 7 5800X, rustc 1.97.1, default workspace release settings, native CPU 5 and WASM CPU 4; one warmup followed by five alternating repetitions. No source or binary changed during the measurements. All 28 comparisons preserved completed depth, nodes, score, PV, ranked lines and stop reason within their runtime. The current matrix and chart are updated by the later measurements below.

Native Go 19×19 speedups were 2.34× initially and 2.04× in the generated midgame; WASM was 2.15× and 1.87×. Chess ranged from 1.09–1.27× native and 1.09–1.21× WASM. The chart also retains slower/noisy results: native international checkers had a 0.85× median, WASM 3×3 tic-tac-toe had a 0.85× median, and Battleship setup stayed approximately unchanged. Short-case timing spreads overlap widely. Other user jobs were observed consuming substantial CPU during these runs; pinning does not remove cache, scheduling, thermal or frequency contention. Small ratios are provisional observations, not established effects. Raw timing samples remain local under `target/monumla-bench/verified-{native,wasm}-matrix/`.

[The performance journal](performance.json) contains only fixed-work observations with non-overlapping timing ranges; [its history graph](performance-history.png) retains each suite's accepted chain. The complete matrix retains every excluded/noisy/regressing case. These are throughput measurements, not evidence that playing strength improved.

[Strength statistics](latest-strength.json) and [their graph](strength.png) now show the complete matched tournament below. [The strength improvement journal](strength-history.json) remains empty: none of the comparisons establishes a positive lower confidence bound. Online ratings in [elo-pools.json](elo-pools.json) retain the historical pilots and the new results, with disconnected rating components labeled explicitly. No calibrated absolute Elo or all-game strength improvement is established.


## Matched all-pool strength run (2026-10-27)

The frozen **`44fa55c` → `07a2c49`** pair from the performance matrix played all **98 predeclared games**, 500 ms/move for both entrants. Eight pools had six swapped-side pairs each; Go 19×19 had one explicitly declared pilot pair. Four shared physical-core slots bounded concurrency. The run was interrupted and resumed with the same binaries, seeds, settings and game numbers; completed scores were preserved and interrupted traces archived. Restart metadata remains in the report. A dirty runner workspace at restart contained a separate, unbuilt optimization prototype; the exact experiment binaries remained unchanged.

| Pool | Candidate paired W–D–L | Complete / excluded pairs | Relative Elo estimate | 95% bound | Search forfeits, all games |
|---|---|---|---|---|---|
| Chess | 6–2–4 | 6 / 0 | +58.5 | −347.6 to +519.0 | 1 |
| Russian checkers | 3–5–4 | 6 / 0 | −29.0 | −469.9 to +385.2 | 0 |
| International checkers | 3–3–6 | 6 / 0 | −88.7 | −574.6 to +312.1 | 0 |
| Go 9×9 | 3–0–1 | 2 / 4 | +190.8 | −547.8 to +1672.4 | 8 |
| Go 19×19 | 1–0–1 | 1 / 0 | 0.0 | −1522.4 to +1522.4 | 2 |
| Battleship | 4–0–8 | 6 / 0 | −120.4 | −639.1 to +278.2 | 0 |
| Tic-tac-toe 3×3 | 5–2–5 | 6 / 0 | 0.0 | −425.7 to +425.7 | 0 |
| Tic-tac-toe 5×5, connect 4 | 3–7–2 | 6 / 0 | +29.0 | −385.2 to +469.9 | 0 |
| Tic-tac-toe 7×5, connect 5 | 0–12–0 | 6 / 0 | 0.0 | −425.7 to +425.7 | 0 |

The Go 9×9 raw scored-game W–D–L was 7–0–1, with four additional unfinished games. Four finished wins had unfinished counterparts and therefore did **not** contribute to the paired Elo estimate. The report now gives both raw and paired W–D–L. Go 19×19 split two search forfeits; this is an operational failure result, not evidence of normal Go playing strength. In total, the run contains 11 search forfeits and four move-cap exclusions. None of the intervals establishes an improvement. The negative Battleship/checkers estimates are retained rather than discarded.

The native harness requires a completed search iteration before choosing a move. At this short control it sometimes fails to return one. Those losses identify an actual search-budget limitation; they must not be relabeled checkmates, hidden as draws, or used to claim a tactical improvement. Two ordinary opening plies can already create a forced win in tic-tac-toe, so wins from such paired starts do not imply that perfect play from the empty board should be decisive.

## Rejected validation-cache experiment (2026-10-27)

Candidate `92e4586` cached successful structural validation on immutable state snapshots, invalidating it on every mutable projection. It passed 215 release tests and Clippy and preserved depth, nodes, score, PV, ranked lines and stop reason in **all 28 native/WASM comparisons**. The same 14-case suite used one warmup and five alternating measurements against frozen `07a2c49`.

It did not improve performance: the geometric mean of case speed ratios was **0.9843× native** and **0.9966× WASM**. Five native cases and three WASM cases had slower timing ranges with no overlap; WASM Battleship setup fell to **0.9323×**. These are observed ranges, not confidence intervals. The candidate was rejected and kept off `main` on `perf/validation-cache-rejected`; no gain was added to the performance journal. The full per-case measurements remain under ignored `target/monumla-bench/validation-cache-{native,wasm}-matrix/`. Passing correctness tests alone was insufficient to ship this optimization.

## Stop at proven outcomes (2026-10-27)

`16cb748` propagates terminal-outcome proofs through the shared alpha-beta traversal, bound-aware tables, mandatory quiescence responses and resumed MultiPV passes. A terminal principal variation alone does not prove the root: visited heuristic siblings prevent the solved flag. Once every requested variation is proven, iterative deepening stops and subsequent calls reuse the solved result. Hidden-world sampling remains an estimate, never a proof.

The former 3×3 display of **depth 32** was a nominal iteration counter. It did not represent a legal 32-ply line. Reductions can also make the final nominal iteration exceed physical game length: the current reduced search solves the initial board at iteration 11 while reaching only **9 plies**. The finished UI therefore reports **Solved · reached 9 plies**. The empty board is a proven draw, including all three requested variations; optimal self-play also ends in a draw. No game-specific table or depth cap was added.

Cold empty 3×3, one PV, nominal limit 32, 50 million node limit, no deadline; frozen `2811f1c` versus `16cb748`, one warmup and five alternating repetitions on the same pinned CPU within each runtime:

| Runtime | Before median (observed range) | After median (observed range) | Nodes before → after |
|---|---|---|---|
| Native release, CPU 5 | 960.6 ms (928.1–1069.1) | 173.3 ms (167.6–179.7) | 66,247 → 11,604 |
| WASM release, Node 24.13.1, CPU 4 | 1128.3 ms (1118.8–1144.0) | 223.0 ms (222.2–233.4) | 64,885 → 11,360 |

This is avoided search work, **not a fixed-work NPS gain or an Elo improvement**, and the remaining runtime is still substantial for this small game. Raw reports, binary hashes and all samples remain under `target/monumla-bench/proof-solve-{native,wasm}-final.json`. The separate shallow 14-case suite preserved work and answers on native and WASM; its raw runs are `proof-{native,wasm}-matrix/`. All 100 desktop/mobile browser tests passed, including continuous and fallback workers stopping at the proof. Native hook profiling attributes most remaining 3×3 runtime to legal-turn generation and repeated full-history identity computation; that is the next measured optimization target.

## Cache timeline identity (`d0ab8ac`)

The shared timeline now memoizes its existing digest and invalidates it on successful commit, undo and redo. History remains part of the key; digest contents are unchanged. Cache state is excluded from serialization and logical equality. This avoids hashing the same immutable history repeatedly during generation, outcome checks and move application. It changes no game rules, evaluation, pruning or legal actions.

[The current complete matrix](latest-matrix.json) and [chart](performance.png) compare **`07a2c49` → `d0ab8ac`** on all 14 native and 14 WASM cases, with the same controls and five alternating repetitions described above. The rebuilt historical native baseline exactly matched its previously recorded SHA-256, preserving the performance journal's chain. All 28 searches preserved depth, nodes, score, PV, lines and stop reason. Native chess speedups were **1.13–1.20×**, WASM **1.13–1.19×**; the geometric means across all cases were 1.065× and 1.055×. Smaller/noisy effects remain visible, including native initial Go 19×19 at 0.983× and WASM Battleship medians of 0.995–0.998×. Their timing ranges overlap. Only seven native and seven WASM observations met the journal's non-overlapping-range criterion; the other observations did not enter the gain history.

An additional isolated comparison against the immediately preceding search implementation, `16cb748`, measured **1.12–1.18×** native chess speedups on the same three positions. For the proven empty 3×3 search with one PV, the cache reduced native median time **167.7 → 121.1 ms**, WASM **208.0 → 148.2 ms**, retaining 11,604 and 11,360 nodes respectively. These are fixed-work comparisons, separate from the earlier reduction in repeated iterations. Their raw samples are `timeline-cache-isolated-{chess-native,solved-native,solved-wasm}.json` under ignored `target/monumla-bench/`; full matrices are `timeline-cache-{native,wasm}-matrix/`.

All **226 release Rust tests**, Clippy, the production build and **100 desktop/mobile browser tests** passed, including history navigation and both worker modes. The performance and gain-history images have been regenerated. No Elo gain is inferred from these speed measurements.

## First Stockfish reference calibration

The first finished reference match scored **10 wins, 0 draws, 2 losses** against **Stockfish 19, UCI_Elo 1320**, at **120 seconds + 1 second increment**, one search thread per engine. All six swapped-color pairs finished; every game ended by checkmate, with no time/search forfeits or exclusions. The frozen host source is `f05cb34`, with the shared engine implementation from `d0ab8ac` unchanged. Binary identities and all options are retained in the result.

The estimate is **1599.6 Elo on the Stockfish UCI_Elo reference scale**, with a conservative paired 95% interval of **1172.0–2428.7**. Twelve games give only a coarse initial calibration, not a precise human rating or evidence of an improvement over an older Monumla version. Additional reference levels are being tested to find a closer, approximately equal-strength opponent.

The exact calibration control and CCRL anchor are documented in [Stockfish's UCI reference](https://official-stockfish.github.io/docs/stockfish-wiki/UCI-Protocol-and-Stockfish-Commands.html#uci_elo). All 840 imported positions were audited through ordinary chess rules: final winners agreed with every result, and no claimable-draw positions occurred. All twelve settings files matched the declared reference level and clock. PGNs, move logs and the audit remain under ignored `target/monumla-bench/chess-reference-1320-d0ab8ac/`; clock settings in the manifest and `settings.tsv` are authoritative, while the legacy PGN `SearchBudgetMs` tag still describes the unused fixed-budget fallback.

## Find an even Stockfish opponent (2026-10-28)

The same frozen Monumla host and Stockfish 19 binaries completed the second predeclared discovery batch at **UCI_Elo 1600**, still at **120s+1s**. Monumla scored **5 wins, 0 draws, 7 losses (41.7%)**. All six color-swapped pairs finished by checkmate, without forfeits or exclusions. The interrupted run preserved its three completed games and archived incomplete traces before resuming. All **1,113 positions** passed the rules audit; actual level and clock settings matched the plan. The expanded intended-move claim audit is described below.

[The discovery curve](elo-calibration.png) shows both tested settings: 10–0–2 at 1320 and 5–0–7 at 1600. These are different opponents for the same engine, not successive engine improvements. The per-setting 95% bounds remain broad. The 1600 result is inside the predeclared 40–60% target band, selecting it for a **fresh 48-game confirmation**, whose complete schedule was recorded before play. The confirmation subsequently completed; its result replaces the older preliminary estimate below.

The durable driver in `scripts/bench/reference_ladder.py` records every batch before play and resumes the same schedule after interruption. Discovery chooses the reference setting; only the independent confirmation determines the final fixed-sample Elo interval. Raw discovery, confirmation, plans and restart metadata remain under ignored `target/monumla-bench/chess-calibration-d0ab8ac/` and the original reference directories.

## Fixed chess confirmation (2026-10-28)

[The Elo graph](elo.png) and [complete reference result](elo.json) now use all **48 predeclared confirmation games** against Stockfish 19 at **UCI_Elo 1600**, **120s+1s**, one search thread each: **13 wins, 1 draw, 34 losses (28.125%)**. All 24 swapped-color pairs finished, with **zero forfeits, unfinished games or exclusions**. There were 47 checkmates and one rules-defined dead-position draw. All **5,047 positions** passed the frozen WASM rules audit, without result mismatches; each settings file matched the declared clocks and reference level. The expanded intended-move claim audit is described below.

The estimate is **1437.0 Elo on the Stockfish UCI_Elo reference scale**, with a conservative paired 95% interval of **1195.5–1638.5**. The original 12-game pilot's higher point estimate was uncertain; this is a larger calibration of the **same frozen engine**, not a measured regression or improvement. The host remains `f05cb34`, shared engine implementation `d0ab8ac`, and the binary SHA-256 remains `2f0fbe22f5af0e110f872aac361f42f828dd579c9f94efe88358799dc206b0a6`. The small three-prefix opening suite limits how representative this reference is; it is not a FIDE or Lichess rating.

The confirmation shows that 1600 is stronger than the desired even opponent in this sample. A follow-up selection plan, recorded when 24 confirmation games had completed, calls for further full 12-game level checks if needed. These checks use the existing bracket rule and retain every result. **The original 48-game confirmation always determines the primary estimate and interval**, regardless of later scores; later checks cannot replace or improve that result selectively. The level checks appear separately in [the calibration curve](elo-calibration.png). Raw results, PGNs, immutable declarations and audits remain under ignored `target/monumla-bench/chess-calibration-d0ab8ac/`.

## Intended-move draw audit

The first auditor checked current-position draw flags. It now also checks claims by an intended move and verifies claimed terminations without trusting PGN result tags. The first **96 completed games** were re-audited with the frozen WASM rules. The 48-game primary match contains **15 optional intended-move claim positions**: 13 offered to Stockfish in games it eventually won, and two offered to Monumla in a game it won. No Stockfish draw opportunity was missed in a game Monumla won. Scores and the primary estimate are unchanged.

The 1375 follow-up batch finished **8–1–3**. Its draw is a valid fifty-move claim by an intended move at halfmove 99. The exported PGN imports it as a generic recorded draw and omits the original intended SAN. The auditor independently verified legal witnesses `Kf5` and `Kf4`, applied a legal claim through the frozen API, and checked the exact resulting outcome. It records a witness rather than claiming to recover the original choice. Three optional repetition claims earlier in that game were available to Stockfish; the game ultimately ended drawn. Seven frozen-WASM integration checks cover valid current/intended claims, fabricated results, the claiming player and natural checkmate. These audit changes affect no engine, rules or match result.

## Six-level chess calibration completed (2026-10-28)

All **120 games** finished at **120s+1s**, with the same frozen Monumla and Stockfish binaries, one thread per engine, paired openings and four CPU slots. No game was discarded, forfeited or left unfinished.

| Stockfish UCI_Elo | Stage | Monumla W–D–L | Score |
|---|---|---|---|
| 1320 | Initial pilot | 10–0–2 | 83.3% |
| 1375 | Follow-up check | 8–1–3 | 70.8% |
| 1400 | Follow-up check | 11–0–1 | 91.7% |
| 1425 | Follow-up check | 9–0–3 | 75.0% |
| 1450 | Follow-up check | 2–0–10 | 16.7% |
| 1600 | Discovery probe | 5–0–7 | 41.7% |
| 1600 | Independent fixed confirmation | 13–1–34 | 28.1% |

[The complete level curve](elo-calibration.png) retains every batch, including the sharp and nonmonotone sample results. The **observed crossover is 1425–1450**, at the search's 25-Elo grid resolution. This is descriptive evidence from small batches, **not an exact equal-strength setting or a rating confidence interval**. Stockfish 19 chooses its weakened move at a depth determined by an integer skill component ([control definition](https://github.com/official-stockfish/Stockfish/blob/sf_19/src/search.h), [selection during search](https://github.com/official-stockfish/Stockfish/blob/sf_19/src/search.cpp)); this can introduce steps between nearby settings. Sampling noise remains substantial.

The follow-up initially allowed up to eight probe/check batches. After inspecting that limiter, a separate resolution-stop declaration was recorded **before inspecting the completed 1425 result**: finish that entire batch, then stop at an observed near-even score or a crossover bracket no wider than the existing grid. This timing and the reason are retained in the published metadata. The final bracket has no fresh interior setting on that grid, so endpoint batches were not repeated to obtain an apparently even score. No Stockfish code or calibration coefficients were copied into Monumla.

[The primary Elo result](elo.json) and [rating image](elo.png) remain **1437.0 [1195.5, 1638.5]** on the Stockfish UCI_Elo reference scale, using only the original **48-game confirmation**. Later checks did not refit, replace or select that estimate. This is a calibration baseline for the unchanged engine, not an Elo gain for its history journal.

All **120 PGNs and 11,973 positions** passed the expanded frozen-WASM audit. It recorded **28 intended-move claim opportunities**, zero current-position claims, one independently reconstructed legal fifty-move claim, and zero invalid results. No unclaimed Stockfish draw opportunity occurred in a game Monumla won. The published JSON retains aggregate audit counts and the frozen WASM checksum; detailed records remain in ignored run directories. **38 Python benchmark tests, seven frozen-WASM result/claim checks, formatting and changed-file ESLint passed.**
