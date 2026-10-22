use monumla_core::{
    Choice, ChoiceKind, ChoiceSpec, EntityId, EntityPresentation, EntityRule, EntityRuleContext,
    EntityState, EntityTypeId, GameOutcome, GameState, InteractionError, InteractionFlow,
    InteractionRules, OutcomeRule, PlayerId, PlayerState, Position, PresentationCue, RecordedAction,
    RuleContext, RuleError, RuleRegistry, StateMap, StateValue, TurnSession,
};

pub const CHECKER: EntityTypeId = EntityTypeId::new(1);
pub const WHITE: PlayerId = PlayerId::new(1);
pub const BLACK: PlayerId = PlayerId::new(2);
const KING: &str = "checkers.king";
const SELECTED: &str = "checkers.selected";
const FORCED: &str = "checkers.forced";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CheckerMove {
    actor: EntityId,
    to: Position,
    capture: Option<EntityId>,
}

pub struct CheckerRule;

impl CheckerRule {
    fn directions(entity: &EntityState) -> &'static [(i16, i16)] {
        const WHITE_DIRS: [(i16, i16); 2] = [(-1, 1), (1, 1)];
        const BLACK_DIRS: [(i16, i16); 2] = [(-1, -1), (1, -1)];
        const KING_DIRS: [(i16, i16); 4] = [(-1, 1), (1, 1), (-1, -1), (1, -1)];
        if is_king(entity) {
            &KING_DIRS
        } else if entity.owner == WHITE {
            &WHITE_DIRS
        } else {
            &BLACK_DIRS
        }
    }

    fn captures(
        state: &GameState,
        actor: EntityId,
    ) -> Result<Vec<CheckerMove>, InteractionError> {
        let entity = state.entity(actor)?;
        let mut moves = Vec::new();
        for &(dx, dy) in Self::directions(entity) {
            let Some(middle) = offset(entity.position, dx, dy) else {
                continue;
            };
            let Some(to) = offset(entity.position, dx * 2, dy * 2) else {
                continue;
            };
            if state.entity_at(to)?.is_some() {
                continue;
            }
            let Some(victim) = state.entity_at(middle)? else {
                continue;
            };
            if victim.owner == entity.owner {
                continue;
            }
            moves.push(CheckerMove {
                actor,
                to,
                capture: Some(victim.id),
            });
        }
        Ok(moves)
    }

    fn quiet_moves(
        state: &GameState,
        actor: EntityId,
    ) -> Result<Vec<CheckerMove>, InteractionError> {
        let entity = state.entity(actor)?;
        let mut moves = Vec::new();
        for &(dx, dy) in Self::directions(entity) {
            let Some(to) = offset(entity.position, dx, dy) else {
                continue;
            };
            if state.entity_at(to)?.is_none() {
                moves.push(CheckerMove {
                    actor,
                    to,
                    capture: None,
                });
            }
        }
        Ok(moves)
    }

    fn promote_if_needed(
        state: &mut GameState,
        actor: EntityId,
    ) -> Result<bool, InteractionError> {
        let entity = state.entity(actor)?;
        let should_promote = !is_king(entity)
            && ((entity.owner == WHITE && entity.position.y == 7)
                || (entity.owner == BLACK && entity.position.y == 0));
        if should_promote {
            state.entity_mut(actor)?.state.insert(KING, true);
        }
        Ok(should_promote)
    }
}

impl EntityRule for CheckerRule {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        let entity = context.entity();
        let side = if entity.owner == WHITE { "white" } else { "black" };
        let rank = if is_king(entity) { "king" } else { "man" };
        Ok(EntityPresentation::new(format!("checkers/{side}/{rank}"))
            .with_label(format!("{side} {rank}")))
    }
}

pub fn registry() -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    registry
        .register_entity(CHECKER, CheckerRule)
        .expect("checkers registers one entity rule");
    registry.register_outcome(CheckersOutcomeRule);
    registry
}

pub fn standard_state() -> GameState {
    let mut state = GameState::new(8, 8).expect("valid checkers board");
    state.add_player(PlayerState::new(WHITE)).unwrap();
    state.add_player(PlayerState::new(BLACK)).unwrap();
    state.set_active_players(vec![WHITE]).unwrap();

    let mut next = 1_u32;
    for y in 0..3 {
        for x in 0..8 {
            if (x + y) % 2 == 1 {
                state
                    .add_entity(EntityState::new(
                        EntityId::new(next),
                        CHECKER,
                        WHITE,
                        Position::new(x, y),
                    ))
                    .unwrap();
                next += 1;
            }
        }
    }
    for y in 5..8 {
        for x in 0..8 {
            if (x + y) % 2 == 1 {
                state
                    .add_entity(EntityState::new(
                        EntityId::new(next),
                        CHECKER,
                        BLACK,
                        Position::new(x, y),
                    ))
                    .unwrap();
                next += 1;
            }
        }
    }
    state
}

fn is_king(entity: &EntityState) -> bool {
    entity
        .state
        .get(KING)
        .and_then(StateValue::as_bool)
        .unwrap_or(false)
}

fn offset(position: Position, dx: i16, dy: i16) -> Option<Position> {
    let x = i32::from(position.x) + i32::from(dx);
    let y = i32::from(position.y) + i32::from(dy);
    if !(0..8).contains(&x) || !(0..8).contains(&y) {
        return None;
    }
    Some(Position::new(u16::try_from(x).ok()?, u16::try_from(y).ok()?))
}


fn captures_for_player(
    state: &GameState,
    player: PlayerId,
) -> Result<Vec<CheckerMove>, InteractionError> {
    let mut captures = Vec::new();
    for entity in state
        .entities
        .values()
        .filter(|entity| entity.controller == player && entity.entity_type == CHECKER)
    {
        captures.extend(CheckerRule::captures(state, entity.id)?);
    }
    Ok(captures)
}

fn legal_moves_for_player(
    state: &GameState,
    player: PlayerId,
) -> Result<Vec<CheckerMove>, InteractionError> {
    let captures = captures_for_player(state, player)?;
    if !captures.is_empty() {
        return Ok(captures);
    }
    let mut moves = Vec::new();
    for entity in state
        .entities
        .values()
        .filter(|entity| entity.controller == player && entity.entity_type == CHECKER)
    {
        moves.extend(CheckerRule::quiet_moves(state, entity.id)?);
    }
    Ok(moves)
}

fn active_player(state: &GameState) -> Result<PlayerId, InteractionError> {
    let [player] = state.turn.active_players.as_slice() else {
        return Err(InteractionError::RuleViolation(
            "checkers requires exactly one active player".into(),
        ));
    };
    Ok(*player)
}

fn opponent(player: PlayerId) -> Result<PlayerId, InteractionError> {
    match player {
        WHITE => Ok(BLACK),
        BLACK => Ok(WHITE),
        _ => Err(InteractionError::RuleViolation(
            "unknown checkers player".into(),
        )),
    }
}

fn draft_entity(draft: &StateMap, key: &str) -> Option<EntityId> {
    draft
        .get(key)
        .and_then(StateValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .map(EntityId::new)
}

fn set_draft_entity(draft: &mut StateMap, key: &str, entity: EntityId) {
    draft.insert(key, u64::from(entity.get()));
}

fn move_choice(movement: CheckerMove) -> ChoiceSpec {
    let mut choice = ChoiceSpec::position(movement.to);
    choice.data.insert("actor", u64::from(movement.actor.get()));
    if let Some(capture) = movement.capture {
        choice.data.insert("capture", u64::from(capture.get()));
    }
    choice
}

fn actor_from_choice(choice: &Choice) -> Result<EntityId, InteractionError> {
    choice
        .data
        .get("actor")
        .and_then(StateValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .map(EntityId::new)
        .ok_or_else(|| InteractionError::RuleViolation("checkers move has no actor".into()))
}

pub struct CheckersInteractionRules;

impl CheckersInteractionRules {
    pub fn selected_entity(draft: &StateMap) -> Option<EntityId> {
        draft_entity(draft, SELECTED).or_else(|| draft_entity(draft, FORCED))
    }

    pub fn forced_entity(draft: &StateMap) -> Option<EntityId> {
        draft_entity(draft, FORCED)
    }
}

impl InteractionRules for CheckersInteractionRules {
    fn choices(
        &self,
        turn: &TurnSession,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let player = active_player(&turn.working)?;
        if let Some(forced) = draft_entity(draft, FORCED) {
            return Ok(CheckerRule::captures(&turn.working, forced)?
                .into_iter()
                .map(move_choice)
                .collect());
        }

        let legal = legal_moves_for_player(&turn.working, player)?;
        if legal.is_empty() {
            return Ok(Vec::new());
        }
        let selected = draft_entity(draft, SELECTED);
        let mut choices = Vec::new();
        for entity in turn
            .working
            .entities
            .values()
            .filter(|entity| entity.controller == player && entity.entity_type == CHECKER)
        {
            if legal.iter().any(|movement| movement.actor == entity.id) {
                choices.push(ChoiceSpec::entity(entity.id));
            }
        }
        choices.extend(
            legal
                .into_iter()
                .filter(|movement| selected.is_none() || selected == Some(movement.actor))
                .map(move_choice),
        );
        Ok(choices)
    }

    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        match &choice.kind {
            ChoiceKind::SelectEntity { entity } => {
                let player = active_player(&turn.working)?;
                let piece = turn.working.entity(*entity)?;
                if piece.controller != player || piece.entity_type != CHECKER {
                    return Err(InteractionError::RuleViolation(
                        "selected checker is not controlled by the active player".into(),
                    ));
                }
                if !legal_moves_for_player(&turn.working, player)?
                    .iter()
                    .any(|movement| movement.actor == *entity)
                {
                    return Err(InteractionError::RuleViolation(
                        "selected checker has no legal move".into(),
                    ));
                }
                set_draft_entity(draft, SELECTED, *entity);
                Ok(InteractionFlow::Continue)
            }
            ChoiceKind::SelectPosition { position } => {
                let player = active_player(&turn.working)?;
                let actor = actor_from_choice(choice)?;
                if let Some(forced) = draft_entity(draft, FORCED) {
                    if forced != actor {
                        return Err(InteractionError::RuleViolation(
                            "a capture chain must continue with the same checker".into(),
                        ));
                    }
                }
                if let Some(selected) = draft_entity(draft, SELECTED) {
                    if selected != actor {
                        return Err(InteractionError::RuleViolation(
                            "destination belongs to another checker".into(),
                        ));
                    }
                }

                let legal = if draft_entity(draft, FORCED).is_some() {
                    CheckerRule::captures(&turn.working, actor)?
                } else {
                    legal_moves_for_player(&turn.working, player)?
                        .into_iter()
                        .filter(|movement| movement.actor == actor)
                        .collect()
                };
                let movement = legal
                    .into_iter()
                    .find(|movement| movement.to == *position)
                    .ok_or_else(|| {
                        InteractionError::RuleViolation("illegal checkers move".into())
                    })?;

                let action = RecordedAction::new("checkers.move");
                let (_promoted, has_more_capture) = turn
                    .apply_transaction(
                        action,
                        |transaction| -> Result<(bool, bool), InteractionError> {
                            if let Some(capture) = movement.capture {
                                transaction.remove_entity(capture)?;
                            }
                            transaction.move_entity(actor, *position)?;
                            let promoted = CheckerRule::promote_if_needed(transaction.raw_state_mut(), actor)?;
                            if promoted {
                                let mut data = StateMap::new();
                                data.insert("entity", u64::from(actor.get()));
                                transaction.present(
                                    PresentationCue::new("checkers.promote").with_data(data),
                                );
                            }
                            let has_more_capture = movement.capture.is_some()
                                && !promoted
                                && !CheckerRule::captures(transaction.state(), actor)?.is_empty();
                            if !has_more_capture {
                                transaction
                                    .raw_state_mut()
                                    .set_active_players(vec![opponent(player)?])?;
                            }
                            Ok((promoted, has_more_capture))
                        },
                    )?
                    .value;

                if has_more_capture {
                    set_draft_entity(draft, SELECTED, actor);
                    set_draft_entity(draft, FORCED, actor);
                    return Ok(InteractionFlow::Continue);
                }

                draft.remove(SELECTED);
                draft.remove(FORCED);
                Ok(InteractionFlow::FinishTurn)
            }
            _ => Err(InteractionError::RuleViolation(
                "unexpected checkers choice".into(),
            )),
        }
    }
}

pub struct CheckersOutcomeRule;

impl OutcomeRule for CheckersOutcomeRule {
    fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        let player = match context.state().turn.active_players.as_slice() {
            [player] => *player,
            _ => return Ok(None),
        };
        let legal = legal_moves_for_player(context.state(), player)
            .map_err(|error| RuleError::Rejected(error.to_string()))?;
        if !legal.is_empty() {
            return Ok(None);
        }
        let winner = if player == WHITE { BLACK } else { WHITE };
        Ok(Some(
            GameOutcome::new("checkers.no_legal_moves")
                .with_winner(winner)
                .with_loser(player),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monumla_core::{GameTimeline, InteractionDriver, InteractionUpdate, StateChange};

    fn chain_state() -> (GameState, EntityId) {
        let mut state = GameState::new(8, 8).unwrap();
        state.add_player(PlayerState::new(WHITE)).unwrap();
        state.add_player(PlayerState::new(BLACK)).unwrap();
        state.set_active_players(vec![WHITE]).unwrap();
        let white = EntityId::new(1);
        state
            .add_entity(EntityState::new(
                white,
                CHECKER,
                WHITE,
                Position::new(1, 1),
            ))
            .unwrap();
        state
            .add_entity(EntityState::new(
                EntityId::new(2),
                CHECKER,
                BLACK,
                Position::new(2, 2),
            ))
            .unwrap();
        state
            .add_entity(EntityState::new(
                EntityId::new(3),
                CHECKER,
                BLACK,
                Position::new(4, 4),
            ))
            .unwrap();
        (state, white)
    }

    #[test]
    fn forced_multi_capture_is_one_turn_with_multiple_steps() {
        let (state, white) = chain_state();
        let turn = TurnSession::new(&state, WHITE).unwrap();
        let mut driver = InteractionDriver::new(CheckersInteractionRules, turn).unwrap();

        let first = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| {
                matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(3, 3))
            })
            .unwrap()
            .clone();
        driver.choose(first.id).unwrap();

        assert_eq!(driver.turn().steps.len(), 1);
        assert_eq!(driver.turn().working.entity(white).unwrap().position, Position::new(3, 3));
        assert!(driver.interaction().choices.iter().all(|choice| {
            matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(5, 5))
        }));

        let second = driver.interaction().choices[0].clone();
        assert_eq!(driver.choose(second.id).unwrap(), InteractionUpdate::Finished);
        assert_eq!(driver.turn().steps.len(), 2);
        assert_eq!(driver.turn().working.entity(white).unwrap().position, Position::new(5, 5));
        assert_eq!(driver.turn().working.turn.active_players, vec![BLACK]);
        assert_eq!(driver.turn().working.entities.len(), 1);
    }

    #[test]
    fn promotion_is_piece_local_state_and_emits_presentation() {
        let mut state = GameState::new(8, 8).unwrap();
        state.add_player(PlayerState::new(WHITE)).unwrap();
        state.add_player(PlayerState::new(BLACK)).unwrap();
        state.set_active_players(vec![WHITE]).unwrap();
        let checker = EntityId::new(1);
        state
            .add_entity(EntityState::new(
                checker,
                CHECKER,
                WHITE,
                Position::new(1, 6),
            ))
            .unwrap();
        let turn = TurnSession::new(&state, WHITE).unwrap();
        let mut driver = InteractionDriver::new(CheckersInteractionRules, turn).unwrap();
        let choice = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| {
                matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(0, 7))
            })
            .unwrap()
            .clone();
        driver.choose(choice.id).unwrap();

        assert_eq!(
            driver
                .turn()
                .working
                .entity(checker)
                .unwrap()
                .state
                .get(KING)
                .and_then(StateValue::as_bool),
            Some(true)
        );
        assert!(driver.turn().steps[0]
            .delta
            .changes
            .iter()
            .any(|change| matches!(change, StateChange::EntityStateChanged { entity, .. } if *entity == checker)));
        assert_eq!(
            driver.turn().steps[0].presentation[0].kind,
            "checkers.promote"
        );
    }

    #[test]
    fn custom_checker_entity_registers_without_core_changes() {
        let state = standard_state();
        let registry = registry();
        let first = *state.entities.keys().next().unwrap();
        let presentation = registry
            .presentation(RuleContext::from_state(&state, None), first)
            .unwrap();
        assert!(presentation.asset_key.starts_with("checkers/"));
    }

    #[test]
    fn no_moves_is_a_ruleset_level_outcome() {
        let mut state = GameState::new(8, 8).unwrap();
        state.add_player(PlayerState::new(WHITE)).unwrap();
        state.add_player(PlayerState::new(BLACK)).unwrap();
        state.set_active_players(vec![WHITE]).unwrap();
        state
            .add_entity(EntityState::new(
                EntityId::new(1),
                CHECKER,
                BLACK,
                Position::new(1, 1),
            ))
            .unwrap();
        let outcome = registry()
            .outcome(RuleContext::from_state(&state, None))
            .unwrap()
            .unwrap();
        assert_eq!(outcome.key, "checkers.no_legal_moves");
        assert_eq!(outcome.winners, vec![BLACK]);
    }

    #[test]
    fn checkers_turn_commits_normally_to_generic_timeline() {
        let (state, _) = chain_state();
        let mut timeline = GameTimeline::new(state).unwrap();
        let turn = timeline.begin_turn(WHITE).unwrap();
        let mut driver = InteractionDriver::new(CheckersInteractionRules, turn).unwrap();
        let first = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectPosition { .. }))
            .unwrap()
            .clone();
        driver.choose(first.id).unwrap();
        let second = driver.interaction().choices[0].clone();
        driver.choose(second.id).unwrap();
        timeline.commit_turn(driver.into_turn().unwrap()).unwrap();
        assert_eq!(timeline.history().len(), 1);
        assert_eq!(timeline.history().previous_turn().unwrap().steps.len(), 2);
    }
}
