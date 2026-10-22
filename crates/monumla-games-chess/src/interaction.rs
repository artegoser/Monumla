use crate::{ChessError, ChessRules, ChessSide, PseudoMove};
use monumla_core::{
    Choice, ChoiceInput, ChoiceKind, ChoiceSpec, EntityId, History, InteractionError, InteractionFlow,
    InteractionRules, RuleContext, StateMap, TurnSession,
};

const SELECTED_ENTITY: &str = "chess.selected_entity";
const PENDING_X: &str = "chess.pending_x";
const PENDING_Y: &str = "chess.pending_y";

pub struct ChessInteractionRules {
    rules: ChessRules,
    history: Option<History>,
}

impl ChessInteractionRules {
    pub fn new(rules: &ChessRules) -> Self {
        Self {
            rules: rules.clone(),
            history: None,
        }
    }

    pub fn with_history(rules: &ChessRules, history: &History) -> Self {
        Self {
            rules: rules.clone(),
            history: Some(history.clone()),
        }
    }

    fn side_to_move(&self, turn: &TurnSession) -> Result<ChessSide, InteractionError> {
        let [player] = turn.working.turn.active_players.as_slice() else {
            return Err(InteractionError::RuleViolation(
                "standard chess requires exactly one active player".into(),
            ));
        };
        ChessSide::from_player(*player).ok_or_else(|| {
            InteractionError::RuleViolation(format!("active player {player} is not a chess side"))
        })
    }

    pub fn selected_entity(draft: &StateMap) -> Option<EntityId> {
        let raw = draft.get(SELECTED_ENTITY)?.as_u64()?;
        let raw = u32::try_from(raw).ok()?;
        Some(EntityId::new(raw))
    }

    fn set_selected(draft: &mut StateMap, entity: EntityId) {
        draft.insert(SELECTED_ENTITY, u64::from(entity.get()));
        draft.remove(PENDING_X);
        draft.remove(PENDING_Y);
    }

    pub fn pending_target(draft: &StateMap) -> Option<monumla_core::Position> {
        let x = u16::try_from(draft.get(PENDING_X)?.as_u64()?).ok()?;
        let y = u16::try_from(draft.get(PENDING_Y)?.as_u64()?).ok()?;
        Some(monumla_core::Position::new(x, y))
    }

    fn set_pending_target(draft: &mut StateMap, target: monumla_core::Position) {
        draft.insert(PENDING_X, u64::from(target.x));
        draft.insert(PENDING_Y, u64::from(target.y));
    }

    fn clear_selection(draft: &mut StateMap) {
        draft.remove(SELECTED_ENTITY);
        draft.remove(PENDING_X);
        draft.remove(PENDING_Y);
    }

    fn rule_error(error: ChessError) -> InteractionError {
        InteractionError::RuleViolation(error.to_string())
    }

    fn legal_moves(
        &self,
        turn: &TurnSession,
        entity: EntityId,
    ) -> Result<Vec<PseudoMove>, InteractionError> {
        self.rules
            .legal_moves_with_history(&turn.working, self.history.as_ref(), entity)
            .map_err(Self::rule_error)
    }

    fn movement_to(
        &self,
        turn: &TurnSession,
        actor: EntityId,
        target: monumla_core::Position,
    ) -> Result<PseudoMove, InteractionError> {
        self.legal_moves(turn, actor)?
            .into_iter()
            .find(|movement| movement.to == target)
            .ok_or_else(|| {
                InteractionError::RuleViolation("selected destination is not legal".into())
            })
    }

    fn execute(
        &self,
        turn: &mut TurnSession,
        movement: PseudoMove,
        input: Option<&ChoiceInput>,
    ) -> Result<InteractionFlow, InteractionError> {
        self.rules
            .execute_move(turn, self.history.as_ref(), movement, input)
            .map_err(Self::rule_error)?;
        Ok(InteractionFlow::FinishTurn)
    }
}

impl InteractionRules for ChessInteractionRules {
    fn choices(
        &self,
        turn: &TurnSession,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let side = self.side_to_move(turn)?;
        let empty_history = History::default();
        let history = self.history.as_ref().unwrap_or(&empty_history);
        if self
            .rules
            .status(&turn.working, history)
            .map_err(Self::rule_error)?
            .outcome
            .is_some()
        {
            return Ok(Vec::new());
        }

        if let Some(target) = Self::pending_target(draft) {
            let actor = Self::selected_entity(draft).ok_or_else(|| {
                InteractionError::RuleViolation("pending continuation has no selected entity".into())
            })?;
            let movement = self.movement_to(turn, actor, target)?;
            let choices = self
                .rules
                .move_choices(&turn.working, self.history.as_ref(), movement, draft)
                .map_err(Self::rule_error)?;
            if choices.is_empty() {
                return Err(InteractionError::RuleViolation(
                    "pending move continuation no longer has legal choices".into(),
                ));
            }
            return Ok(choices);
        }

        let mut choices = Vec::new();
        for entity in turn
            .working
            .entities
            .values()
            .filter(|entity| entity.controller == side.player())
        {
            let legal_moves = self.legal_moves(turn, entity.id)?;
            let mut destinations = Vec::new();
            for movement in legal_moves {
                let local_continuations = self
                    .rules
                    .piece_move_choices(&turn.working, self.history.as_ref(), movement)
                    .map_err(Self::rule_error)?;
                if !local_continuations.is_empty()
                    && self
                        .rules
                        .move_choices(&turn.working, self.history.as_ref(), movement, draft)
                        .map_err(Self::rule_error)?
                        .is_empty()
                {
                    // A game rule filtered every required piece-local continuation,
                    // therefore this move cannot currently be completed.
                    continue;
                }

                let mut choice = ChoiceSpec::position(movement.to);
                choice.data.insert("actor", u64::from(entity.id.get()));
                if let Some(capture) = movement.capture {
                    choice.data.insert("capture", u64::from(capture.get()));
                }
                choice.data.insert(
                    "move_kind",
                    match movement.kind {
                        crate::ChessMoveKind::Normal => "normal",
                        crate::ChessMoveKind::EnPassant { .. } => "en_passant",
                        crate::ChessMoveKind::Castle { .. } => "castle",
                    },
                );
                destinations.push(choice);
            }
            if destinations.is_empty() {
                continue;
            }
            choices.push(ChoiceSpec::entity(entity.id));
            choices.extend(destinations);
        }

        self.rules
            .resolve_game_choices(
                RuleContext::from_turn(turn, self.history.as_ref()),
                side.player(),
                draft,
                choices,
            )
            .map_err(Self::rule_error)
    }

    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        if let Some(flow) = self
            .rules
            .apply_game_choice(self.history.as_ref(), turn, draft, choice)
            .map_err(Self::rule_error)?
        {
            return Ok(flow);
        }

        if let Some(target) = Self::pending_target(draft) {
            let actor = Self::selected_entity(draft).ok_or_else(|| {
                InteractionError::RuleViolation("pending continuation has no selected entity".into())
            })?;
            let movement = self.movement_to(turn, actor, target)?;
            let input = ChoiceInput::from(choice);
            let result = self.execute(turn, movement, Some(&input))?;
            Self::clear_selection(draft);
            return Ok(result);
        }

        match &choice.kind {
            ChoiceKind::SelectEntity { entity } => {
                if Self::selected_entity(draft) == Some(*entity) {
                    Self::clear_selection(draft);
                    return Ok(InteractionFlow::Continue);
                }
                let side = self.side_to_move(turn)?;
                let piece = turn.working.entity(*entity)?;
                if piece.controller != side.player() {
                    return Err(InteractionError::RuleViolation(
                        "selected entity is not controlled by the active player".into(),
                    ));
                }
                if self.legal_moves(turn, *entity)?.is_empty() {
                    return Err(InteractionError::RuleViolation(
                        "selected entity has no legal moves".into(),
                    ));
                }
                Self::set_selected(draft, *entity);
                Ok(InteractionFlow::Continue)
            }
            ChoiceKind::SelectPosition { position } => {
                let actor_from_choice = choice
                    .data
                    .get("actor")
                    .and_then(monumla_core::StateValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .map(EntityId::new)
                    .ok_or_else(|| {
                        InteractionError::RuleViolation("destination choice has no actor".into())
                    })?;
                let actor = match Self::selected_entity(draft) {
                    Some(selected) if selected != actor_from_choice => {
                        return Err(InteractionError::RuleViolation(
                            "destination belongs to a different selected entity".into(),
                        ));
                    }
                    Some(selected) => selected,
                    None => {
                        Self::set_selected(draft, actor_from_choice);
                        actor_from_choice
                    }
                };
                let movement = self.movement_to(turn, actor, *position)?;
                let local_continuations = self
                    .rules
                    .piece_move_choices(&turn.working, self.history.as_ref(), movement)
                    .map_err(Self::rule_error)?;
                if !local_continuations.is_empty() {
                    let choices = self
                        .rules
                        .move_choices(&turn.working, self.history.as_ref(), movement, draft)
                        .map_err(Self::rule_error)?;
                    if choices.is_empty() {
                        return Err(InteractionError::RuleViolation(
                            "move has no legal continuation after game-rule filtering".into(),
                        ));
                    }
                    Self::set_pending_target(draft, *position);
                    return Ok(InteractionFlow::Continue);
                }

                let result = self.execute(turn, movement, None)?;
                Self::clear_selection(draft);
                Ok(result)
            }
            _ => Err(InteractionError::RuleViolation(
                "choice is not valid for the current chess interaction".into(),
            )),
        }
    }
}
