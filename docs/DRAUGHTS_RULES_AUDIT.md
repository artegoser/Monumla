# Draughts rules coverage

`monumla-games-checkers` owns both variants. `DraughtsVariant` is game configuration; the generic runtime has no draughts-specific enum. `checkers` selects Russian draughts, and `checkers-international` selects international draughts.

The digital board rules follow the [IDF draughts-64 rules](https://fmjd64.org/rules-of-the-game/) and [FMJD international rules, Annex 1](https://www.ffjd.fr/fichiers/reglements/FMJD_Annexes_2018_en.pdf).

| Behavior                                      | Russian                                                                | International                                           |
| --------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------- |
| Board and initial men per side                | 8×8, 12                                                                | 10×10, 20                                               |
| Quiet man move                                | One square forward                                                     | One square forward                                      |
| Man capture                                   | Forward and backward                                                   | Forward and backward                                    |
| King movement and captures                    | Flying diagonals                                                       | Flying diagonals                                        |
| Capture priority                              | Any complete capture path                                              | Greatest number of victims across all pieces            |
| King priority when capture lengths tie        | None                                                                   | None                                                    |
| Promotion during capture                      | Immediate; continue as king                                            | At turn end, only when finishing on the last row        |
| Previously captured pieces                    | Block until turn ends; cannot be jumped twice                          | Same                                                    |
| No pieces or no legal move                    | Loss                                                                   | Loss                                                    |
| Repetition                                    | Third occurrence with same player to move                              | Same                                                    |
| King-only moves without man moves or captures | 15 moves each                                                          | 25 moves each                                           |
| Small ending limits                           | 5 moves each; 15 moves by stronger side with 3+ kings against one king | 5 or 16 moves each; the 16-move clock survives captures |
| Unchanged material with kings on both sides   | 30 moves each for 4–5 pieces; 60 for 6–7                               | Not applicable                                          |

Russian five-move endings include up to two pieces with a king against a lone king, and three pieces with a king against a lone king on the long diagonal. A decisive final move takes precedence over a draw clock.

Move generation returns complete paths, including intermediate landings and victims. Browser interaction offers one legal landing at a time, preserving capture priority after each step. Every jump records an authoritative transaction; the whole chain commits as one turn. `play_move` reuses that interaction path, rejects forged/incomplete turns, and leaves the source state untouched. Pending victims remain on the board until the last jump. Undo and redo restore the whole turn, including promotion and draw counters.

Draw counters are state owned so undo/redo and search restore them exactly. Repetition compares board occupancy, ranks, and side to move through committed history, ignoring entity IDs and counters. Outcome evaluation skips unfinished capture chains.

Tests cover setup, two-ply opening counts, backward captures, mandatory and maximum captures, priority across actors and continuations, flying landings and blockers, promotion differences, incomplete-path rejection, forced-sequence cancellation, history, terminal precedence, and draw counters.

This is a digital board-rules implementation. Physical touch-move, arbiter penalties, clocks, and rapid/blitz-specific administrative draw procedures belong to a session layer. Exhaustive tablebase adjudication of positions that neither player can win is not implemented. Draw agreement and resignation controls are not yet exposed for draughts.
