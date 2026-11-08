# Monumla

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Players who want to find opponents, play several strategy games, and track their progress in one account.

## Product Purpose

A Lichess-inspired multiplayer platform spanning chess, Russian and international checkers, Go, Battleship, and tic-tac-toe. Every game is retained in player history.

## Stack

SvelteKit, Prisma ORM, SQLite, and WebSockets. Reuse the Rust/WASM rules and computer engine. Docker deployment on Coolify.

## Capabilities and Constraints

Accounts with country and biography; user search; friends; direct challenges; matchmaking; computer opponents; persistent game histories; separate Elo ratings for every game and time-control category. Multiplayer rules and clocks are authoritative on the server. Live changes use WebSockets. The initial deployment runs one application instance with persistent SQLite storage.

## Brand Commitments

The product is named Monumla. The repository uses GNU AGPL version 3. Lichess is the requested functional reference.

## Evidence on Hand

Six working rulesets and their existing board components, move notation, computer search, and test suites. No production users or statistics are supplied; show real counts and honest empty states.

## Product Principles

- Put playing and finding opponents first.
- Keep every game's rules in the shared runtime.
- Retain games and ratings across reconnects and server restarts.
- Keep profiles and game histories easy to browse.

## Open Decisions

The interface language and visual direction are implementation choices for this iteration. No email provider or email delivery workflow has been requested.
