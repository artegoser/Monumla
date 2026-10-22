# Universal search: first implementation

`monumla_core::search` implements iterative-deepening alpha-beta search. It depends only on a `SearchDomain` trait: no pieces, boards, chess rules, draughts rules, or game-specific evaluation live in the algorithm. The two game crates supply their own adapters.

A domain supplies its position, complete action, perspective, error type, terminal utility, static evaluation, node role, legal actions, and successor function. Scores always use the caller's fixed perspective. Explicit maximizing/minimizing roles allow extra turns and opposing coalitions without assuming alternating colors. A position can include any history required by its rules.

```rust
use monumla_core::{search::{search, SearchLimits}, GameTimeline};
use monumla_games_checkers::{search::DraughtsSearch, standard_state, WHITE};

let position = GameTimeline::new(standard_state())?;
let result = search(&DraughtsSearch, &position, WHITE, SearchLimits {
    max_depth: 4,
    max_nodes: Some(10_000),
})?;
// result.best_action contains a complete turn, including every capture landing.
```

The result includes the best action, score, principal variation, completed depth, total visited nodes, and stopping reason. Node budgets apply across all iterations. `search_with_control` checks cancellation before each visited node; a host may supply a deadline or cancellation signal without requiring a native clock in WASM. Incomplete iterations never overwrite the last completed result. If interrupted before the first iteration finishes, the result explicitly has no score or best action. Depth zero only evaluates the position. An adapter with no actions in a nonterminal position returns an error rather than silently inventing a draw. The recursive implementation accepts depths up to 256.

## Available adapters

- `monumla_games_checkers::search::DraughtsSearch`: both Russian and international rules, complete capture turns, promotions, draw counters, and history-based repetition. It reuses production interaction execution. Evaluation uses material and man advancement.
- `monumla_games_chess::search::ChessSearch`: standard chess including castling, en passant, all promotion continuations, terminal outcomes, history, and optional draw claims, including claims by an intended move. Evaluation starts with material values; capture and promotion ordering stays in the game crate.

Terminal wins/losses score ±100,000; draws score zero. These numbers are adapter policy, not search-core knowledge. Both adapters use independent `GameTimeline` successors and preserve the source timeline, including undo/redo state. Every action in a returned principal variation can be replayed through the adapter.

Run the native example:

```bash
cargo run --release -p monumla-wasm --example analyze -- checkers 4 10000
cargo run --release -p monumla-wasm --example analyze -- checkers-international 4 10000
cargo run --release -p monumla-wasm --example analyze -- chess 2 10000
cargo run --release -p monumla-wasm --example analyze -- chess 2 10000 '7k/8/5KQ1/8/8/8/8/8 w - - 0 1'
```

## Current scope and next steps

This is a working search foundation, not a Stockfish-strength engine. The first algorithm handles deterministic, perfect-information games with two opposed utilities. A generic domain boundary does not make alpha-beta appropriate for hidden information, chance nodes, simultaneous actions, or independent multiplayer objectives. Those require additional algorithms. Go has no search adapter yet. Browser analysis controls and background workers are not included.

Current successors clone timelines for correctness, and game evaluation is deliberately basic. There is no transposition table, quiescence search, aspiration window, learned evaluator, tablebase, or parallel search yet. Cancellation bounds visited nodes, not the time spent inside a game adapter's move-generation call. Native example FEN imports cannot reconstruct repetition history absent from FEN.

Next milestones are measured performance and allocation work, a history-safe position-key contract and bounded transposition table, adapter-defined tactical extension hooks, stronger evaluators, and a cancellable background analysis host. These should retain the piece-independent domain boundary.

Verification includes generated abstract trees compared against exhaustive minimax, pruning, extra-turn roles, fixed perspectives, deterministic results, integer score extremes, node budgets, cancellation, domain errors, replayable principal variations, real draughts capture chains and repetition, and chess mate, promotions, and draw claims.
