# Search-hook profile

Target: final board of [`../after-probe-fix-2026-10-23/game-002.pgn`](../after-probe-fix-2026-10-23/game-002.pgn), with the recorded forfeit result ignored. Depth one, 10,000-node cap, no deadline. The ordinary runtime rules and shared evaluator are used.

The before/after profiles both visited **128 nodes** and returned **−157** from Black's perspective. These are single diagnostic runs on the recorded machine, not statistical speed estimates.

| Hook | Calls | Before, ms (`89e1bf6`) | After, ms (`9792e70`) |
|---|---:|---:|---:|
| Total search | — | 992.94 | 607.73 |
| Quiescence | 37 | 877.097 | 526.207 |
| Action priority | 157 | 34.546 | 0.014 |
| Evaluation | 127 | 26.964 | 26.523 |
| Outcome | 128 | 11.830 | 11.500 |
| Play | 119 | 26.730 | 27.432 |

Reading cached turn effects directly avoids copying full state snapshots merely to classify and order moves. The optional stand-pat witness also avoids full tactical generation on eligible cutoffs; it was not triggered in this particular depth-one profile. A starting-position depth-two profile used it 134 times, finishing 185 nodes in 341.34 ms. The other forfeited position finished depth one in 635.56 ms after the copy reduction.

The desktop/mobile midgame browser regression reached its required depth in about 8 seconds after the change, versus about 32 seconds in preceding runs. Those are observed test durations, not a guarantee for arbitrary positions or hardware.

Reproduce with the `profile_search` example documented in the benchmark README. Match results are recorded separately in `report.md`; neither these timings nor two match games establish an Elo gain.
