# Standard Chess Rules Audit

## Scope

This audit covers the authoritative standard-chess rules implemented by `monumla-games-chess` and the chess-only FEN/SAN/PGN and browser-runtime adapters that expose those rules.

The target is ordinary over-the-board standard chess position and game semantics: legal piece movement, king safety, special moves, terminal positions, FIDE repetition and move-count draws, resignation/agreement recording, and interoperable notation/position formats.

Physical tournament procedure is intentionally outside the board-rules runtime. Monumla does not model a physical touch-move violation, clock-button handling, flag fall, arbiter penalties, scoresheet possession, or other venue/competition procedure. Those belong to a future clock/tournament/session layer rather than piece or board legality.

## Audited rule coverage

### Movement and legality

The existing move generator remains authoritative and is covered by reference perft positions plus focused regressions for:

- pawn single and double advances;
- pawn captures and attack/move separation;
- en passant derived from the immediately preceding committed chess move;
- en passant that is illegal because it exposes the moving king;
- knight movement;
- sliding-piece blocking and captures;
- king attack maps and king-safety filtering;
- pins, discovered attacks, double check, and illegal king captures;
- king-side and queen-side castling;
- castling rights derived from king/rook piece-local `has_moved` state;
- attacked castling transit squares;
- promotion and underpromotion, including capture-promotion.

No chess-specific movement state is stored in `monumla-core`.

### Terminal positions and draw rules

`ChessRules::status` covers:

- checkmate;
- stalemate;
- automatic fivefold repetition;
- automatic seventy-five-move draw, with checkmate evaluated first;
- sound material-dead classes currently recognized by the runtime;
- persistent explicit terminal outcomes such as resignation, agreement, accepted draw claims, and PGN-declared results.

Claimable draws cover both FIDE forms:

- a position that has already appeared for the third time;
- an indicated legal next move that would create the third appearance, without committing that move;
- fifty completed moves by each player without a pawn move or capture;
- an indicated legal next move that would complete the fiftieth move by each player, again without committing that move.

A draw by agreement is not recordable before both players have made at least one move. Chess terminal actions are committed as normal timeline turns, so local undo/redo restores the exact pre-terminal game instead of maintaining a second UI-only outcome state.

### Repetition equivalence

The repetition key includes:

- side to move;
- piece type, colour/owner, and square;
- effective castling rights;
- only an en-passant possibility that is actually legal in the position.

Terminal metadata turns do not become extra repetition occurrences and do not hide the last real chess move from repetition or en-passant reconstruction.

## FEN

FEN parsing and serialization are Rust-authoritative.

Import validates the six standard fields and additionally rejects structurally impossible standard positions such as:

- more than 32 pieces globally or more than 16 pieces for one side;
- more than eight pawns for one side;
- pawns on the first or eighth rank;
- promoted material that cannot be accounted for by missing pawns;
- a position where the side that just moved has left its own king attacked;
- castling rights without the required king/rook placement;
- inconsistent en-passant predecessor information.

Castling rights are reconstructed into king/rook movement state. An en-passant target reconstructs only the minimal synthetic previous pawn move needed for normal history-derived en-passant logic. FEN cannot reconstruct repetition history that the format does not contain, and Monumla does not fabricate it.

Terminal outcome metadata is deliberately ignored when determining the FEN en-passant field, because resignation/agreement/claim recording does not alter the board position or erase the immediately preceding pawn double advance.

## SAN and PGN

SAN generation covers ordinary moves, captures, disambiguation, castling, promotion/underpromotion, check, and mate.

SAN import is deliberately tolerant of common presentation annotations such as `!?`, numeric annotation glyphs, optional `+`/`#` suffixes, zero-form castling, and `e.p.` while still resolving the resulting token through the authoritative legal move generator.

PGN main-line import/export now preserves game-result semantics:

- `Result` tag and movetext result are checked for conflicts;
- a declared terminal result on an otherwise non-terminal board is stored as a terminal history action rather than inventing a board move;
- a result that conflicts with a naturally checkmated/drawn final board is rejected;
- an explicit `*` after a naturally finished game is rejected;
- exported result is recomputed from authoritative final state;
- standard source tags and additional source tags are preserved where possible;
- comments, NAGs, and side variations may be consumed while replaying only the main line.

SAN/PGN are chess-specific adapters. They are not the future ruleset-agnostic Monumla replay format.

## Explicit limitations

### Exact arbitrary dead-position reachability

FIDE defines a dead position as one where neither player can checkmate by any series of legal moves. The current hot-path detector intentionally recognizes sound standard material-dead classes such as king versus king, king plus a single bishop/knight versus king, and bishop-only positions where every bishop is confined to the same colour complex.

It does **not** claim to solve every constructed dead position involving blocked pawns or other material. Exact recognition of arbitrary such positions is a reachability problem and remains an explicit deferred item rather than being approximated with an unsound heuristic.

### Full retrograde legality of arbitrary FEN positions

The importer performs strict structural and immediate king-legality checks, but it does not attempt an exhaustive retrograde proof that every accepted constructed FEN is reachable from the standard initial position. Arbitrary retrograde reachability is intentionally distinct from normal forward move legality.

### Historical information absent from FEN

A standalone FEN contains no full move history. Therefore a position loaded from FEN cannot acquire pre-import repetition occurrences or other facts that are not encoded by FEN. Only the minimum castling/en-passant continuation semantics are reconstructed.

### Tournament procedure

Clock expiry, touch-move enforcement, illegal-move penalties, arbiter decisions, draw-offer communication protocol, and other physical/competition procedure are session/tournament concerns and are not represented as board-legality rules in this phase. `agree_draw` records a mutual agreement that has already occurred; it is not itself a network draw-offer handshake.
