use crate::{
    piece::set_has_moved, pieces::king::castling_moves, Bishop, ChessError, ChessMoveKind, ChessPieceContext,
    ChessPieceRule, ChessSide, King, Knight, Pawn, PseudoMove, Queen, Rook, BISHOP, KING, KNIGHT,
    PAWN, QUEEN, ROOK,
};
use monumla_core::{
    Choice, ChoiceInput, ChoiceSpec, EntityId, EntityPresentation, EntityState, EntityTypeId, GameRule,
    GameRuleSet, GameState, History, InteractionFlow, PlayerId, PlayerState, Position,
    RecordedAction, RuleContext, StateMap, TeamId, TeamState, TurnSession,
};
use std::{collections::BTreeMap, sync::Arc};

pub const WHITE_PLAYER: PlayerId = PlayerId::new(1);
pub const BLACK_PLAYER: PlayerId = PlayerId::new(2);
pub const WHITE_TEAM: TeamId = TeamId::new(1);
pub const BLACK_TEAM: TeamId = TeamId::new(2);

impl ChessSide {
    pub const fn player(self) -> PlayerId {
        match self {
            Self::White => WHITE_PLAYER,
            Self::Black => BLACK_PLAYER,
        }
    }

    pub const fn team(self) -> TeamId {
        match self {
            Self::White => WHITE_TEAM,
            Self::Black => BLACK_TEAM,
        }
    }

    pub const fn from_player(player: PlayerId) -> Option<Self> {
        if player.get() == WHITE_PLAYER.get() {
            Some(Self::White)
        } else if player.get() == BLACK_PLAYER.get() {
            Some(Self::Black)
        } else {
            None
        }
    }

    pub const fn forward(self) -> i16 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }

    pub const fn pawn_start_rank(self) -> u16 {
        match self {
            Self::White => 1,
            Self::Black => 6,
        }
    }

    pub const fn opponent(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    pub const fn home_rank(self) -> u16 {
        match self {
            Self::White => 0,
            Self::Black => 7,
        }
    }

}

pub fn empty_chess_state() -> Result<GameState, ChessError> {
    let mut state = GameState::new(8, 8)?;
    state.add_team(TeamState::new(WHITE_TEAM))?;
    state.add_team(TeamState::new(BLACK_TEAM))?;
    state.add_player(PlayerState::new(WHITE_PLAYER).with_team(WHITE_TEAM))?;
    state.add_player(PlayerState::new(BLACK_PLAYER).with_team(BLACK_TEAM))?;
    state.set_active_players(vec![WHITE_PLAYER])?;
    Ok(state)
}

pub fn standard_chess_state() -> Result<GameState, ChessError> {
    let mut state = empty_chess_state()?;
    let back_rank = [ROOK, KNIGHT, BISHOP, QUEEN, KING, BISHOP, KNIGHT, ROOK];
    let mut next_id = 1_u32;

    for (x, entity_type) in back_rank.iter().copied().enumerate() {
        add_piece(
            &mut state,
            &mut next_id,
            entity_type,
            ChessSide::White,
            Position::new(x as u16, 0),
        )?;
    }
    for x in 0..8_u16 {
        add_piece(
            &mut state,
            &mut next_id,
            PAWN,
            ChessSide::White,
            Position::new(x, 1),
        )?;
    }
    for x in 0..8_u16 {
        add_piece(
            &mut state,
            &mut next_id,
            PAWN,
            ChessSide::Black,
            Position::new(x, 6),
        )?;
    }
    for (x, entity_type) in back_rank.iter().copied().enumerate() {
        add_piece(
            &mut state,
            &mut next_id,
            entity_type,
            ChessSide::Black,
            Position::new(x as u16, 7),
        )?;
    }

    state.validate()?;
    Ok(state)
}

fn add_piece(
    state: &mut GameState,
    next_id: &mut u32,
    entity_type: EntityTypeId,
    side: ChessSide,
    position: Position,
) -> Result<EntityId, ChessError> {
    let id = EntityId::new(*next_id);
    *next_id = (*next_id).saturating_add(1);
    let mut entity = EntityState::new(id, entity_type, side.player(), position);
    set_has_moved(&mut entity, false);
    state.add_entity(entity)?;
    Ok(id)
}

#[derive(Clone)]
pub struct ChessRules {
    pieces: BTreeMap<EntityTypeId, Arc<dyn ChessPieceRule>>,
    game_rules: GameRuleSet,
}

impl ChessRules {
    pub fn standard() -> Self {
        let mut rules = Self {
            pieces: BTreeMap::new(),
            game_rules: GameRuleSet::new(),
        };
        rules.register(Pawn).expect("pawn type is unique");
        rules.register(Knight).expect("knight type is unique");
        rules.register(Bishop).expect("bishop type is unique");
        rules.register(Rook).expect("rook type is unique");
        rules.register(Queen).expect("queen type is unique");
        rules.register(King).expect("king type is unique");
        rules
    }

    pub fn outcome_rule(&self) -> crate::ChessOutcomeRule {
        crate::ChessOutcomeRule::new(self.clone())
    }

    pub fn register_game_rule<R>(&mut self, rule: R)
    where
        R: GameRule + 'static,
    {
        self.game_rules.register(rule);
    }

    pub fn game_rule_count(&self) -> usize {
        self.game_rules.len()
    }

    pub fn register<R>(&mut self, rule: R) -> Result<(), ChessError>
    where
        R: ChessPieceRule + 'static,
    {
        let entity_type = rule.entity_type();
        if self.pieces.contains_key(&entity_type) {
            return Err(ChessError::DuplicatePieceRule(entity_type));
        }
        self.pieces.insert(entity_type, Arc::new(rule));
        Ok(())
    }

    pub fn piece_rule(&self, entity_type: EntityTypeId) -> Result<&dyn ChessPieceRule, ChessError> {
        self.pieces
            .get(&entity_type)
            .map(Arc::as_ref)
            .ok_or(ChessError::PieceRuleNotFound(entity_type))
    }

    pub fn presentation(
        &self,
        state: &GameState,
        history: Option<&History>,
        entity: EntityId,
    ) -> Result<EntityPresentation, ChessError> {
        let context = RuleContext::from_state(state, history).entity_context(entity)?;
        Ok(self
            .piece_rule(context.entity().entity_type)?
            .presentation(context)?)
    }

    pub(crate) fn piece_move_choices(
        &self,
        state: &GameState,
        history: Option<&History>,
        movement: PseudoMove,
    ) -> Result<Vec<ChoiceSpec>, ChessError> {
        let context = ChessPieceContext::with_history(state, history, movement.actor)?;
        self.piece_rule(context.entity().entity_type)?
            .move_choices(context, &movement)
    }

    pub fn move_choices(
        &self,
        state: &GameState,
        history: Option<&History>,
        movement: PseudoMove,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, ChessError> {
        let actor = state.entity(movement.actor)?.controller;
        let choices = self.piece_move_choices(state, history, movement)?;
        Ok(self.game_rules.transform_choices(
            RuleContext::from_state(state, history),
            actor,
            draft,
            choices,
        )?)
    }

    pub fn resolve_game_choices(
        &self,
        context: RuleContext<'_>,
        actor: PlayerId,
        draft: &StateMap,
        choices: Vec<ChoiceSpec>,
    ) -> Result<Vec<ChoiceSpec>, ChessError> {
        Ok(self
            .game_rules
            .resolve_choices(context, actor, draft, choices)?)
    }

    pub fn apply_game_choice(
        &self,
        history: Option<&History>,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<Option<InteractionFlow>, ChessError> {
        Ok(self
            .game_rules
            .apply_choice(history, turn, draft, choice)?)
    }

    pub fn pseudo_moves(
        &self,
        state: &GameState,
        entity: EntityId,
    ) -> Result<Vec<PseudoMove>, ChessError> {
        self.pseudo_moves_with_history(state, None, entity)
    }

    pub fn pseudo_moves_with_history(
        &self,
        state: &GameState,
        history: Option<&History>,
        entity: EntityId,
    ) -> Result<Vec<PseudoMove>, ChessError> {
        let context = ChessPieceContext::with_history(state, history, entity)?;
        let mut moves = self
            .piece_rule(context.entity().entity_type)?
            .pseudo_moves(context)?;
        if context.entity().entity_type == KING {
            moves.extend(castling_moves(self, context)?);
        }
        Ok(moves)
    }

    pub fn attacks(
        &self,
        state: &GameState,
        entity: EntityId,
    ) -> Result<Vec<Position>, ChessError> {
        let context = ChessPieceContext::new(state, entity)?;
        self.piece_rule(context.entity().entity_type)?
            .attacks(context)
    }

    pub fn king(&self, state: &GameState, side: ChessSide) -> Result<EntityId, ChessError> {
        let mut kings = state
            .entities
            .values()
            .filter(|entity| entity.owner == side.player() && entity.entity_type == KING)
            .map(|entity| entity.id);
        let king = kings.next().ok_or(ChessError::MissingKing(side.player()))?;
        if kings.next().is_some() {
            return Err(ChessError::MultipleKings(side.player()));
        }
        Ok(king)
    }

    pub fn is_square_attacked(
        &self,
        state: &GameState,
        by_side: ChessSide,
        square: Position,
    ) -> Result<bool, ChessError> {
        for entity in state
            .entities
            .values()
            .filter(|entity| entity.owner == by_side.player())
        {
            if self.attacks(state, entity.id)?.contains(&square) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn in_check(&self, state: &GameState, side: ChessSide) -> Result<bool, ChessError> {
        let king = self.king(state, side)?;
        let king_square = state.entity(king)?.position;
        self.is_square_attacked(state, side.opponent(), king_square)
    }

    pub fn legal_moves(
        &self,
        state: &GameState,
        entity: EntityId,
    ) -> Result<Vec<PseudoMove>, ChessError> {
        self.legal_moves_with_history(state, None, entity)
    }

    pub fn legal_moves_with_history(
        &self,
        state: &GameState,
        history: Option<&History>,
        entity: EntityId,
    ) -> Result<Vec<PseudoMove>, ChessError> {
        let actor = state.entity(entity)?;
        let side = ChessSide::from_player(actor.owner).ok_or(ChessError::UnknownSide(actor.owner))?;
        let mut legal = Vec::new();

        for movement in self.pseudo_moves_with_history(state, history, entity)? {
            if let Some(captured) = movement.capture {
                if state.entity(captured)?.entity_type == KING {
                    continue;
                }
            }

            let mut candidate = state.clone();
            self.apply_move_unchecked(&mut candidate, &movement)?;
            if !self.in_check(&candidate, side)? {
                legal.push(movement);
            }
        }

        Ok(legal)
    }

    pub fn legal_moves_for_side(
        &self,
        state: &GameState,
        side: ChessSide,
    ) -> Result<Vec<PseudoMove>, ChessError> {
        self.legal_moves_for_side_with_history(state, None, side)
    }

    pub fn legal_moves_for_side_with_history(
        &self,
        state: &GameState,
        history: Option<&History>,
        side: ChessSide,
    ) -> Result<Vec<PseudoMove>, ChessError> {
        let mut legal = Vec::new();
        for entity in state
            .entities
            .values()
            .filter(|entity| entity.owner == side.player())
        {
            legal.extend(self.legal_moves_with_history(state, history, entity.id)?);
        }
        Ok(legal)
    }

    pub fn execute_move(
        &self,
        turn: &mut TurnSession,
        history: Option<&History>,
        movement: PseudoMove,
        input: Option<&ChoiceInput>,
    ) -> Result<(), ChessError> {
        self.game_rules
            .validate(RuleContext::from_turn(turn, history))?;
        let empty_history = History::default();
        let status_history = history.unwrap_or(&empty_history);
        if self.status(&turn.working, status_history)?.outcome.is_some() {
            return Err(ChessError::GameFinished);
        }
        let actor = turn.working.entity(movement.actor)?;
        let side = ChessSide::from_player(actor.owner).ok_or(ChessError::UnknownSide(actor.owner))?;
        let actor_type = actor.entity_type;
        let is_capture = movement.capture.is_some();
        let piece_context = ChessPieceContext::with_history(&turn.working, history, movement.actor)?;
        let piece_rule = self.piece_rule(actor_type)?;
        let legal = self.legal_moves_with_history(&turn.working, history, movement.actor)?;
        if !legal.contains(&movement) {
            return Err(ChessError::IllegalMove(movement.actor, movement.to));
        }
        let local_choices = piece_rule.move_choices(piece_context, &movement)?;
        let allowed_inputs = self.game_rules.transform_choices(
            RuleContext::from_turn(turn, history),
            actor.controller,
            &StateMap::new(),
            local_choices.clone(),
        )?;
        if !local_choices.is_empty() && allowed_inputs.is_empty() {
            return Err(ChessError::MoveInputRejected(movement.actor));
        }
        piece_rule.validate_move_input(piece_context, &movement, input)?;
        if let Some(input) = input {
            if !allowed_inputs
                .iter()
                .map(ChoiceInput::from)
                .any(|candidate| candidate == *input)
            {
                return Err(ChessError::MoveInputRejected(movement.actor));
            }
        }

        let mut action_data = StateMap::new();
        action_data.insert("actor", u64::from(movement.actor.get()));
        action_data.insert("from_x", u64::from(movement.from.x));
        action_data.insert("from_y", u64::from(movement.from.y));
        action_data.insert("to_x", u64::from(movement.to.x));
        action_data.insert("to_y", u64::from(movement.to.y));

        let action = RecordedAction {
            kind: "chess_move".into(),
            data: action_data,
        };
        let before = turn.working.clone();
        turn.apply_transaction(action.clone(), |transaction| -> Result<(), ChessError> {
            {
                let state = transaction.raw_state_mut();
                self.apply_move_unchecked(state, &movement)?;
                piece_rule.apply_move_input(state, &movement, input)?;
                self.update_halfmove_clock_for_move(state, actor_type, is_capture);
                if side == ChessSide::Black {
                    let next_fullmove = self.fullmove_number(state).saturating_add(1);
                    self.set_fullmove_number(state, next_fullmove);
                }
                state.set_active_players(vec![side.opponent().player()])?;
            }
            self.game_rules.react(&before, &action, transaction)?;
            Ok(())
        })?;
        Ok(())
    }

    pub(crate) fn apply_move_unchecked(
        &self,
        state: &mut GameState,
        movement: &PseudoMove,
    ) -> Result<(), ChessError> {
        let actor = state.entity(movement.actor)?;
        let actor_type = actor.entity_type;
        if actor.position != movement.from {
            return Err(ChessError::StaleMove(movement.actor));
        }

        match movement.kind {
            ChessMoveKind::Normal => {
                if let Some(captured) = movement.capture {
                    state.remove_entity(captured)?;
                }
                state.move_entity(movement.actor, movement.to)?;
            }
            ChessMoveKind::EnPassant { victim } => {
                state.remove_entity(victim)?;
                state.move_entity(movement.actor, movement.to)?;
            }
            ChessMoveKind::Castle { rook, rook_to } => {
                state.move_entity(movement.actor, movement.to)?;
                state.move_entity(rook, rook_to)?;
                set_has_moved(state.entity_mut(rook)?, true);
            }
        }

        if actor_type == KING || actor_type == ROOK {
            set_has_moved(state.entity_mut(movement.actor)?, true);
        }
        Ok(())
    }
}

impl Default for ChessRules {
    fn default() -> Self {
        Self::standard()
    }
}
