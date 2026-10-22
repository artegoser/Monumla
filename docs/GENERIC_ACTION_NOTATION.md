# Generic Action Notation and Replay Specification

## 1. Goal

Monumla needs a canonical game record that can represent and replay turns for any ruleset built on the generic core, not only standard chess.

The record must handle, without adding mechanic-specific variants to `monumla-core`:

- arbitrary entity types;
- arbitrary board sizes;
- movement and non-movement actions;
- named abilities;
- entity and position targets;
- arbitrary options;
- multiple sequential steps in one turn;
- forced continuation chains;
- state-changing abilities such as damage, healing, spawning, control changes, or rewind;
- two or more players and teams;
- future user-defined rulesets.

Standard chess SAN/PGN remains a chess-specific human notation. It is not the universal storage/replay model.

## 2. Core principle: record decisions, not effects

The canonical record must describe the authoritative choices that were accepted by the interaction runtime.

It must not use `StateDelta` as the command language. Deltas describe what changed after execution and are useful for presentation, diagnostics, and verification, but replaying deltas would bypass rules.

It must also not record ephemeral `ChoiceId` values. A `ChoiceId` is valid only for one generated interaction and intentionally becomes stale after refresh.

The universal replay identity of a choice is therefore its deterministic semantic payload:

```rust
ChoiceInput {
    kind: ChoiceKind,
    data: StateMap,
}
```

`label` and `asset_key` are presentation and are not part of canonical identity.

Within one generated `Interaction`, `(ChoiceKind, data)` must identify at most one choice. Phase 17 should enforce this invariant so a recorded choice can be resolved unambiguously during replay.

## 3. Turn decision trace

A committed `TurnRecord` should gain an ordered decision trace in addition to its existing before/after snapshots and structural step records:

```rust
TurnRecord {
    actor: PlayerId,
    decisions: Vec<ChoiceInput>,
    before: GameState,
    steps: Vec<StepRecord>,
    after: GameState,
    synthetic: bool,
}
```

This is intentionally turn-level rather than one decision per `StepRecord`.

A single logical step may require several choices before it can execute:

```text
select entity
select ability
select target entity
select option
=> one state-mutating step
```

Conversely, one turn may contain several independently committed steps:

```text
move
cast Fireball
target entity
finish turn
```

Recording the ordered accepted choices preserves both cases without teaching core what a move, spell, capture, combo, or promotion means.

## 4. Cancellation and draft input

Transient UI exploration must not pollute the canonical record.

Example:

```text
select entity A
select ability Fireball
cancel
select entity B
move B
finish
```

The cancelled `A -> Fireball` path should not be stored in the committed turn.

`InteractionDriver` should therefore keep a decision checkpoint at the last authoritative state mutation. Choices after that checkpoint are pending draft input. `reset_draft()` discards decisions back to that checkpoint. When a choice produces one or more committed steps, the decision checkpoint advances. A successfully accepted `FinishTurn` choice remains part of the committed decision sequence when it is semantically required by the interaction flow.

This preserves the distinction already present in the architecture between transient interaction draft and authoritative world state.

## 5. Canonical replay algorithm

Replay always goes through the same rules and interaction protocol as live play.

For every recorded turn:

1. start a `TurnSession` for the recorded actor;
2. create the ruleset's normal `InteractionDriver`;
3. for each `ChoiceInput`, inspect the currently generated `Interaction`;
4. find exactly one choice whose `kind` and deterministic `data` equal the recorded choice;
5. submit that choice's newly generated ephemeral `ChoiceId`;
6. require the turn to finish at the same point as the record;
7. commit through `GameTimeline` normally;
8. optionally compare the resulting state/hash with the recorded verification data.

A record never directly mutates `GameState` during replay.

This means replay automatically exercises current legality, forced continuations, history-dependent rules, abilities, and multi-step turns.

## 6. Record envelope

A portable game record needs enough context to know which rules can replay it:

```text
GameRecord
├─ format version
├─ ruleset identifier
├─ ruleset version / compatibility identifier
├─ initial state or ruleset-specific initial-state payload
├─ optional metadata
├─ committed turns
│  └─ actor + ordered ChoiceInput values
├─ optional terminal/outcome metadata
└─ optional verification hashes
```

The ruleset version is important. If a later version intentionally changes available choices or their semantics, the runtime must not silently pretend an old record means the new rules.

The initial state may be stored directly as generic `GameState` for internal/test records. Public rulesets may additionally provide compact import/export adapters such as FEN for chess.

## 7. Human-readable notation is a presentation layer

One universal compact human syntax cannot remain equally ergonomic for chess, checkers, a Fireball ability, a card-like target selector, and an Ekko-style history rewind without becoming a programming language.

Monumla should therefore separate:

### Canonical generic record

Lossless and ruleset-agnostic. Used for replay, persistence, debugging, networking logs, and deterministic tests.

### Ruleset-specific notation adapter

Optional human-facing formatter/parser.

Examples:

- standard chess: SAN + PGN;
- checkers: its conventional capture notation;
- a future custom tactics ruleset: a compact formatter such as `Knight e4; Fireball -> #27`;
- an editor/debugger: a generic expanded rendering of the recorded choices.

A pretty notation adapter may parse text only by resolving it back into choices offered by the authoritative ruleset. It never becomes a second game engine.

## 8. Generic expanded text rendering

For debugging and unsupported rulesets, the engine may expose a deterministic expanded representation without claiming that it is the preferred player-facing notation.

For example, conceptually:

```text
turn player=1
  select_entity entity=12
  select_position position=(4,3) actor=12
  select_ability ability=7 actor=12
  select_entity entity=27
  select_option key="heavy"
  finish_turn
```

The exact serialized grammar should be chosen when Phase 17 is implemented. The semantic model above is the contract; punctuation is not.

## 9. Relationship to existing history

Each existing history layer keeps a different purpose:

- `TurnRecord.decisions` — what authoritative choices the actor made; replay input;
- `StepRecord.action` — semantic action description emitted by the ruleset; diagnostics/notation support;
- `StepRecord.delta` — structural before/after change trace; animation/inspection;
- `StepRecord.before/after` and `TurnRecord.before/after` — snapshots; history queries, undo/redo, rewind mechanics.

These are complementary rather than interchangeable.

## 10. Required proof cases

The generic record/replay implementation is not complete until it round-trips at least:

1. a normal chess move;
2. chess castling;
3. chess promotion with an explicit option choice;
4. a test entity move followed by an ability in the same turn;
5. an ability with an entity target and option;
6. a forced multi-capture/continuation chain;
7. a history-dependent rewind/copy action;
8. a control change where `controller` changes but `owner` does not;
9. a turn in a game with at least three players;
10. undo/redo after replay producing the same authoritative states as the original game.

## 11. Non-goals

The generic notation must not:

- hardcode chess piece names or chess coordinates into core;
- add a central enum containing every possible future action type;
- encode authoritative gameplay as frontend commands;
- depend on presentation labels or asset names;
- use ephemeral `ChoiceId` values as persistent identifiers;
- replay by applying `StateDelta` directly;
- require every ruleset to invent a custom parser before games can be persisted.
