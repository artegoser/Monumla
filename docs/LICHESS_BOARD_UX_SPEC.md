# Lichess Board UX Parity Specification

## 1. Goal

Monumla should reproduce the interaction model and visual feedback of the standard Lichess game board as closely as practical while keeping Monumla's own implementation and architecture.

This is a behavioral and visual parity target, not a dependency target. The frontend must not depend on Lichess board packages, and no Lichess/Chessground source code or CSS should be copied into Monumla. Rust/WASM remains the sole authority for legal actions and state transitions.

The parity reference for this specification is the current Lichess game-board behavior inspected on 2026-08-10, including the Lichess round-board integration and the board behavior it configures.

## 2. Architectural boundary

The board UI may own only interaction mechanics and presentation state:

- pointer/touch hit testing;
- click selection;
- drag lifecycle;
- hover state;
- transient drag position and ghost rendering;
- animation timing and interpolation;
- board orientation and coordinates;
- purely visual highlights and drawing overlays.

The board UI must not:

- generate chess moves;
- decide whether a destination is legal;
- infer captures from chess geometry;
- implement castling, en passant, promotion, check, or mate rules;
- mutate the authoritative game position locally and then "tell" Rust what happened.

Legal origins, destinations, options, and final state transitions come from Rust/WASM. The frontend maps those Rust-issued choices into Lichess-compatible board affordances.

## 3. Input parity

### 3.1 Click-to-move

Default desktop behavior must support Lichess-style click selection in addition to drag-and-drop.

Required behavior:

1. clicking a controllable piece selects it;
2. selecting a piece immediately shows its Rust-provided legal destinations;
3. clicking a legal destination submits the corresponding opaque `ChoiceId`;
4. clicking another controllable piece switches selection to that piece;
5. clicking an invalid square clears selection when Lichess would clear it;
6. clicking the selected square again clears selection where applicable;
7. selection must be refreshed after every authoritative state transition.

### 3.2 Drag-and-drop

Dragging must be a first-class move input path rather than emulated click-to-move.

Required behavior:

- left mouse button and one-finger touch can initiate drag;
- use a small movement threshold before a click becomes a drag; target parity is the Lichess desktop threshold of approximately 3 CSS pixels;
- the piece follows the pointer continuously while dragging;
- the dragged piece renders above normal pieces and animations;
- optionally show a translucent ghost at the origin; default behavior should match Lichess highlighting-enabled play;
- legal destinations remain visible during drag;
- hovering a legal destination changes its destination marker to the Lichess-style full-square hover state both during drag and ordinary pointer hover;
- dropping on a legal destination submits the Rust-issued destination `ChoiceId`;
- dropping on the origin or outside/onto an illegal destination does not mutate the game state;
- drag cancellation restores the authoritative board without a synthetic move;
- drag must use the same Rust-issued legal choice set as click-to-move.

### 3.3 Touch behavior

- support one-finger piece dragging;
- avoid creating duplicate mouse actions after touch interaction;
- do not globally block page scrolling unless the interaction began on/near an interactable board piece;
- tap-tap movement remains usable on touch devices;
- touch and mouse paths must resolve to the same opaque Rust choice IDs.

## 4. Legal destination rendering

Lichess distinguishes empty legal destinations from occupied legal capture destinations. Monumla must do the same.

### 4.1 Quiet destination

An empty legal target renders as a small centered circular destination marker, visually matching Lichess's dark/green radial dot behavior rather than a generic opaque dot.

### 4.2 Capture destination

A legal target containing a capturable piece must render as four triangular edge/corner wedges around the occupied square/piece, not as the same centered marker used for empty squares.

This distinction must be derived from the current authoritative `GameView` plus the Rust-issued destination choice, not from chess-specific frontend logic.

### 4.3 Destination hover

Hovering a legal destination must replace/strengthen the normal marker with a full-square translucent highlight matching Lichess interaction feedback.

## 5. Square state rendering

The board must support simultaneously composable square presentation states.

Required states:

- `selected` — currently selected origin;
- `last-move` — origin and destination of the most recently committed move/turn step relevant to board presentation;
- `check` — checked king square;
- `move-dest` — quiet legal target;
- `move-dest capture` — occupied legal capture target rendered with four triangular wedges;
- `move-dest hover` — hovered legal target;
- `current-premove` when premove support becomes meaningful;
- custom presentation overlays emitted by the runtime without breaking the standard states.

When multiple states affect the same square, layering must be deterministic and visually match Lichess priorities.

## 6. Last move and check

### Last move

After every committed chess move, highlight both move endpoints in the Lichess style.

Special cases:

- castling should visually identify the king's effective origin/destination consistently with normal Lichess board feedback;
- promotion should preserve the move endpoints after the promoted entity type appears;
- undo/redo/history navigation must recompute last-move highlighting from authoritative history.

### Check

When the current side is in check, the king square must receive the Lichess-style red radial check highlight.

The frontend receives the checked entity/square from Rust/WASM or from a presentation field derived by the Rust chess layer. It must not compute attacks locally.

## 7. Piece movement animation

Normal state transitions should animate similarly to Lichess rather than teleporting pieces.

Required behavior:

- interpolate a moved piece from previous square to resulting square;
- default animation duration should target approximately 200 ms unless a later user preference overrides it;
- start movement with an explicit browser animation after the authoritative DOM has rendered; do not depend on a CSS-transition/paint race to establish the initial offset;
- a transition either animates for its full configured duration or is intentionally skipped (for reduced motion); it must not randomly teleport because the browser missed an intermediate style;
- use a smooth ease curve with the same perceptual character as Lichess;
- captured pieces fade/remove while the moving piece travels to the target;
- castling animates king and rook as two concurrent moved entities from one authoritative transition;
- en passant moves the capturing pawn while separately removing the captured pawn;
- promotion moves the pawn to the promotion square and then changes/replaces its presentation without a visual teleport;
- undo/redo/history navigation should animate when the navigation mode requests normal board animation;
- active move animation must not fight the dragged piece transform.

Animation plans should be produced from Monumla `StateDelta`/before-after board state, not from duplicated chess rules.

## 8. Piece drag presentation

During drag:

- the dragged piece is rendered above all ordinary pieces;
- normal move animation for that piece is cancelled/suspended;
- pointer position maps directly to the piece center;
- origin ghost is semi-transparent when enabled;
- the original authoritative piece identity is preserved until Rust accepts a destination;
- no speculative authoritative state is committed in JS/Svelte.

## 9. Promotion UX

Promotion must use Rust-provided promotion choices and present them as a board-local selection UI rather than generic text buttons below the board.

Target behavior:

- promotion choices appear attached to/over the promotion file or destination area;
- show piece graphics for queen, rook, bishop, and knight;
- selecting a piece submits its Rust-issued option `ChoiceId`;
- cancellation behavior must be explicit and must not fabricate a move;
- underpromotion must be equally accessible.

## 10. Board geometry and orientation

- board remains exactly square for standard chess;
- keep subtle rounded outer board corners while preserving exact square geometry and clipping overlays/pieces to the board; the radius must stay small enough that coordinate labels are not visibly clipped;
- pieces fill each square at Lichess-like visual scale;
- coordinates match board orientation;
- support white and black orientation;
- orientation flips must not mutate game state;
- dragging/hit testing/highlights must work identically in both orientations;
- responsive resizing must preserve exact square hit boxes and piece transforms.

## 11. Board appearance parity

Phase 13 should move the current board from "Lichess-inspired" to deliberate Lichess parity for the default standard board presentation.

Acceptance target:

- default brown-board light/dark relationship matches Lichess visually;
- destination dot, capture wedges, selected square, last-move square, hover, check, ghost, and piece motion all match Lichess closely enough that side-by-side comparison reveals no obvious behavioral mismatch;
- remove current Monumla-only board affordances that visibly diverge from Lichess unless they are required for generic future game modes;
- generic future-game affordances must remain dormant for standard chess rather than changing standard chess UX.

Do not copy upstream CSS/source verbatim. Recreate the rendered result in Monumla's own Svelte/CSS implementation.

## 12. Drawings and secondary board affordances

Lichess-style right-click board annotations are part of the eventual board parity target:

- right-drag arrow drawing;
- right-click square highlighting;
- multiple annotation colors/modifiers;
- annotations are presentation-only and never enter authoritative game state;
- starting a normal movable-piece interaction clears annotations according to Lichess-like behavior.

These affordances may be implemented after the core Phase 13 move/drag/highlight parity if they would otherwise block chess-runtime correctness work.

## 13. Premove scope

Premove is part of Lichess board UX but is not meaningful in the current instant local two-player loop because the next side becomes active immediately.

The board interaction layer must therefore be designed so premove can be added without restructuring drag/selection state, but actual premove execution is deferred until there is a mode with a meaningful opponent-turn waiting period (for example multiplayer or an asynchronous engine opponent).

No frontend chess move generator may be introduced for premoves. Future premove targets must be supplied/validated through a dedicated runtime contract appropriate to the active mode.

## 14. Accessibility and interaction quality

- keyboard/focus behavior must not regress while pointer UX is improved;
- buttons/squares require meaningful accessible labels even if the visual implementation no longer uses literal buttons for every square;
- reduced-motion preference should allow disabling/simplifying movement animation;
- interaction must not rely only on color to distinguish selected/check states where accessibility metadata can provide additional meaning.

## 15. Phase 13 acceptance test

Phase 13 is not complete until all of the following are true:

1. ordinary moves work by both click-click and drag-drop;
2. quiet and capture destinations are visually distinct exactly in the Lichess interaction style;
3. selected, last-move, hover, and check states are present;
4. drag has threshold, free pointer-follow, z-order, cancellation, and ghost behavior;
5. move/capture/castling/en-passant/promotion transitions animate without chess logic in Svelte;
6. promotion uses board-local graphical piece choices;
7. white/black orientation and responsive sizing preserve hit testing;
8. a complete local game remains Rust-authoritative;
9. side-by-side manual comparison against the current Lichess board shows no obvious mismatch in the standard move interaction loop.
