# Monumla

Monumla is a local-first programmable game runtime with a Svelte frontend. Its generic Rust core hosts composable entity, ability, game, interaction, history, and outcome rules; the built-in chess, checkers, and Go rulesets compile to WebAssembly for the browser.

## Repository layout

- `crates/monumla-core` — generic state, history, turns, interaction, and runtime primitives.
- `crates/monumla-games-chess` — standard chess rules built on the generic core.
- `crates/monumla-games-checkers` — playable compact checkers ruleset.
- `crates/monumla-games-go` — playable digital AGA Go ruleset with 9×9/13×13/19×19 boards, superko, scoring review, komi and handicap.
- `crates/monumla-wasm` — browser bindings for the Rust runtime.
- `src` — SvelteKit presentation and input layer.
- `docs/CORE_IMPLEMENTATION_PLAN.md` — architecture and implementation plan.
- `docs/CORE_IMPLEMENTATION_CHECKLIST.md` — implementation checklist.
- `docs/LICHESS_BOARD_UX_SPEC.md` — Lichess board interaction and visual parity specification.
- `docs/GENERIC_ACTION_NOTATION.md` — ruleset-agnostic action recording and deterministic replay specification.
- `docs/GENERIC_OUTCOME_RULES.md` — ruleset-wide terminal outcome contract and precedence rules.
- `docs/GENERIC_GAME_RULES.md` — composition contract for entity-local and ruleset-wide mechanics.
- `docs/BUILTIN_RULESETS.md` — Phase 16 built-in rulesets and shared-runtime coverage.
- `docs/GO_RULES_AUDIT.md` — digital AGA Go rules/scoring coverage and explicit session-layer boundaries.

## Requirements

- Node.js 22+
- pnpm
- Rust 1.80+
- `wasm-pack` for browser WASM builds

Install `wasm-pack` if necessary:

```bash
cargo install wasm-pack
```

## Development

Install frontend dependencies:

```bash
pnpm install --frozen-lockfile
```

Run the local Svelte application backed by the shared Rust/WASM runtime:

```bash
pnpm dev
```

`pnpm dev`, `pnpm build`, and `pnpm check` build the browser WASM package first. You can also build it explicitly:

```bash
pnpm wasm:build
```

The generated bindings are written to `src/lib/wasm/pkg/` and are not committed.

## Verification

Rust:

```bash
cargo test --workspace
cargo check --workspace
cargo clippy --workspace --all-targets --all-features
```

The normal workspace suite keeps expensive depth-4 perft cases ignored so the development loop stays fast. Run the slow correctness gate explicitly when changing move generation, history semantics, or special-move execution:

```bash
cargo test --release -p monumla-games-chess perft::tests:: -- --ignored
```

Frontend:

```bash
pnpm check
pnpm lint
pnpm build
```

## Development plan

See:

- [`docs/CORE_IMPLEMENTATION_PLAN.md`](docs/CORE_IMPLEMENTATION_PLAN.md)
- [`docs/CORE_IMPLEMENTATION_CHECKLIST.md`](docs/CORE_IMPLEMENTATION_CHECKLIST.md)
- [`docs/GENERIC_ACTION_NOTATION.md`](docs/GENERIC_ACTION_NOTATION.md)
- [`docs/GENERIC_OUTCOME_RULES.md`](docs/GENERIC_OUTCOME_RULES.md)
- [`docs/GENERIC_GAME_RULES.md`](docs/GENERIC_GAME_RULES.md)
- [`docs/BUILTIN_RULESETS.md`](docs/BUILTIN_RULESETS.md)
