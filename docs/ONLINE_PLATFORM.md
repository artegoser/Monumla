# Monumla online platform

Monumla runs SvelteKit and its WebSocket service on the same Node.js HTTP server. Prisma 7 with the better-sqlite3 adapter owns SQLite persistence. The Rust runtime is compiled twice: browser bindings for the analysis board and Node.js bindings for authoritative online games. Computer searches run in separate worker threads.

## Accounts and community

Accounts use case-insensitive unique usernames, private unique email addresses, salted scrypt password hashes, and random 256-bit session tokens. Only SHA-256 token hashes are stored. Sessions expire after 30 days and use HttpOnly, SameSite cookies, with Secure on HTTPS. Login and registration have rate limits. Country selection accepts ISO 3166-1 alpha-2 codes; biographies contain at most 500 characters and render as plain text.

Players can search usernames, send and accept friend requests, remove friends, see online presence, and challenge any player. Challenges expire after ten minutes and can be accepted or declined only by their recipient; senders can cancel. A player can have one active online game. Multiple tabs share that game.

## Rules, clocks, and reconnects

Online moves consist of runtime choice IDs, never client-supplied positions or results. The server validates the authenticated player, active side, game revision, current choice, and remaining time. Each completed turn atomically saves its authoritative snapshot, player-specific replay positions, notation, clock balances, and revision. Partial selections and unfinished multi-step turns are transient: after a server restart, they return to the last committed turn. The original turn clock continues.

Clock classification uses base seconds plus forty increments: under 180 seconds is bullet, under 480 is blitz, under 1500 is rapid, and the remainder is classical. All six rulesets use these categories, with independent rating pools. Clock increments apply once per completed runtime turn. Battleship setup turns also use the selected clock.

WebSockets carry matchmaking, presence, friends, challenges, game moves, draw offers, results, and rating notifications. Reconnecting clients reauthenticate and subscribe to the latest saved state. Clocks continue through disconnects and server downtime; overdue games are adjudicated when the server starts. Queues are cleared on a server restart, and a disconnected player's queue entry is removed when their final tab disconnects.

Battleship uses complete server-only snapshots for persistence. Opponents receive their own perspective, never the enemy fleet or a full runtime snapshot. Active Battleship games cannot be spectated. Stored replay views preserve the information available to each player at each turn.

## Ratings and history

Elo starts at 1500 independently for each ruleset/category pair. The K factor is 40 for the first 30 rated games, then 24, or 16 at ratings of at least 2400. Expected score is `1 / (1 + 10 ** ((opponent - player) / 400))`. Each player's delta is rounded independently. A draw scores one half. A question mark indicates a provisional rating.

Only rated games between two accounts update ratings. Computer and casual games are still retained. Natural outcomes, resignation, agreed draws, and timeouts all finalize a game. Finalization claims an active game and updates both ratings inside one database transaction, preventing duplicate results. Initial and final positions, completed turns, and both players' rating changes remain in history. Finished chess games and live chess games can be downloaded as PGN.

The analysis board at `/analysis` remains a local sandbox: exploring or undoing an analysis position does not create an online match or alter player history.

## Coolify deployment

Use the repository's Dockerfile build pack, expose port **3000**, and set `ORIGIN` to the exact public URL, for example `https://monumla.example.com`, without a trailing slash. Attach persistent storage at **`/app/data`**, writable by UID/GID 1000. Alternatively select the Docker Compose build pack and `compose.yaml`; its named volume is declared there. For a Compose deployment behind Coolify's proxy, remove the host `ports` mapping if port 3000 is already allocated, and select the service's internal port 3000 in Coolify.

The container runs as the unprivileged Node user, applies committed Prisma migrations before starting, serves both HTTP and `/ws` on port 3000, and exposes `/health` for readiness checks. Coolify's reverse proxy must permit WebSocket upgrades; no second public port is required. Use HTTPS for production session cookies.

Keep **one application replica**. The live game service and matchmaking queue are coordinated within one Node process; multiple replicas require shared coordination and a database suitable for concurrent application servers. SQLite and its WAL files must stay on persistent local storage. Preserve the data volume during deployments and take consistent SQLite backups before upgrades. Do not copy only the main database file while the application is writing; stop it first or use SQLite's backup API.

Local Docker deployment:

```sh
docker compose up --build -d
docker compose logs -f monumla
```

The default Compose origin is `http://localhost:3000`. Override it through the deployment environment for a public host. No database service or Redis is required for this single-instance deployment.

## Source and license

The repository is licensed under AGPL-3.0-only. The build publishes an allowlisted source archive at `/source.tar.gz` and the license at `/license`, linked in the footer. The archive contains application and engine source, migrations, lockfiles, tests, and build instructions; credentials, databases, generated bindings, caches, and dependencies are excluded. Rebuild the archive whenever deploying modified source. Earlier published MIT crate releases keep their original license.
