# Generic Game Rules

## Purpose

Monumla separates mechanics by ownership instead of collecting every rule in one ruleset switch.

- `EntityRule` / a ruleset-specific entity extension owns mechanics intrinsic to one entity type.
- `AbilityRule` owns mechanics intrinsic to one explicit ability.
- `GameRule` owns match, map, variant, or environment mechanics that apply across entities.
- `OutcomeRule` owns terminal game semantics.

A standard chess pawn therefore owns promotion. A chess variant that removes queen promotion does not redefine the pawn; it installs a `GameRule` that transforms the pawn's generated continuation choices.

## Core composition

`monumla-core` exposes a clonable `GameRuleSet`. Rules execute in registration order.

The choice pipeline is:

```text
entity / ability / local interaction choices
        ↓
GameRule 1: add global choices
        ↓
GameRule 1: transform/filter combined choices
        ↓
GameRule 2: add global choices
        ↓
GameRule 2: transform/filter combined choices
        ↓
frontend Interaction
```

For a forced local continuation, unrelated global choices are deliberately not added:

```text
required entity/ability continuation
        ↓
GameRule transforms/constraints only
        ↓
frontend Interaction
```

This prevents a forced capture, promotion, target selection, or similar continuation from accidentally exposing unrelated top-level actions.

A `GameRule` may also handle a choice it introduced or intentionally intercepted. The first registered rule returning a handled `InteractionFlow` owns that choice. Local interaction handling remains the fallback.

For piece-local continuations, `transform_choices` is primarily a constraint/composition stage. A transform may filter or relabel a local choice without changing its semantic `ChoiceInput`. If a global rule replaces the semantic payload itself, that rule must also own how the replacement choice is executed rather than relying on the original entity rule to understand foreign input.

## Validation

Every registered `GameRule` can validate the current `RuleContext`. Validation runs before game-rule choice resolution and before authoritative chess move execution.

Global mechanics must not be copied into every entity implementation merely to make them enforceable. Examples that belong to `GameRule` include:

- a terrain rule that damages any unit entering lava;
- a variant that filters a class of upgrade choices;
- a map rule that grants an action while an objective is controlled;
- a turn-count modifier that changes a global restriction;
- a silence rule that removes ability choices.

Post-step environment reactions use `GameRule::react(before, action, transaction)`. The hook runs inside the same authoritative transaction after local action mechanics have produced their working state. Rules can compare the pre-step snapshot with `transaction.state()` and mutate through the ordinary open transaction API. No closed `Effect` enum is introduced.

## Piece-local move continuations

Ruleset-specific entity traits may expose generic `ChoiceSpec` continuations for a selected action. Standard chess uses this for promotion:

```text
Pawn move to final rank
        ↓
Pawn::move_choices
        ↓
SelectOption(queen / rook / bishop / knight)
        ↓
GameRuleSet transforms choices
        ↓
ephemeral-free `ChoiceInput { kind, data }` passed opaquely to `execute_move`
        ↓
Pawn::validate_move_input
        ↓
geometric move
        ↓
Pawn::apply_move_input changes EntityTypeId
```

The chess interaction coordinator does not know:

- what a promotion rank is;
- that pawns promote;
- which entity types are promotion targets;
- how the selected option mutates the pawn.

It only knows that a selected move may require one or more generic continuation choices.

## Standard chess promotion

`Pawn` owns all standard promotion semantics:

- whether a move requires a continuation;
- the four standard target types;
- option labels and presentation asset keys;
- validation of the selected semantic choice input;
- the final entity-type mutation.

The move executor accepts an opaque `ChoiceInput` (`ChoiceKind` + `StateMap`) rather than a promotion-specific argument. `ChoiceId`, labels, and presentation asset keys are intentionally excluded. SAN/PGN remain chess-specific human notation adapters; they resolve a requested promotion through the currently legal generated continuation choices before executing it.

This is important for composition: a ruleset-level modifier can filter pawn-owned promotion choices without adding a pawn branch to `ChessRules` or `ChessInteractionRules`.

## Ownership rule

Use this test when deciding where a mechanic belongs:

> If the mechanic should travel with the entity when that entity type is reused elsewhere, it belongs to the entity rule. If the mechanic exists because the current match, variant, map, or environment imposes it on entities, it belongs to a game rule.

Examples:

| Mechanic | Owner |
| --- | --- |
| Pawn reaches final rank and must transform | `Pawn` entity rule |
| Pawn standard targets are Q/R/B/N | `Pawn` entity rule |
| Variant forbids queen promotion | `GameRule` |
| Every unit entering center receives an upgrade choice | `GameRule` |
| Knight movement geometry | `Knight` entity rule |
| Lava damages every entity entering a tile | `GameRule` |
| Fireball targeting/damage | Fireball `AbilityRule` |
| Checkmate / stalemate / repetition terminal semantics | `OutcomeRule` |

## Invariants

1. `monumla-core` contains no promotion, pawn, chess-piece, terrain, spell, or similar mechanic-specific enum variant.
2. `ChessInteractionRules` coordinates interaction; it does not own piece mechanics.
3. Global rules are composable and ordered rather than represented by one monolithic `ChessRules` branch table.
4. Required local continuations suppress unrelated global choices while still allowing global constraints to transform them.
5. A global rule that filters every required continuation makes the initiating action unavailable.
6. Programmatic execution, perft, and notation paths must resolve ruleset-modified continuations through the same authoritative rule choices rather than bypassing them.
7. State mutation remains transactional and open-ended; do not replace this architecture with a closed universal effect enum.
