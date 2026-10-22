# Go Rules Coverage Audit

## Scope

`monumla-games-go` implements a digital American Go Association (AGA) ruleset. The default game is 19×19 with territory counting and 7.5 komi. The browser may also start 9×9 and 13×13 even games, switch between AGA territory and area counting, and start the standard 19×19 handicaps.

Rules references:

- AGA official rules mirror maintained by the British Go Association: <https://www.britgo.org/rules/agarules.html>
- Current AGA Congress rules summary: <https://www.usgo.org/content.aspx?club_id=454497&module_id=563542&page_id=22>

The ruleset deliberately models game rules, not over-the-board tournament administration. Clock handling, rank-based pairing/handicap assignment, accidental illegal-move penalties, and referee procedure belong to a later session/tournament layer.

## Implemented play rules

- Black plays first in even and one-stone-handicap games.
- White plays first after fixed handicap setup stones.
- Placement is on an empty intersection, including edges and corners.
- Orthogonal connectivity, strings/groups, and liberties are derived from the current board.
- Opposing groups with no liberties after placement are captured transactionally.
- Self-capture is illegal after opposing captures are resolved.
- Captured stones become prisoners of the capturing player.
- Passing is always available and transfers a pass stone to the opponent as a prisoner.
- Resignation is an authoritative terminal turn and participates in undo/redo.

## Repetition

AGA repeated-position legality is implemented as situational superko: a placement may not recreate an earlier full-board stone position with the same player to move.

The superko set is derived from committed history. It is not stored as a mutable ko flag. Passes remain legal even when the board position is unchanged. Undo/redo therefore changes superko legality through the authoritative timeline rather than through repair logic.

## Ending and scoring review

Two consecutive passes enter scoring review instead of immediately inventing a numeric result.

During review:

- each player can mark connected groups as dead/alive using an independent assessment;
- matching assessments finalize the agreed dead groups;
- agreed dead stones are removed and become prisoners;
- differing assessments leave the game in a scoring dispute;
- either side may resume ordinary play to resolve a disputed status;
- if resumed play ends immediately with the required consecutive passes, remaining stones are treated as alive;
- White makes an additional final pass when required so the AGA equal-move/pass-stone convention is preserved.

The review UI is a digital agreement protocol. It does not pretend that clicking a dead-group marker is an ordinary stone play. All actual placements and passes remain authoritative timeline turns.

## Counting

The scoring method is selected before the game.

### Territory counting

- territory is each empty connected region bordered only by one colour;
- neutral regions score for neither side;
- each player's AGA territory score is that player's territory minus prisoners held by the opponent;
- prisoners include ordinary captures, agreed dead stones, and pass stones;
- komi is added to White.

### Area counting

- area is live stones plus surrounded territory;
- prisoners do not score;
- komi is added to White;
- in handicap games White receives the AGA area-counting compensation of one extra point for every handicap stone after the first.

All score arithmetic uses integer half-points. Standard 7.5/0.5 komi therefore does not rely on floating-point arithmetic.

## Komi and handicap

Current built-in defaults follow current AGA tournament practice:

- even game: 7.5 komi to White;
- handicap game: 0.5 komi to White;
- one-stone handicap has no setup stone and Black still moves first;
- fixed 2–9 stone handicap setup uses the traditional 19×19 AGA star-point order;
- area counting adds `handicap - 1` points to White so area and territory counting preserve the same result.

Fixed multi-stone handicap placement on smaller boards is intentionally not invented because the AGA rules do not standardize it. The UI still supports even and one-stone-handicap 9×9/13×13 games.

## Coordinates and presentation

Go coordinates use letters from A through T while skipping I, with numeric ranks from Black's side. The Svelte board renders Go as intersections of grid lines rather than as chess-style square centres and includes the conventional hoshi points for 9×9, 13×13, and 19×19.

Stone hover previews, dead/disputed review presentation, prisoners, komi, scoring method, and final score are presentation derived from authoritative Rust state. TypeScript does not implement legality or scoring.

## Deliberate non-ruleset boundaries

The following are not hidden rule simplifications:

- **Illegal-move tournament penalty:** the digital runtime simply does not offer illegal placements. The AGA over-the-board procedure that replaces an accidentally played illegal move with a pass belongs to tournament/session administration.
- **Time controls and timeout:** not part of `monumla-games-go`; a future session layer should own clocks and timeout outcomes.
- **Rank calculation and automatic handicap assignment:** matchmaking/tournament policy, not board rules.
- **SGF:** game-record interchange is not yet implemented. This is a format capability, not a legality/scoring shortcut.
- **Non-standard handicap layouts on smaller boards:** intentionally unsupported until a concrete variant defines them.

## Architectural result

Full AGA Go still requires no Go-specific variants in `monumla-core`:

```text
SelectPosition -> spawn stone -> group capture -> history-derived superko
SelectOption(pass) -> pass stone -> scoring review
SelectEntity(group) -> review metadata
SelectOption(resume) -> ordinary play
OutcomeRule -> score/resignation result
```

The ruleset validates that Monumla can model placement games, group topology, whole-history legality, multi-phase endgame agreement, and two scoring systems through the existing generic state/history/interaction primitives.
