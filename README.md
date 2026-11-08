# Monumla

Monumla is a multiplayer games club built with SvelteKit, Prisma, SQLite, and WebSockets. Create an account, find opponents across six rulesets, challenge friends, play the computer, and revisit every online game. Ratings are independent for each ruleset and time-control category.

The shared programmable game runtime has a Svelte frontend. Its generic Rust core hosts composable entity, ability, game, interaction, history, and outcome rules; the built-in chess, checkers, Go, Battleship, and tic-tac-toe rulesets compile to WebAssembly for the browser.

## Repository layout

- `crates/monumla-core` — generic state, history, turns, interaction, runtime primitives.
- `crates/monumla-search` — game-independent search and analysis.
- `crates/monumla-games-chess` — standard chess rules built on the generic core.
- `crates/monumla-games-checkers` — Russian 8×8 and international 10×10 checkers, sharing move generation and interaction.
- `crates/monumla-games-go` — playable digital AGA Go ruleset with 9×9/13×13/19×19 boards, superko, scoring review, komi and handicap.
- `crates/monumla-games-battleship` — classic 10×10 fleets, manual/automatic setup, validated rotation, and player observations.
- `crates/monumla-games-tictactoe` — rectangular 3–9 row/column boards with configurable winning-line length.
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

- Node.js 24 LTS
- pnpm
- Rust stable for development (supported minimum: 1.80)
- `wasm-pack` for browser WASM builds

Install `wasm-pack` if necessary:

```bash
cargo install wasm-pack
```

## Development

The toolchain file follows `stable` without pinning a compiler version and installs
Clippy, rustfmt and the WASM target. Older compilers are used only for compatibility
checks; benchmark comparisons record the exact compiler used for both builds.

Install frontend dependencies:

```bash
pnpm install --frozen-lockfile
pnpm db:generate
pnpm db:migrate
```

Run the local Svelte application backed by the shared Rust/WASM runtime:

```bash
pnpm dev
```

`pnpm dev` and `pnpm build` generate Prisma and both browser/server WASM bindings. `pnpm check` builds the browser WASM package before checking Svelte. You can also build it explicitly:

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

The supported minimum is checked with the locked dependencies, including examples,
tests and all features:

```bash
cargo install cargo-msrv --locked
cargo msrv --workspace find -- cargo check --workspace --all-targets --all-features --locked
```

On 2026-10-28, cargo-msrv 0.19.6 found Rust **1.80.1**. Explicit checks also
passed on **1.80.0**, both for the complete native workspace and for
`monumla-wasm --lib --target wasm32-unknown-unknown`. A separate find using
`--ignore-rust-version` reached 1.71.0; bypassing package requirements does not
change the supported minimum. Neither check writes a pinned toolchain file.

The normal workspace suite keeps expensive depth-4 perft cases ignored so the development loop stays fast. Run the slow correctness gate explicitly when changing move generation, history semantics, or special-move execution:

```bash
cargo test --release -p monumla-games-chess perft::tests:: -- --ignored
```

Online platform:

```bash
pnpm test:server
pnpm exec tsc --noEmit -p tsconfig.server.json
```

Frontend:

```bash
pnpm check
pnpm lint
pnpm build
```

## Online play and deployment

See [the online platform guide](docs/ONLINE_PLATFORM.md) for accounts, matchmaking, friends, challenges, clocks, Elo, history, and Coolify configuration. The Dockerfile and `compose.yaml` serve HTTP and WebSockets on port 3000 with persistent SQLite storage at `/app/data`. Set `ORIGIN` to your public URL and run one replica.

```sh
docker compose up --build -d
```

The repository uses [AGPLv3](LICENSE). Builds include corresponding source at `/source.tar.gz`. Previously published MIT versions retain their original license.

## Development plan

See:

- [`docs/CORE_IMPLEMENTATION_PLAN.md`](docs/CORE_IMPLEMENTATION_PLAN.md)
- [`docs/CORE_IMPLEMENTATION_CHECKLIST.md`](docs/CORE_IMPLEMENTATION_CHECKLIST.md)
- [`docs/GENERIC_ACTION_NOTATION.md`](docs/GENERIC_ACTION_NOTATION.md)
- [`docs/GENERIC_OUTCOME_RULES.md`](docs/GENERIC_OUTCOME_RULES.md)
- [`docs/GENERIC_GAME_RULES.md`](docs/GENERIC_GAME_RULES.md)
- [`docs/BUILTIN_RULESETS.md`](docs/BUILTIN_RULESETS.md)

## Search engine

`monumla-search` searches the ordinary core rules for chess, both checkers variants, Go, and tic-tac-toe through one shared runtime path. It owns iterative deepening, alpha-beta/PVS, transposition storage, quiescence, and ranked continuations. Battleship uses the shared observation-sampling algorithm: the worker receives its own fleet and public results, then compares shots against possible enemy fleets. Optional position estimates remain ruleset knowledge; no game implements a search algorithm. The browser offers automatic computer play and a separate Analysis mode with evaluations and arrows. Game crates contain no search adapters. See [`docs/SEARCH_ENGINE.md`](docs/SEARCH_ENGINE.md) for behavior, examples, and limits.
