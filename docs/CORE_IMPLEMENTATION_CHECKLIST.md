# Monumla Rust Core Implementation Checklist

This checklist tracks the implementation described in `CORE_IMPLEMENTATION_PLAN.md`.

## Architectural invariants

- [x] `monumla-core` contains no chess-specific piece/rule concepts.
- [x] Core player identity uses `PlayerId`, not white/black.
- [x] Core supports `TeamId` independently from `PlayerId`.
- [x] Entities have distinct `owner` and `controller` fields.
- [x] Entities have stable IDs and an extensible custom-state mechanism.
- [x] Core entities carry no ruleset-specific movement-history field; rulesets use entity-local state/history as needed.
- [x] History is readable by rules, not only by undo code.
- [x] Normal-play en passant is derived from history rather than a global target flag.
- [x] Normal-play castling eligibility is derived from entity/current state rather than global castling-right bits.
- [x] Piece/rule code can mutate a transactional working game state directly.
- [x] Gameplay is not constrained by a closed central `Effect` enum.
- [x] Structural state changes are separately exposed for frontend animation/debugging.
- [x] Optional semantic presentation cues are non-authoritative.
- [x] One player turn may contain multiple sequential steps.
- [x] Continuation choices are computed from the updated working state after each step.
- [x] Frontend interaction is driven by Rust-provided choices/opaque IDs.
- [x] Chess attack semantics are separate from movement semantics.
- [x] Speculative legality states are never committed to history.

## Phase 0 — Repository preparation

- [x] Add root `Cargo.toml` workspace.
- [x] Add `crates/monumla-core`.
- [x] Add `crates/monumla-games-chess`.
- [x] Add `crates/monumla-wasm`.
- [x] Define workspace dependency/version policy.
- [x] Add Rust formatting/lint/test commands.
- [x] Add WASM browser build integration skeleton.
- [x] Keep the existing Svelte application building during migration.
- [x] Document local development/build commands.

## Phase 1 — Generic world state

- [x] Implement `PlayerId`.
- [x] Implement `TeamId`.
- [x] Implement `EntityId`.
- [x] Implement `EntityTypeId`.
- [x] Implement `AbilityId`.
- [x] Implement `ChoiceId`.
- [x] Implement rectangular `Position`/coordinate primitives.
- [x] Implement rectangular `Board`.
- [x] Implement entity placement lookup.
- [x] Implement `PlayerState`/player store.
- [x] Implement `TeamState`/team store.
- [x] Implement `EntityState`/entity store.
- [x] Add entity `owner`.
- [x] Add entity `controller`.
- [x] Keep movement-history semantics out of generic `EntityState`.
- [x] Add extensible entity-local state storage.
- [x] Add ruleset-local state extension point.
- [x] Define and enforce core state invariants.
- [x] Unit-test board/entity/player/team operations.

## Phase 2 — History and turn sessions

- [x] Implement full `GameState` snapshots.
- [x] Implement `History`.
- [x] Implement `TurnRecord`.
- [x] Implement `StepRecord`.
- [x] Implement `TurnSession` with `before` and mutable `working` state.
- [x] Record multiple steps inside one turn.
- [x] Implement turn commit.
- [x] Implement turn rollback/cancel.
- [x] Add `previous_turn()` history query.
- [x] Add `last_step()` history query.
- [x] Add state-at-turn/history queries.
- [x] Add entity-at-history queries.
- [x] Ensure speculative legality checks do not enter history.
- [x] Implement undo foundation.
- [x] Implement redo foundation.
- [x] Test snapshot/history integrity across multi-step turns.

## Phase 3 — Generic interaction protocol

- [x] Define generic interaction/choice data model.
- [x] Support selecting an entity.
- [x] Support selecting a board position.
- [x] Support selecting an ability.
- [x] Support selecting a generic option.
- [x] Support explicit `FinishTurn` when a ruleset needs it.
- [x] Assign opaque stable-enough `ChoiceId`s for a current interaction state.
- [x] Reject stale/invalid choice IDs safely.
- [x] Allow a simple move to skip an unnecessary ability menu.
- [x] Re-query choices after every committed step in a turn session.
- [x] Add test-only multi-step interaction flow.
- [x] Add test-only forced-continuation flow.

## Phase 4 — Transactional mutation and change tracing

- [x] Implement transactional working-state mutation.
- [x] Provide safe entity lookup/mutation helpers.
- [x] Provide move helper.
- [x] Provide spawn helper.
- [x] Provide remove helper.
- [x] Provide player/team/ruleset-state mutation access where permitted.
- [x] Define controlled raw-state mutation escape hatch.
- [x] Validate transaction before committing a step.
- [x] Compute or record structural before/after state changes.
- [x] Trace entity movement.
- [x] Trace entity addition/removal.
- [x] Trace entity type changes.
- [x] Trace custom entity-state changes.
- [x] Add optional presentation-cue channel.
- [x] Ensure presentation data cannot alter authoritative state.
- [x] Test rollback on invalid mutations.

## Phase 5 — Rules and presentation registration

- [x] Define generic entity-rule interface.
- [x] Define generic ruleset/game-rule interface.
- [x] Reserve a clean ability-rule extension point.
- [x] Implement rule/type registry without spreading centralized matches through core.
- [x] Provide rule context with current state.
- [x] Provide rule context with history.
- [x] Provide rule context with current turn session/steps.
- [x] Define frontend-facing entity presentation metadata.
- [x] Ensure presentation can depend on current entity/state context.
- [x] Test registering a non-chess test entity without modifying core.

## Phase 6 — Standard chess pieces

### Chess setup

- [x] Define `ChessSide` in `monumla-games-chess` only.
- [x] Map `ChessSide::White` to a concrete `PlayerId`.
- [x] Map `ChessSide::Black` to a concrete `PlayerId`.
- [x] Create standard 8×8 chess board setup.
- [x] Create standard initial entity placement.
- [x] Register all six standard piece rules.

### Pawn

- [x] Generate one-square forward movement.
- [x] Generate legal initial two-square pseudo-movement.
- [x] Generate diagonal attacked squares separately from movement.
- [x] Generate ordinary diagonal captures.
- [x] Respect blocking on forward movement.

### Knight

- [x] Generate all L-shaped pseudo-moves.
- [x] Generate knight attacked squares.
- [x] Handle friendly/enemy occupancy correctly.

### Bishop

- [x] Generate diagonal rays.
- [x] Stop rays correctly at first occupied square.
- [x] Generate attacked squares correctly.

### Rook

- [x] Generate orthogonal rays.
- [x] Stop rays correctly at first occupied square.
- [x] Generate attacked squares correctly.

### Queen

- [x] Generate combined bishop/rook rays.
- [x] Generate attacked squares correctly.

### King

- [x] Generate adjacent pseudo-moves.
- [x] Generate adjacent attacked squares independently from move legality.

## Phase 7 — Attack maps and legal chess actions

- [x] Locate each side's king robustly.
- [x] Query whether a square is attacked by a given side/player.
- [x] Detect current check.
- [x] Apply candidate action to a speculative state.
- [x] Reject actions leaving own king attacked.
- [x] Handle pinned pieces correctly.
- [x] Handle discovered checks correctly.
- [x] Handle double check correctly.
- [x] Validate king moves into/out of attacked squares.
- [x] Validate king captures using resulting-state attacks.
- [x] Expose only legal chess destinations/options to frontend interactions.

## Phase 8 — Special chess moves

### Pawn double move

- [x] Require appropriate starting geometry/state.
- [x] Derive initial double-step eligibility from the pawn starting rank.
- [x] Require both traversed/destination cells to be free.
- [x] Do not store pawn movement flags solely for double-step eligibility.

### En passant

- [x] Do not add a normal-play global en-passant target flag.
- [x] Pawn detects adjacent enemy pawn.
- [x] Pawn inspects the immediately preceding completed action/state transition.
- [x] Verify the exact adjacent pawn was the previous actor/mover.
- [x] Verify the pawn moved exactly two ranks in a legal initial double move.
- [x] Generate the correct destination.
- [x] Remove the bypassed pawn during the candidate transition.
- [x] Allow en passant only immediately after the double move.
- [x] Reject en passant if the resulting state exposes own king.
- [x] Test en passant for both colors/sides.

### Castling

- [x] Do not add normal-play global castling-right bits.
- [x] Require king entity-local `has_moved == false`.
- [x] Locate the correct rook.
- [x] Require rook entity-local `has_moved == false`.
- [x] Require clear path.
- [x] Reject while king is currently in check.
- [x] Reject if transit square is attacked.
- [x] Reject if destination square is attacked.
- [x] Move king and rook in one chess action/step.
- [x] Support king-side castling for both sides.
- [x] Support queen-side castling for both sides.
- [x] Verify rook moved away and returned still cannot castle.
- [x] Verify king moved away and returned still cannot castle.

### Promotion

- [x] Detect promotion rank after pawn move/capture selection.
- [x] Request explicit promotion choice from Rust interaction layer.
- [x] Offer queen.
- [x] Offer rook.
- [x] Offer bishop.
- [x] Offer knight.
- [x] Apply chosen type change.
- [x] Support promotion after capture.
- [x] Support underpromotion.

## Phase 9 — Chess outcomes and draw rules

- [x] Detect checkmate.
- [x] Detect stalemate.
- [x] Add resignation outcome/API.
- [x] Add draw-by-agreement outcome/API.
- [x] Reject draw-by-agreement recording until both players have made at least one move.
- [x] Record resignation/agreement/accepted claims as authoritative timeline turns so undo/redo restores them normally.
- [x] Build a repetition-equivalence position key.
- [x] Include side to move in repetition equivalence.
- [x] Include effective castling possibilities in repetition equivalence.
- [x] Include effective en-passant possibilities in repetition equivalence.
- [x] Detect claimable threefold repetition in the current position.
- [x] Detect legal declared next moves that would create the third repetition without committing the move.
- [x] Detect automatic fivefold repetition.
- [x] Track/reset the halfmove clock semantics needed for draw rules.
- [x] Detect claimable fifty-move rule in the current position.
- [x] Detect legal declared next moves that would complete the 50-move threshold without committing the move.
- [x] Detect automatic seventy-five-move rule.
- [x] Implement dead-position detection for standard material-dead classes (K vs K, K+B/K+N vs K, same-color bishops only).
- [ ] Exhaustive recognition of arbitrary dead positions with blocked material/pawns; this requires reachability search and is intentionally deferred from the hot-path rules runtime.
- [x] Ensure checkmate takes precedence where required over automatic move-count draw handling.

## Phase 10 — FEN

- [x] Remove TypeScript FEN authority.
- [x] Parse piece placement in Rust.
- [x] Parse side to move.
- [x] Parse castling field.
- [x] Parse en-passant target field.
- [x] Parse halfmove clock.
- [x] Parse fullmove number.
- [x] Serialize complete FEN.
- [x] Reconstruct movement metadata consistent with imported castling rights.
- [x] Ensure absent castling rights do not accidentally reappear from original-piece placement.
- [x] Synthesize minimal previous pawn state/action for imported en-passant target semantics.
- [x] Keep synthetic import history clearly distinct from known real history internally if needed.
- [x] Add FEN validation/errors.
- [x] Reject structurally impossible standard FEN material/pawn/king states.
- [x] Keep terminal outcome metadata from erasing the immediately preceding FEN en-passant target.
- [x] Add FEN roundtrip tests.
- [x] Add imported castling/en-passant continuation tests.

## Phase 11 — WASM bridge

- [x] Create browser-facing `GameHandle`.
- [x] Expose `new_chess()`.
- [x] Expose `from_fen()`.
- [x] Expose current `GameView`.
- [x] Expose current `InteractionView`.
- [x] Expose choice resolution.
- [x] Expose transition/change results.
- [x] Expose optional presentation cues.
- [x] Expose undo.
- [x] Expose redo.
- [x] Expose FEN serialization.
- [x] Expose compact history/move-log view.
- [x] Keep Rust internals out of frontend DTO contracts.
- [x] Keep calls coarse-grained across the JS/WASM boundary.

## Phase 12 — Svelte game-model migration

- [x] Wire the WASM module into the Svelte/Vite application.
- [x] Make `GameHandle` the authoritative local game state.
- [x] Remove gameplay use of `src/lib/pieces/*.ts`.
- [x] Remove TypeScript `BoardPieces` authority.
- [x] Remove TypeScript FEN parser/authority.
- [x] Preserve existing SVG piece assets.
- [x] Adapt `Board.svelte` to `GameView`.
- [x] Adapt piece rendering to entity/presentation DTOs.
- [x] Render Rust-provided legal selections.
- [x] Send selected `ChoiceId`/input back to WASM.
- [x] Ensure Svelte contains no duplicated legality checks.

## Phase 13 — Lichess-parity interactive local chess UI

Reference: [`LICHESS_BOARD_UX_SPEC.md`](./LICHESS_BOARD_UX_SPEC.md). The target is the standard Lichess board interaction experience, implemented independently in Monumla with Rust/WASM remaining authoritative.

### Authority boundary

- [x] Keep all legal-origin/legal-destination generation in Rust/WASM.
- [x] Keep castling/en-passant/promotion/check semantics out of Svelte.
- [x] Map Rust-issued opaque `ChoiceId`s to board origins/destinations/options.
- [x] Ensure drag and click use exactly the same Rust-issued choices.
- [x] Never commit speculative board state in Svelte before Rust accepts a choice.

### Click selection

- [x] Select a controllable piece/entity by click.
- [x] Clicking another controllable piece switches selection.
- [x] Clicking a legal destination executes the Rust-issued destination choice.
- [x] Clicking the selected origin clears it where Lichess does.
- [x] Clicking an invalid square clears/recomputes selection with Lichess-compatible behavior.
- [x] Clear/recompute selection after every authoritative transition.

### Drag and drop

- [x] Add true pointer-driven piece drag; do not emulate drag as click-click.
- [x] Use an approximately 3 CSS-pixel drag threshold on desktop.
- [x] Support one-finger touch drag.
- [x] Keep the dragged piece centered under/following the pointer.
- [x] Raise the dragged piece above normal pieces/animations.
- [x] Show a translucent origin ghost when highlighting is enabled.
- [x] Keep legal destination markers visible while dragging.
- [x] Add Lichess-style legal-destination hover feedback during drag and ordinary pointer hover.
- [x] Drop on a legal target by submitting its existing Rust `ChoiceId`.
- [x] Cancel cleanly when dropped on origin, outside the board, or on an illegal target.
- [x] Ensure cancelled drag leaves authoritative state unchanged.
- [x] Ensure active piece movement animation does not conflict with drag transforms.

### Destination/highlight parity

- [x] Render quiet legal destinations as Lichess-style centered radial dots.
- [x] Render occupied/capture legal destinations as four Lichess-style triangular corner/edge wedges instead of dots.
- [x] Determine occupied-target presentation from `GameView`, not chess geometry in Svelte.
- [x] Add full-square hover feedback for legal destinations.
- [x] Add selected-origin highlight.
- [x] Add last-move origin highlight.
- [x] Add last-move destination highlight.
- [x] Add Lichess-style radial red check highlight on the checked king square.
- [x] Define deterministic layering when selected/last-move/check/destination states overlap.

### Animation parity

- [x] Animate ordinary movement from structural deltas/before-after entity positions.
- [x] Target approximately 200 ms default move animation with Lichess-like easing.
- [x] Fade/remove captures while the capturing piece moves.
- [x] Animate king and rook concurrently for castling.
- [x] Animate en passant as capturing-pawn movement plus independent victim removal.
- [x] Animate promotion movement and resulting type/presentation change without teleporting.
- [x] Keep animation implementation generic and driven by `StateDelta`, not chess-specific frontend rules.
- [x] Keep authoritative `GameView` positions independent from temporary animation offsets.
- [x] Make animation offsets explicit Svelte dependencies so cleanup/undo/redo cannot leave stale piece transforms.
- [x] Cancel stale animation frames when a newer transition supersedes them.
- [x] Start move/capture animation explicitly after DOM synchronization (Web Animations API) instead of relying on a CSS-transition paint race.
- [x] Ensure ordinary enabled animations either run for the configured duration or are explicitly cancelled/superseded, never randomly teleport because an intermediate style was not painted.
- [x] Respect reduced-motion preferences.

### Promotion

- [x] Replace generic text promotion buttons with a board-local graphical chooser.
- [x] Render queen/rook/bishop/knight graphics from Rust-provided options.
- [x] Submit the corresponding opaque option `ChoiceId`.
- [x] Keep underpromotion equally accessible.
- [x] Define explicit cancellation behavior without fabricating a move.

### Geometry/orientation/touch

- [x] Keep standard chess board exactly square and responsive.
- [x] Preserve exact square hit boxes after resize.
- [x] Support white orientation.
- [x] Support black orientation.
- [x] Flip coordinates with orientation.
- [x] Verify click hit testing in both orientations.
- [x] Verify drag hit testing in both orientations.
- [x] Avoid duplicate mouse actions after touch interaction.
- [x] Avoid blocking page scroll unless a board interaction actually begins.

### Board appearance

- [x] Match the default Lichess brown-board visual relationship closely.
- [x] Match Lichess-like piece scale/alignment within squares.
- [x] Keep subtle rounded outer board corners, clip overlays/pieces consistently, and avoid clipping coordinate labels.
- [x] Remove current Monumla-only destination marker styling that visibly differs from Lichess.
- [x] Remove/adjust current board-only styling that visibly prevents side-by-side Lichess parity.
- [x] Do not add any Lichess/Chessground runtime dependency.
- [x] Do not copy upstream source/CSS verbatim; independently recreate the behavior/rendered result.

### Board-local game controls

- [x] Show active side/player.
- [x] Show check status.
- [x] Show checkmate/stalemate/draw outcome.
- [x] Add undo control.
- [x] Add redo control.
- [x] Add reset-to-start control.
- [x] Add FEN input/load development control.
- [x] Add current FEN display/copyable field.
- [x] Add basic move/history display.

### Secondary Lichess board affordances

- [x] Design the interaction state so future premove support does not require a drag/selection rewrite.
- [x] Defer actual premove execution until a mode has a meaningful opponent-turn waiting period.
- [x] Add presentation-only right-click square annotations.
- [x] Add presentation-only right-drag arrows.
- [x] Keep drawings out of authoritative game state.

### Acceptance

- [ ] Verify a complete local two-player chess game can be played without reloads.
- [ ] Verify both click-click and drag-drop for ordinary moves and captures.
- [ ] Verify castling, en passant, and promotion through both relevant interaction paths.
- [ ] Verify capture targets are immediately distinguishable from quiet targets.
- [ ] Verify selected/last-move/check/hover states against Lichess side-by-side.
- [ ] Verify drag threshold, ghost, z-order, cancellation, and touch behavior side-by-side against Lichess.
- [ ] Verify move/capture/castling/en-passant/promotion animation side-by-side against Lichess.
- [ ] Confirm no frontend chess legality implementation was introduced.


## Phase 14 — Correctness suite

### Perft

- [x] Initial position depth 1 = 20.
- [x] Initial position depth 2 = 400.
- [x] Initial position depth 3 = 8,902.
- [x] Initial position depth 4 = 197,281.
- [x] Add known castling-heavy perft position(s).
- [x] Add known en-passant/check interaction perft position(s).
- [x] Add known promotion perft position(s).
- [x] Keep expensive depth-4/reference perft cases as an explicit release-mode slow correctness gate instead of running them in every development test cycle.

### Regressions

- [x] En passant cannot persist for an extra turn.
- [x] En passant can expose a rook/bishop/queen line and become illegal.
- [x] Castling cannot pass through check.
- [x] Castling cannot leave/enter check.
- [x] Returned rook does not recover castling eligibility.
- [x] Returned king does not recover castling eligibility.
- [x] Pinned piece legal actions are filtered.
- [x] Double-check responses are correct.
- [x] All four promotion types work.
- [x] Stalemate examples are correct.
- [x] Checkmate examples are correct.
- [x] Threefold/fivefold repetition behavior is correct.
- [x] Fifty/seventy-five move behavior is correct.
- [x] Dead-position examples are correct.
- [x] Undo/redo restores all gameplay-relevant state/history.
- [x] FEN import/export keeps continuation semantics correct.

## Phase 15 — SAN and PGN

- [x] Generate SAN for ordinary moves.
- [x] Generate SAN captures.
- [x] Generate SAN disambiguation.
- [x] Generate SAN castling.
- [x] Generate SAN promotions.
- [x] Generate SAN check/checkmate suffixes.
- [x] Export PGN from game history.
- [x] Import standard PGN main line.
- [x] Preserve and validate PGN `Result` instead of silently treating declared terminal games as ongoing.
- [x] Preserve source tags while recomputing authoritative `Result`/setup tags on export.
- [x] Accept common SAN annotations/NAGs while resolving through legal-move authority.
- [x] Reject PGN result declarations that conflict with a naturally terminal board.
- [x] Test PGN/FEN interoperability where applicable.

## Generic outcome architecture

Reference: [`GENERIC_OUTCOME_RULES.md`](./GENERIC_OUTCOME_RULES.md).

- [x] Add generic `GameOutcome` without chess-specific variants.
- [x] Support winner/loser players and teams plus extensible outcome data.
- [x] Add ruleset-level `OutcomeRule`.
- [x] Register multiple outcome rules with explicit deterministic precedence.
- [x] Keep terminal decisions out of `EntityRule`.
- [x] Add a core test proving first-matching outcome-rule precedence.
- [x] Adapt standard chess outcomes through `ChessOutcomeRule`.
- [x] Prove chess checkmate maps to the generic outcome contract.

## Generic composable game rules

Reference: [`GENERIC_GAME_RULES.md`](./GENERIC_GAME_RULES.md).

- [x] Replace the single optional core `GameRule` slot with ordered composable `GameRuleSet`.
- [x] Allow game rules to add top-level generic choices.
- [x] Allow game rules to transform/filter combined generic choices in deterministic registration order.
- [x] Apply only transforms, not unrelated global actions, during forced local continuations.
- [x] Allow a game rule to handle a choice it introduced or intercepted.
- [x] Run game-rule validation before choice resolution and authoritative chess move execution.
- [x] Move standard promotion trigger and target generation into `Pawn`.
- [x] Remove pawn/promotion/type branches from `ChessInteractionRules`.
- [x] Pass selected move continuation semantics opaquely through `ChoiceInput` (`ChoiceKind` + `StateMap`) without `ChoiceId`, labels, or presentation asset keys.
- [x] Separate choice presentation (`label`, `asset_key`) from semantic choice data before canonical replay recording.
- [x] Route any pending move continuation `ChoiceKind` through the same generic `ChoiceInput` path rather than hardcoding `SelectOption`.
- [x] Let `Pawn` validate promotion input and perform the entity-type mutation.
- [x] Remove promotion-specific data from the recorded `chess_move` action; derive SAN promotion from before/after state.
- [x] Make perft enumerate generated move continuations rather than hardcoding four promotion branches.
- [x] Make SAN/PGN resolve promotions through currently allowed generated move continuations.
- [x] Add a test proving a `GameRule` can filter pawn-owned promotion choices without redefining `Pawn`.
- [x] Prove direct authoritative execution cannot bypass a game-rule-filtered piece continuation.
- [x] Add core tests proving ordered global choice augmentation/filtering and global choice handling.
- [x] Add a generic transactional post-step reaction hook for terrain/environment mechanics without introducing a closed effect enum.
- [x] Prove chess execution runs registered game-rule reactions inside the same authoritative transaction.

## Phase 16 — Architecture proof

Reference: [`BUILTIN_RULESETS.md`](./BUILTIN_RULESETS.md).

- [x] Register non-chess checker and Go stone entity types without editing `monumla-core` enums.
- [x] Force an additional checkers capture continuation after the first capture step.
- [x] Read complete committed play history for AGA situational-superko legality.
- [x] Prove a primary game action can be placement/spawn rather than movement through the Go example.
- [x] Prove transactional group capture by removing multiple Go entities from one placement.
- [x] Confirm all built-in non-chess rulesets use existing generic choices, transactions, state, history, teams, abilities, and outcomes with no mechanic-specific core enum variants.
- [x] Expose all three built-in rulesets through one WASM `GameHandle` selected by ruleset ID.
- [x] Add a Svelte ruleset selector and make chess, checkers, and Go playable through the shared board/interaction pipeline.
- [x] Keep FEN/PGN as chess-only capabilities rather than leaking them into the generic runtime contract.
- [x] Verify Go board-position choices work without selecting or moving an existing entity.
- [x] Delete the temporary `monumla-examples` crate once the rulesets are independently runnable.
- [x] Replace the 9×9/living-stone Go proof with a configurable 9×9/13×13/19×19 AGA ruleset.
- [x] Implement pass stones, two-pass scoring review, independent dead-group assessment, dispute resume, and White-last-pass handling.
- [x] Implement exact-half-point AGA territory and area scoring with current even/handicap komi and area handicap compensation.
- [x] Implement standard 19×19 fixed handicap setup and one-stone-handicap semantics.
- [x] Render Go on intersections with conventional hoshi points rather than chess-style square centres.
- [x] Document Go completeness and tournament/session boundaries in `GO_RULES_AUDIT.md`.

## Phase 17 — Generic action notation and deterministic replay

Reference: [`GENERIC_ACTION_NOTATION.md`](./GENERIC_ACTION_NOTATION.md).

- [x] Add canonical ephemeral-free `ChoiceInput` (`ChoiceKind` + data), excluding `ChoiceId`, labels, and presentation asset keys.
- [ ] Persist accepted `ChoiceInput` values in turn history for generic action notation/replay.
- [ ] Enforce that `(ChoiceKind, data)` is unique within one generated `Interaction`.
- [ ] Record the ordered accepted decision trace for every committed non-synthetic turn.
- [ ] Keep transient/cancelled draft choices out of the committed decision trace.
- [ ] Preserve multi-choice input that resolves into one state-mutating step.
- [ ] Preserve multiple sequential state-mutating steps inside one turn.
- [ ] Add a ruleset/version identifier to portable game records.
- [ ] Replay records by resolving each recorded semantic choice against the currently generated interaction and submitting its fresh `ChoiceId`.
- [ ] Never replay by directly applying `StateDelta`.
- [ ] Optionally verify replayed states with deterministic hashes/checkpoints.
- [ ] Round-trip a normal chess move through the generic record.
- [ ] Round-trip castling and promotion through the generic record.
- [ ] Round-trip move + ability in one turn using the Phase 16 test ruleset.
- [ ] Round-trip an ability target + option choice.
- [ ] Round-trip forced multi-step continuation.
- [ ] Round-trip a history-dependent rewind/copy action.
- [ ] Round-trip a controller change without owner change.
- [ ] Keep SAN/PGN as a chess-specific pretty notation/import adapter rather than the universal record.
- [ ] Expose a deterministic generic expanded text/debug rendering for rulesets with no custom human notation adapter.

## Final acceptance

- [ ] Standard chess is fully playable locally through Svelte + Rust/WASM.
- [ ] Rust is the sole authority for game rules and state transitions.
- [ ] All standard legal-move edge cases are covered.
- [ ] All required terminal/draw conditions are covered.
- [ ] En passant uses pawn-observable history in normal play.
- [ ] Castling uses king/rook state in normal play.
- [ ] History supports rule queries and undo/redo.
- [ ] Generic turns support multiple sequential steps.
- [ ] Frontend animations are driven by state changes/presentation metadata rather than duplicated game logic.
- [ ] Standard chess board interaction is Lichess-parity for click, drag, quiet dots/capture wedges, selected/last-move/check feedback, rounded clipping, and deterministic special-move animation.
- [ ] Perft/regression suites pass.
- [ ] Architecture-proof tests pass.
- [ ] Generic action records replay arbitrary accepted choice sequences through the same authoritative interaction runtime.
- [x] Generic terminal outcomes are ruleset-level and independent of entity-rule implementations.
- [ ] No AI/search engine has been added yet.
- [ ] No multiplayer/server implementation has been added yet.
- [ ] No user-facing dynamic-piece DSL/runtime has been added yet.
