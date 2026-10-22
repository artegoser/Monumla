# Generic Outcome Rules

## Purpose

Game completion is a ruleset-wide concern. Entity rules own local mechanics and semantic state; they do not decide whether the whole game is terminal.

The generic core therefore exposes `OutcomeRule` and `GameOutcome` rather than putting win/loss behavior into `EntityRule` or adding mechanic-specific variants to core.

## Separation of responsibility

### Entity rules

Entity rules may expose local facts that outcome logic can inspect, for example:

- HP or mana;
- a `royal`/`objective` tag;
- ownership/controller information;
- entity-local counters;
- movement/attack/ability semantics.

They do not declare the game finished.

### Outcome rules

An `OutcomeRule` evaluates the complete `RuleContext`, including current state and committed history when available, and returns either:

- `None` when its terminal condition is not satisfied; or
- one ruleset-defined `GameOutcome`.

`GameOutcome` is intentionally generic. It carries:

- a stable ruleset-defined reason key;
- winning players;
- losing players;
- winning teams;
- losing teams;
- extensible ruleset-specific data.

## Registry precedence

`RuleRegistry` stores outcome rules in registration order. Evaluation stops at the first rule that returns an outcome.

This makes precedence explicit. For example, a chess ruleset can ensure checkmate is considered before an automatic seventy-five-move draw without teaching generic core anything about either rule.

## Chess adapter

`ChessOutcomeRule` adapts the existing standard-chess status evaluator to the generic outcome contract.

Examples:

- checkmate -> `chess.checkmate` with winner/loser players;
- stalemate -> `chess.stalemate`;
- resignation -> `chess.resignation` with winner/loser players;
- repetition/dead-position/move-count draws -> stable draw reason keys.

Chess-specific APIs such as draw claims remain in `monumla-games-chess`; the generic layer only receives the resulting terminal outcome.

## Future rulesets

The same contract can represent rules such as:

- eliminate all opponent objectives;
- reach a target square;
- reduce a royal entity to zero HP;
- reach a score threshold;
- survive a turn limit;
- control a set of zones;
- last team standing.

A ruleset may inspect any entity semantics it defines, but the decision that the game is terminal remains outside the entity implementation.

## Non-goals

`OutcomeRule` is not a replacement for:

- entity death/removal;
- player elimination in a game that continues with other players;
- turn continuation rules;
- interaction choices;
- history/replay;
- human-readable notation.

Those remain separate runtime concepts.
