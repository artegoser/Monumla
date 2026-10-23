# Universal search

`monumla-search` owns search, evaluation, move ordering, tactical continuations, and caching. Game crates provide their ordinary `InteractionRules` and outcome rules. They have no search modules and do not depend on `monumla-search`.

`RuntimeDomain` is the single connection between the core runtime and the search algorithm. It enumerates complete turns through authoritative choices, including draft inputs, promotion options, and capture continuations. Selection cycles and duplicate completed turns are discarded. Returned actions contain the core `TurnInput`, so a browser worker can send them back for validation and replay through the same rules as a human move. `EntityActionPath` also lives in core; draughts rules use it for legal capture paths.

The WASM host registers the existing chess, Russian/international draughts, or Go rules. Its game-specific switches create ordinary rules and format notation; they do not implement search or evaluation. Go uses this same route, including pass and scoring-review choices. Resignation is discarded when a non-losing continuation exists, based on the declared outcome rather than an action name.

## Algorithm

- Iterative deepening with alpha-beta pruning and principal variation search.
- Aspiration windows, with full-window re-search when needed.
- Bounded transposition storage with verified 128-bit keys, score bounds, and depth/generation replacement.
- Principal variation, killer, history, and generic effect-based action ordering.
- Bounded quiescence over entity removals, transformations, and control changes. A lazy complete-turn witness permits early stand-pat cutoffs only when a non-losing quiet continuation exists; compulsory captures keep the full tactical path. Cached turn effects are inspected without cloning their states.
- Up to eight ranked root continuations (the browser exposes one, three, or five).
- Node budgets, cancellation, and progress after complete iterations.

Incomplete iterations preserve the last completed primary result. Cancellation before the first completed iteration returns no score or best action. It never substitutes the first legal action. Scores use a fixed perspective; explicit maximizing/minimizing roles support extra turns and allied players. History participates in position identity, including action data, so repetition and superko contexts are preserved. History records are shared between cloned branches.

The shared heuristic estimates entity control, centrality, and legal-choice mobility. It probes the ordinary rules to estimate movement capacity for entity types and their state, preserving enough surrounding entities to keep probes nonterminal. This avoids treating a piece as immobile simply because isolation ended the game. It contains no piece IDs, game-specific values, state-key names, or game-specific move generators. Terminal outcomes dominate heuristic scores; mate-distance adjustment prefers faster wins and slower losses. Heuristic values are not calibrated centipawns, Go territory, or win probabilities.

This is finite search, not perfect play. A draw under perfect play does not imply that bounded self-play must draw. The heuristic has limited strategic knowledge, particularly in Go; it is not comparable in strength to Stockfish or specialist Go engines. The current algorithm targets deterministic, sequential, perfect-information games with two opposed utilities. Hidden information, chance, simultaneous turns, and independent multiplayer utilities need additional algorithms.

## Browser

**Play** defaults to a computer opponent, which moves automatically. Choose a side or a local opponent; thinking time is under engine settings. Engine scores, suggestions, and assessments are hidden in Play.

**Analysis** starts a background worker automatically and deepens until paused or a resource limit is reached. It shows the last completed score, ranked continuations, and ranked candidate arrows on the board, with matching colors and numbers. Selecting a line emphasizes its arrow. Settings control lines, arrows, evaluation, assessments, and coordinates. Evaluation is labeled as a generic heuristic. Entering Analysis starts a separate cancellable game review, beginning with the latest played move. It searches the best move and the played candidate from the same original position and compares their scores at the same completed depth. History entries and the last-move badge show the resulting assessments; tooltips give depth, score loss, and the preferred move. Shallow searches remain unassessed except when a large tactical loss is already visible. These are heuristic assessments, with no fabricated opening-book labels.

Move navigation supports buttons, history entries, and left/right arrow keys. Position changes, imports, mode changes, and unfinished turns cancel obsolete workers. Each result is tied to the original position and is revalidated before application. Bot play retries with a longer budget when its first iteration cannot complete. Undo pauses computer play so it does not immediately replay the undone move. FEN/PGN controls and game actions are in collapsible sections.

Workers keep the UI responsive; termination cancels synchronous WASM. Node/deadline checks occur between nodes, so one rule-generation call can exceed the nominal deadline. Browser analysis has a ten-minute, 50-million-node, depth-32 cap per run; resume starts a new run. The generic turn enumerator has a 128-input continuation guard and bounded action cache.

## Verification

```sh
cargo test --workspace
cargo run --release -p monumla-wasm --example analyze -- chess 2 10000
cargo run --release -p monumla-wasm --example analyze -- checkers 4 10000
cargo run --release -p monumla-wasm --example analyze -- checkers-international 4 10000
cargo run --release -p monumla-wasm --example analyze -- go 1 10000
pnpm check
pnpm test:browser
```

Tests compare abstract trees with exhaustive minimax, check caching and budgets, and search a new rules-only take-away game without a game search implementation. Host tests cover every built-in game, worker-style action replay, stale results, incomplete search, promotion continuations, mate, the reported chess midgame, the queen-hanging regression, and equal-depth move review. Browser tests cover automatic computer play, analysis-only hints, Go analysis, navigation, cancellation, settings, and desktop/mobile layout.

## Playing strength

Use [matches against Stockfish](../benchmarks/stockfish/README.md) as the chess strength benchmark for the shared engine. The runner saves matched color pairs, settings, per-move statistics, and PGNs. Rule tests and search-tree tests remain correctness gates; they are not playing-strength measurements.
