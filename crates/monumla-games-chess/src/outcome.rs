use crate::{
    piece::has_moved, pieces::pawn::en_passant_moves_from_previous, ChessError, ChessPieceContext,
    ChessRules, ChessSide, BISHOP, KING, KNIGHT, PAWN, QUEEN, ROOK,
};
use monumla_core::{
    EntityTypeId, GameOutcome, GameState, History, OutcomeRule, PlayerId, Position, RuleContext,
    RuleError, StateValue, TurnRecord,
};
use serde::{Deserialize, Serialize};

const HALF_MOVE_CLOCK: &str = "chess.halfmove_clock";
pub(crate) const FULL_MOVE_NUMBER: &str = "chess.fullmove_number";
const EXPLICIT_OUTCOME: &str = "chess.outcome";
const EXPLICIT_WINNER: &str = "chess.outcome_winner";
const EXPLICIT_LOSER: &str = "chess.outcome_loser";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChessDrawClaim {
    ThreefoldRepetition,
    FiftyMoveRule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChessOutcome {
    Checkmate { winner: PlayerId, loser: PlayerId },
    Stalemate,
    Resignation { winner: PlayerId, loser: PlayerId },
    DrawAgreement,
    ThreefoldRepetition,
    FivefoldRepetition,
    FiftyMoveRule,
    SeventyFiveMoveRule,
    DeadPosition,
    RecordedWin { winner: PlayerId, loser: PlayerId },
    RecordedDraw,
}

#[derive(Clone)]
pub struct ChessOutcomeRule {
    rules: ChessRules,
}

impl ChessOutcomeRule {
    pub fn new(rules: ChessRules) -> Self {
        Self { rules }
    }
}

impl OutcomeRule for ChessOutcomeRule {
    fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        let empty_history = History::default();
        let history = context.history().unwrap_or(&empty_history);
        let status = self
            .rules
            .status(context.state(), history)
            .map_err(|error| RuleError::Rejected(error.to_string()))?;
        Ok(status.outcome.as_ref().map(ChessOutcome::as_game_outcome))
    }
}

impl ChessOutcome {
    pub fn as_game_outcome(&self) -> GameOutcome {
        match self {
            ChessOutcome::Checkmate { winner, loser } => GameOutcome::new("chess.checkmate")
                .with_winner(*winner)
                .with_loser(*loser),
            ChessOutcome::Stalemate => GameOutcome::new("chess.stalemate"),
            ChessOutcome::Resignation { winner, loser } => GameOutcome::new("chess.resignation")
                .with_winner(*winner)
                .with_loser(*loser),
            ChessOutcome::DrawAgreement => GameOutcome::new("chess.draw_agreement"),
            ChessOutcome::ThreefoldRepetition => GameOutcome::new("chess.threefold_repetition"),
            ChessOutcome::FivefoldRepetition => GameOutcome::new("chess.fivefold_repetition"),
            ChessOutcome::FiftyMoveRule => GameOutcome::new("chess.fifty_move_rule"),
            ChessOutcome::SeventyFiveMoveRule => GameOutcome::new("chess.seventy_five_move_rule"),
            ChessOutcome::DeadPosition => GameOutcome::new("chess.dead_position"),
            ChessOutcome::RecordedWin { winner, loser } => GameOutcome::new("chess.recorded_win")
                .with_winner(*winner)
                .with_loser(*loser),
            ChessOutcome::RecordedDraw => GameOutcome::new("chess.recorded_draw"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChessStatus {
    pub side_to_move: ChessSide,
    pub in_check: bool,
    pub outcome: Option<ChessOutcome>,
    pub repetition_count: usize,
    pub halfmove_clock: u16,
    pub can_agree_draw: bool,
    pub can_claim_threefold_repetition: bool,
    pub can_claim_fifty_move_rule: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct PositionPieceKey {
    position: Position,
    entity_type: EntityTypeId,
    owner: PlayerId,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PositionKey {
    pieces: Vec<PositionPieceKey>,
    side_to_move: PlayerId,
    castling_rights: u8,
    en_passant_targets: Vec<Position>,
}

impl ChessRules {
    pub fn side_to_move(&self, state: &GameState) -> Result<ChessSide, ChessError> {
        let [player] = state.turn.active_players.as_slice() else {
            return Err(ChessError::InvalidTurnState);
        };
        ChessSide::from_player(*player).ok_or(ChessError::UnknownSide(*player))
    }

    pub fn halfmove_clock(&self, state: &GameState) -> u16 {
        state
            .ruleset_state
            .get(HALF_MOVE_CLOCK)
            .and_then(StateValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0)
    }

    pub fn set_halfmove_clock(&self, state: &mut GameState, value: u16) {
        state
            .ruleset_state
            .insert(HALF_MOVE_CLOCK, u64::from(value));
    }

    pub fn fullmove_number(&self, state: &GameState) -> u32 {
        state
            .ruleset_state
            .get(FULL_MOVE_NUMBER)
            .and_then(StateValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
            .unwrap_or(1)
    }

    pub fn set_fullmove_number(&self, state: &mut GameState, value: u32) {
        state
            .ruleset_state
            .insert(FULL_MOVE_NUMBER, u64::from(value.max(1)));
    }

    pub fn status(&self, state: &GameState, history: &History) -> Result<ChessStatus, ChessError> {
        let side = self.side_to_move(state)?;
        let in_check = self.in_check(state, side)?;
        let halfmove_clock = self.halfmove_clock(state);
        let repetition_count = self.repetition_count(state, history)?;

        if let Some(outcome) = self.explicit_outcome(state)? {
            return Ok(ChessStatus {
                side_to_move: side,
                in_check,
                outcome: Some(outcome),
                repetition_count,
                halfmove_clock,
                can_agree_draw: false,
                can_claim_threefold_repetition: false,
                can_claim_fifty_move_rule: false,
            });
        }

        let legal = self.has_legal_move(state, Some(history), side)?;
        let outcome = if !legal {
            if in_check {
                Some(ChessOutcome::Checkmate {
                    winner: side.opponent().player(),
                    loser: side.player(),
                })
            } else {
                Some(ChessOutcome::Stalemate)
            }
        } else if self.is_dead_position(state) {
            Some(ChessOutcome::DeadPosition)
        } else if repetition_count >= 5 {
            Some(ChessOutcome::FivefoldRepetition)
        } else if halfmove_clock >= 150 {
            Some(ChessOutcome::SeventyFiveMoveRule)
        } else {
            None
        };

        Ok(ChessStatus {
            side_to_move: side,
            in_check,
            can_agree_draw: outcome.is_none() && self.fullmove_number(state) > 1,
            can_claim_threefold_repetition: outcome.is_none() && repetition_count >= 3,
            can_claim_fifty_move_rule: outcome.is_none() && halfmove_clock >= 100,
            outcome,
            repetition_count,
            halfmove_clock,
        })
    }

    pub fn resign(
        &self,
        state: &mut GameState,
        history: &History,
        player: PlayerId,
    ) -> Result<ChessOutcome, ChessError> {
        self.ensure_ongoing(state, history)?;
        let side = ChessSide::from_player(player).ok_or(ChessError::UnknownSide(player))?;
        let outcome = ChessOutcome::Resignation {
            winner: side.opponent().player(),
            loser: player,
        };
        self.write_explicit_outcome(state, &outcome);
        Ok(outcome)
    }

    pub fn agree_draw(
        &self,
        state: &mut GameState,
        history: &History,
    ) -> Result<ChessOutcome, ChessError> {
        self.ensure_ongoing(state, history)?;
        if self.fullmove_number(state) <= 1 {
            return Err(ChessError::DrawAgreementUnavailable);
        }
        let outcome = ChessOutcome::DrawAgreement;
        self.write_explicit_outcome(state, &outcome);
        Ok(outcome)
    }

    pub fn claim_draw(
        &self,
        state: &mut GameState,
        history: &History,
        claim: ChessDrawClaim,
    ) -> Result<ChessOutcome, ChessError> {
        let status = self.status(state, history)?;
        let outcome = match claim {
            ChessDrawClaim::ThreefoldRepetition if status.can_claim_threefold_repetition => {
                ChessOutcome::ThreefoldRepetition
            }
            ChessDrawClaim::FiftyMoveRule if status.can_claim_fifty_move_rule => {
                ChessOutcome::FiftyMoveRule
            }
            _ => return Err(ChessError::InvalidDrawClaim),
        };
        self.write_explicit_outcome(state, &outcome);
        Ok(outcome)
    }

    /// Returns legal moves that make a FIDE draw claim valid before the move is played.
    ///
    /// Article 9.2.1 allows a threefold claim when the indicated next move would
    /// create the third occurrence. Article 9.3.1 does the same for the fifty-move
    /// rule. The projected move is never committed to the authoritative state.
    pub fn claimable_draw_moves(
        &self,
        state: &GameState,
        history: &History,
        claim: ChessDrawClaim,
    ) -> Result<Vec<(crate::PseudoMove, Option<EntityTypeId>)>, ChessError> {
        if self.status(state, history)?.outcome.is_some() {
            return Ok(Vec::new());
        }
        let side = self.side_to_move(state)?;
        let mut claimable = Vec::new();
        for movement in self.legal_moves_for_side_with_history(state, Some(history), side)? {
            let choices = self.move_choices(
                state,
                Some(history),
                movement,
                &monumla_core::StateMap::new(),
            )?;
            if choices.is_empty() {
                if self.projected_move_satisfies_claim(state, history, claim, movement, None)? {
                    claimable.push((movement, None));
                }
                continue;
            }
            for choice in choices {
                let promotion = choice
                    .data
                    .get("entity_type")
                    .and_then(StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(EntityTypeId::new);
                if self
                    .projected_move_satisfies_claim(state, history, claim, movement, promotion)?
                {
                    claimable.push((movement, promotion));
                }
            }
        }
        Ok(claimable)
    }

    pub fn claim_draw_after_move(
        &self,
        state: &mut GameState,
        history: &History,
        claim: ChessDrawClaim,
        movement: crate::PseudoMove,
        promotion: Option<EntityTypeId>,
    ) -> Result<ChessOutcome, ChessError> {
        self.ensure_ongoing(state, history)?;
        if !self.projected_move_satisfies_claim(state, history, claim, movement, promotion)? {
            return Err(ChessError::InvalidDrawClaim);
        }
        let outcome = match claim {
            ChessDrawClaim::ThreefoldRepetition => ChessOutcome::ThreefoldRepetition,
            ChessDrawClaim::FiftyMoveRule => ChessOutcome::FiftyMoveRule,
        };
        self.write_explicit_outcome(state, &outcome);
        Ok(outcome)
    }

    pub(crate) fn record_result(
        &self,
        state: &mut GameState,
        winner: Option<PlayerId>,
    ) -> Result<ChessOutcome, ChessError> {
        let outcome = if let Some(winner) = winner {
            let side = ChessSide::from_player(winner).ok_or(ChessError::UnknownSide(winner))?;
            ChessOutcome::RecordedWin {
                winner,
                loser: side.opponent().player(),
            }
        } else {
            ChessOutcome::RecordedDraw
        };
        self.write_explicit_outcome(state, &outcome);
        Ok(outcome)
    }

    fn ensure_ongoing(&self, state: &GameState, history: &History) -> Result<(), ChessError> {
        if self.status(state, history)?.outcome.is_some() {
            return Err(ChessError::GameFinished);
        }
        Ok(())
    }

    fn projected_move_satisfies_claim(
        &self,
        state: &GameState,
        history: &History,
        claim: ChessDrawClaim,
        movement: crate::PseudoMove,
        promotion: Option<EntityTypeId>,
    ) -> Result<bool, ChessError> {
        let (projected, projected_history) =
            self.simulate_notated_turn(state, history, movement, promotion)?;
        Ok(match claim {
            ChessDrawClaim::ThreefoldRepetition => {
                self.repetition_count(&projected, &projected_history)? >= 3
            }
            ChessDrawClaim::FiftyMoveRule => self.halfmove_clock(&projected) >= 100,
        })
    }

    pub fn position_key(
        &self,
        state: &GameState,
        history: &History,
    ) -> Result<PositionKey, ChessError> {
        self.position_key_after(state, previous_chess_move(history))
    }

    pub fn repetition_count(
        &self,
        state: &GameState,
        history: &History,
    ) -> Result<usize, ChessError> {
        let target = self.position_key(state, history)?;
        if history.is_empty() {
            return Ok(1);
        }

        let turns = history.turns();
        let mut count = 0;
        if let Some(first) = turns.first() {
            if self.position_key_after(&first.before, None)? == target {
                count += 1;
            }
        }
        for turn in turns.iter().filter(|turn| turn_records_chess_move(turn)) {
            if self.position_key_after(&turn.after, Some(turn))? == target {
                count += 1;
            }
        }
        let current_is_recorded = if let Some(previous) = previous_chess_move(history) {
            self.position_key_after(&previous.after, Some(previous))? == target
        } else if let Some(first) = turns.first() {
            // A terminal metadata turn may exist before any real chess move (for
            // example a PGN declared result). It does not make the unchanged
            // initial position occur a second time.
            self.position_key_after(&first.before, None)? == target
        } else {
            false
        };
        if !current_is_recorded {
            count += 1;
        }
        Ok(count)
    }

    /// Sound hot-path recognition of standard material-dead positions.
    ///
    /// This deliberately returns `false` for exotic blocked-material positions that
    /// would require arbitrary legal-reachability search to prove FIDE deadness.
    /// False negatives are preferable here to declaring a playable position drawn.
    pub fn is_dead_position(&self, state: &GameState) -> bool {
        let non_kings = state
            .entities
            .values()
            .filter(|entity| entity.entity_type != KING)
            .collect::<Vec<_>>();

        if non_kings.iter().any(|entity| {
            entity.entity_type == PAWN || entity.entity_type == ROOK || entity.entity_type == QUEEN
        }) {
            return false;
        }
        if non_kings.is_empty() {
            return true;
        }
        if non_kings.len() == 1 {
            return non_kings[0].entity_type == BISHOP || non_kings[0].entity_type == KNIGHT;
        }
        if non_kings.iter().all(|entity| entity.entity_type == BISHOP) {
            let first_color = square_color(non_kings[0].position);
            return non_kings
                .iter()
                .all(|bishop| square_color(bishop.position) == first_color);
        }
        false
    }

    pub(crate) fn update_halfmove_clock_for_move(
        &self,
        state: &mut GameState,
        actor_type: EntityTypeId,
        is_capture: bool,
    ) {
        let next = if actor_type == PAWN || is_capture {
            0
        } else {
            self.halfmove_clock(state).saturating_add(1)
        };
        self.set_halfmove_clock(state, next);
    }

    fn position_key_after(
        &self,
        state: &GameState,
        previous_turn: Option<&TurnRecord>,
    ) -> Result<PositionKey, ChessError> {
        let side = self.side_to_move(state)?;
        let mut pieces = state
            .entities
            .values()
            .map(|entity| PositionPieceKey {
                position: entity.position,
                entity_type: entity.entity_type,
                owner: entity.owner,
            })
            .collect::<Vec<_>>();
        pieces.sort();

        Ok(PositionKey {
            pieces,
            side_to_move: side.player(),
            castling_rights: self.effective_castling_rights(state),
            en_passant_targets: self.effective_en_passant_targets(state, previous_turn, side)?,
        })
    }

    pub(crate) fn effective_castling_rights(&self, state: &GameState) -> u8 {
        let mut rights = 0_u8;
        for (side, king_side_bit, queen_side_bit) in [
            (ChessSide::White, 0b0001, 0b0010),
            (ChessSide::Black, 0b0100, 0b1000),
        ] {
            let rank = side.home_rank();
            let Some(king) = state.entity_at(Position::new(4, rank)).ok().flatten() else {
                continue;
            };
            if king.entity_type != KING || king.owner != side.player() || has_moved(king) {
                continue;
            }
            for (rook_x, bit) in [(7_u16, king_side_bit), (0_u16, queen_side_bit)] {
                let Some(rook) = state.entity_at(Position::new(rook_x, rank)).ok().flatten() else {
                    continue;
                };
                if rook.entity_type == ROOK && rook.owner == side.player() && !has_moved(rook) {
                    rights |= bit;
                }
            }
        }
        rights
    }

    fn effective_en_passant_targets(
        &self,
        state: &GameState,
        previous_turn: Option<&TurnRecord>,
        side: ChessSide,
    ) -> Result<Vec<Position>, ChessError> {
        let Some(previous_turn) = previous_turn else {
            return Ok(Vec::new());
        };
        let mut targets = Vec::new();
        for pawn in state
            .entities
            .values()
            .filter(|entity| entity.owner == side.player() && entity.entity_type == PAWN)
        {
            let context = ChessPieceContext::new(state, pawn.id)?;
            for movement in en_passant_moves_from_previous(context, previous_turn)? {
                let mut candidate = state.clone();
                self.apply_move_unchecked(&mut candidate, &movement)?;
                if !self.in_check(&candidate, side)? {
                    targets.push(movement.to);
                }
            }
        }
        targets.sort();
        targets.dedup();
        Ok(targets)
    }

    fn write_explicit_outcome(&self, state: &mut GameState, outcome: &ChessOutcome) {
        state.ruleset_state.remove(EXPLICIT_WINNER);
        state.ruleset_state.remove(EXPLICIT_LOSER);
        match outcome {
            ChessOutcome::Resignation { winner, loser } => {
                state.ruleset_state.insert(EXPLICIT_OUTCOME, "resignation");
                state
                    .ruleset_state
                    .insert(EXPLICIT_WINNER, u64::from(winner.get()));
                state
                    .ruleset_state
                    .insert(EXPLICIT_LOSER, u64::from(loser.get()));
            }
            ChessOutcome::DrawAgreement => {
                state
                    .ruleset_state
                    .insert(EXPLICIT_OUTCOME, "draw_agreement");
            }
            ChessOutcome::ThreefoldRepetition => {
                state
                    .ruleset_state
                    .insert(EXPLICIT_OUTCOME, "threefold_repetition");
            }
            ChessOutcome::FiftyMoveRule => {
                state
                    .ruleset_state
                    .insert(EXPLICIT_OUTCOME, "fifty_move_rule");
            }
            ChessOutcome::RecordedWin { winner, loser } => {
                state.ruleset_state.insert(EXPLICIT_OUTCOME, "recorded_win");
                state
                    .ruleset_state
                    .insert(EXPLICIT_WINNER, u64::from(winner.get()));
                state
                    .ruleset_state
                    .insert(EXPLICIT_LOSER, u64::from(loser.get()));
            }
            ChessOutcome::RecordedDraw => {
                state
                    .ruleset_state
                    .insert(EXPLICIT_OUTCOME, "recorded_draw");
            }
            _ => {}
        }
    }

    pub(crate) fn explicit_outcome(
        &self,
        state: &GameState,
    ) -> Result<Option<ChessOutcome>, ChessError> {
        let Some(kind) = state
            .ruleset_state
            .get(EXPLICIT_OUTCOME)
            .and_then(StateValue::as_str)
        else {
            return Ok(None);
        };
        let outcome = match kind {
            "resignation" => {
                let winner = state
                    .ruleset_state
                    .get(EXPLICIT_WINNER)
                    .and_then(StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(PlayerId::new)
                    .ok_or(ChessError::InvalidOutcomeState)?;
                let loser = state
                    .ruleset_state
                    .get(EXPLICIT_LOSER)
                    .and_then(StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(PlayerId::new)
                    .ok_or(ChessError::InvalidOutcomeState)?;
                ChessOutcome::Resignation { winner, loser }
            }
            "draw_agreement" => ChessOutcome::DrawAgreement,
            "threefold_repetition" => ChessOutcome::ThreefoldRepetition,
            "fifty_move_rule" => ChessOutcome::FiftyMoveRule,
            "recorded_win" => {
                let winner = state
                    .ruleset_state
                    .get(EXPLICIT_WINNER)
                    .and_then(StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(PlayerId::new)
                    .ok_or(ChessError::InvalidOutcomeState)?;
                let loser = state
                    .ruleset_state
                    .get(EXPLICIT_LOSER)
                    .and_then(StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(PlayerId::new)
                    .ok_or(ChessError::InvalidOutcomeState)?;
                ChessOutcome::RecordedWin { winner, loser }
            }
            "recorded_draw" => ChessOutcome::RecordedDraw,
            _ => return Err(ChessError::InvalidOutcomeState),
        };
        Ok(Some(outcome))
    }
}

fn turn_records_chess_move(turn: &TurnRecord) -> bool {
    turn.steps
        .iter()
        .any(|step| step.action.kind == "chess_move")
}

fn previous_chess_move(history: &History) -> Option<&TurnRecord> {
    history
        .turns()
        .iter()
        .rev()
        .find(|turn| turn_records_chess_move(turn))
        .map(AsRef::as_ref)
}

fn square_color(position: Position) -> bool {
    (position.x + position.y) % 2 == 0
}
