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

## Allocation-free action keys on stable (2026-10-28)

The shared search now hashes borrowed action inputs directly instead of allocating a byte buffer and formatting choices through `Debug`. Rules, evaluation, pruning and move ordering are unchanged. Native hook profiles measured action-key time falling from **9.073 to 1.376 ms** for solved tic-tac-toe, **4.308 to 1.122 ms** for chess, and **29.885 to 5.582 ms** for midgame Go 19×19. These instrumented times identify the removed cost; the separate uninstrumented matrix measures whole-search performance.

[The complete matrix](latest-matrix.json) and [comparison image](performance.png) compare **`2575c09` → `bf2049a`** on **15 native and 15 WASM cases**, adding a complete classic tic-tac-toe solve to the existing suite. Both sides were freshly built with **stable Rust 1.99.1**; historical Rust 1.97.1 measurements are not joined to this compiler series. The measured candidate was built from an uncommitted patch whose exact contents were subsequently verified against the commit; the report retains that provenance and both artifact checksums. AMD Ryzen 7 5800X, native CPU 5, WASM CPU 4, Node 24.13.1, default release settings. Every case runs at least **five paired warmups and 1,000 ms of warmup work per binary**, followed by **nine measured alternating pairs**. Initial one-warmup measurements exposed V8 tier-up during a short case; those raw results remain preserved, and the entire matrix was rerun under the declared longer warmup rather than rerolling individual cases.

All 30 comparisons preserve depth, selective depth, nodes, score, PV, ranked lines and stop reason on every recorded iteration. The full tic-tac-toe solve reaches **nine actual plies**, taking **125.678 → 117.704 ms native (1.068×)** and **127.618 → 118.043 ms WASM (1.081×)**. Its displayed cap of 32 is only the configured limit. Across all cases the geometric mean is **1.018× native and 1.023× WASM**. Most chess and Go changes remain inside overlapping observed timing ranges. **Native Battleship setup is 2.7% slower with non-overlapping ranges**, and remains visible in the matrix; this localized cleanup is not an all-game performance win.

Only **five** observations satisfy the gain journal's non-overlapping-range rule: native solved, 5×5 and rectangular tic-tac-toe; WASM solved and 5×5 tic-tac-toe. They start separately identified stable/compiler and warmup series in [the gain history](performance-history.png); all earlier series remain intact. Ranges describe observed extrema, not confidence intervals. No Elo improvement is inferred. Raw runs, build manifests and native/V8 profiles remain under ignored `target/monumla-bench/action-hash-stable-*`; the repository contains compact summaries and images only.

All **227 release Rust tests**, warning-free Clippy, **40 Python benchmark tests**, Svelte checks, the production build and **100 desktop/mobile browser tests** passed. Profiling still attributes substantial Go time to occupancy lookups and superko board-key reconstruction; those remain the next optimization targets.

## Borrow occupancy for Go superko keys (2026-10-28)

**`d8af31c`** exposes the core board's existing occupancy as a read-only row-major slice. Go's normal superko rules use it to reconstruct the same color bytes without recreating coordinates and calling `entity_at` for every empty intersection. No cached rule result, search adapter, evaluation term or legality shortcut was added. A rectangular core-board test checks ordering; Go's independent coordinate-based reference verifies exact keys throughout capture/suicide/superko sequences on four board sizes, including owner validation and dangling-occupant behavior.

The frozen **`bf2049a` → `d8af31c`** comparison uses the same stable Rust 1.99.1, CPUs, 15-case suite, five/1,000-ms minimum warmups and nine measured pairs as above. **All 30 native/WASM cases preserve depth, selective depth, nodes, score, PV, ranked lines and stop reason on every recorded iteration.** Later accepted passes update the complete matrix and comparison image.

| Go case | Native median, ms | Native ratio | WASM median, ms | WASM ratio |
|---|---:|---:|---:|---:|
| 9×9 initial | 62.602 → 58.002 | 1.079× | 79.255 → 74.956 | 1.057× |
| 19×19 initial | 1706.049 → 1451.501 | 1.175× | 2292.625 → 1731.269 | 1.324× |
| 19×19 midgame | 2273.297 → 1984.736 | 1.145× | 2962.055 → 2384.998 | 1.242× |

All three native Go gains and both 19×19 WASM gains have non-overlapping observed ranges and enter the gain-only journal. The WASM 9×9 ranges overlap. Every other case remains visible with overlapping ranges; **there is no non-overlapping-range regression in this pass**. The all-case geometric means are **1.026× native and 1.045× WASM**. No Elo improvement is claimed, and the earlier action-hash pass's Battleship regression remains in its record.

Native instrumentation on the same 1,915-node Go midgame search measured **2332.75 → 2064.38 ms**, including actions **760.689 → 634.285 ms** and evaluation **217.668 → 161.749 ms**. Separate named V8 profiles reduced the combined self-sample share of board-key construction and `entity_at` from **29.62% to 14.00%**; sample shares are diagnostics, not timing confidence bounds. State-delta construction and validation remain prominent costs. Raw matrices, matching source-patch/build manifests and both profiles remain under ignored `target/monumla-bench/go-occupancy-*`.

Verification passed: **229 release Rust tests**, warning-free Clippy, **40 Python benchmark tests**, Svelte checks, production build, **100 desktop/mobile browser tests**, and native/WASM compatibility checks on Rust **1.80.0**. The browser WASM exactly matches the measured candidate's SHA-256.

## Rejected sorted-map delta scan (2026-10-28)

Candidate **`894442b`**, preserved off `main` on `perf/delta-scan-rejected`, replaced repeated ID lookups during core state-delta construction with a linear alignment of sorted maps. **231 release Rust tests** and warning-free Clippy passed, including all 65,536 pairs of eight-key sparse map subsets and exact removal/update/addition ordering. All **30 native/WASM comparisons** preserved work, answers and selective depth under the same stable compiler, cases, warmups and nine measured pairs.

The end-to-end result did not justify shipping it: geometric means were **1.0004× native and 1.0017× WASM**, with **no non-overlapping-range gain in any case** and a **3.3% native Battleship setup regression**. Native instrumented Go time was **1922.32 → 1937.48 ms**. V8 self samples inside `StateDelta::between` fell from about 439 to 404 ms, but that local reduction did not produce a whole-search speedup. The candidate was removed from `main`; the accepted Go matrix and gain-history images remain unchanged. Complete raw matrices, profiles and source/build manifests remain under ignored `target/monumla-bench/delta-scan-*`. No journal point or Elo gain was recorded, and no journal bridge comparison was needed.

## Single-traversal fingerprints (2026-10-29)

**`d589aa4`** feeds the two existing fingerprint hash states during one traversal of each value. The hashing helper now has its own core module, with the same public entry point. It retains the existing salt and byte encodings. Independent two-pass compatibility tests cover every primitive write, string prefixes, byte/string lengths, nested state values and signed zero on both stable and Rust 1.80.0. A separate protocol audit confirms that **all 15 native root position keys are unchanged**.

[The complete matrix](latest-matrix.json) and [comparison image](performance.png) compare **`d8af31c` → `d589aa4`** under the same stable Rust 1.99.1, 15-case native/WASM suite, five/1,000-ms minimum warmups and nine measured alternating pairs. Every recorded iteration preserves depth, selective depth, nodes, score, PV, ranked lines and stop reason. The all-case geometric means are **1.0004× native and 1.0169× WASM**: there is no demonstrated native speedup. **WASM international checkers improves 5.6%**, from **110.961 to 105.109 ms**, with observed ranges **108.327–116.017 ms** and **104.157–107.943 ms**. That is the only case with non-overlapping gain ranges; every other comparison overlaps, and no case has a non-overlapping-range regression.

Only that WASM observation enters [the gain history](performance-history.png). Its series starts at the directly measured predecessor; no bridge comparison is needed and all earlier series remain intact. The full source patch was matched to the commit after measuring the frozen candidate. Raw records, build provenance, unchanged-key audit, native hook timings and named V8 profiles remain under ignored `target/monumla-bench/fingerprint-walk-*`. These measurements establish a limited WASM performance gain, not an Elo gain.

Verification passed: **231 release Rust tests**, warning-free Clippy, Svelte checks, production build, **100 desktop/mobile browser tests**, native/WASM checks on Rust **1.80.0**, and the fingerprint compatibility tests on that minimum compiler. The browser WASM checksum exactly matches the measured candidate. Development continues to follow unpinned `stable`.

## Native instruction profiles (2026-11-08)

Valgrind Callgrind **3.27.1** successfully profiled the normal frozen **`d589aa4`** release worker on CPU 5, without kernel sampling access or a special instrumented Rust build. The official Fedora package and its annotation scripts were extracted under ignored `target/monumla-bench/tools/`; no project dependency or system installation was added. Each trace includes initialization/prelude followed by one fixed-depth, node-limited search with no time deadline. All five results match the uninstrumented worker's depth, selective depth, nodes, score, PV, lines and stop reason.

| Representative case | Reported tree nodes | Whole-process instruction references |
|---|---:|---:|
| Chess, Italian position | 2,879 | 5,593,119,107 |
| Russian checkers | 281 | 422,783,108 |
| Go 19×19 midgame | 1,915 | 19,482,849,825 |
| Battleship, damaged fleets | 10,000 | 8,052,202,820 |
| Classic tic-tac-toe solve | 11,604 | 1,102,987,504 |

[The compact report](profiling.json) retains the binary checksum and leading exclusive costs. The chess single-square attack query accounts for **28.10% of process instruction references including its callees**. Go's board-key construction, state validation and delta construction account for about **11.82%, 11.43% and 10.35% exclusively**. Battleship repeatedly formats shot metadata keys; the `shot` query accounts for **53.65% including callees**. Tic-tac-toe spends **21.72% exclusively** in the hash-writing routine. These identify different optimization targets across games.

Instruction references are diagnostics, **not NPS or native wall time**. Inclusive costs overlap and cannot be summed. Valgrind reported its emulated brk-region limit during these runs, allowing libc to take its mmap fallback; that limitation is recorded per case. The searches still completed with identical work and answers. All complete traces, requests, responses, annotation reports and logs remain under ignored `target/monumla-bench/native-callgrind/`. No performance-journal or Elo point is inferred from profiling alone.

## Direct chess attack queries (2026-11-08)

**`a7ed9ac`** optimizes the existing chess legality query used for check, king moves and castling. Built-in pieces can answer whether they attack one requested square without allocating their full attack list. Sliding pieces reject unrelated directions immediately and inspect only the path to the target. The full attack-list API remains available, and custom pieces inherit a default single-square query using their existing list. The shared search continues to consume the same ordinary rules interface.

Independent comparisons against the original attack lists cover **6,534,912** combinations of every built-in piece, side, origin, target and single blocker, including friendly blockers, occupied targets, board edges and out-of-bounds targets. All **233 release workspace tests** pass, along with all three ignored perft gates: initial depth four, Kiwipete depth three, and the en-passant/discovered-check position at depth four.

[The complete 30-case matrix](latest-matrix.json) and [comparison image](performance.png) compare **`d589aa4` → `a7ed9ac`**, stable Rust 1.99.1, with the same CPUs, case suite, five/1,000-ms minimum warmups and nine measured alternating pairs. Every recorded search preserves completed depth, selective depth, nodes, score, PV, ranked lines and stop reason.

| Chess case | Native median, ms | Native ratio | WASM median, ms | WASM ratio |
|---|---:|---:|---:|---:|
| Italian | 505.088 → 408.149 | 1.238× | 635.603 → 530.411 | 1.198× |
| Central knights | 423.632 → 360.488 | 1.175× | 520.745 → 450.475 | 1.156× |
| Queenside | 271.314 → 227.257 | 1.194× | 340.738 → 293.471 | 1.161× |

All six chess gains have non-overlapping observed ranges and enter the gain-only journal. **Native Battleship-ready is 1.4% slower with non-overlapping ranges** and remains visible in the complete matrix. Every other case overlaps. The all-case geometric means are **1.039× native and 1.031× WASM**; this pass establishes chess speedups, not an all-game win or Elo improvement. No journal predecessor bridge is needed, and earlier gain series remain intact.

Callgrind on the identical saved 2,879-node chess request reports **5,593,119,107 → 4,544,791,267** process instruction references, about **18.7% fewer**, with identical results. [The profile report](profiling.json) retains the matched comparison. Separate native hook instrumentation measured **524.10 → 423.10 ms** on the same search; named V8 profiles are also retained. These diagnostics are separate from the uninstrumented timing matrix. Raw matrices, profiles, request replay, source-patch mapping and frozen artifact checksums remain under ignored `target/monumla-bench/attack-query-*`.

Verification passed: **233 release Rust tests**, all **three slow perft gates**, warning-free Clippy, Svelte checks, the production build, **100 desktop/mobile browser tests**, and native/WASM compatibility checks on Rust **1.80.0**. The browser WASM checksum exactly matches the measured candidate.
