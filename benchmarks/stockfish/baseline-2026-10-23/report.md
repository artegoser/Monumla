# Monumla vs Stockfish 19

Source: `9db54e3e86a7396630bb9bc4f37df0766167a4fd` (dirty workspace: false).

Equal nominal search budget: 1000 ms/move; one thread each. Stockfish: Hash 16 MB, Skill Level 20, UCI_LimitStrength false, no pondering or opening book. Monumla: shared RuntimeDomain, depth cap 32, 50M-node cap, 16384 transposition slots, default quiescence and ordering, one PV. No game-specific evaluator or search. Matched opening pairs swap colors.

Monumla: **0 wins, 0 draws, 2 losses, 0 unfinished**. Unfinished games at the ply cap are not draws. This small sample is a baseline, not an Elo estimate. Deadlines are checked between nodes; generation and move/notation overhead can exceed the nominal budget. See moves.tsv for actual elapsed time and search depth.

| Game | Monumla | Result | Plies | Termination |
|---|---|---|---|---|
| 1 | White | 0-1 | 38 | chess.checkmate |
| 2 | Black | 1-0 | 21 | chess.checkmate |
