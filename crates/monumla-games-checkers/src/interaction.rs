use crate::{
    is_king, legal_moves,
    moves::{forced, pending},
    outcome::update_draw_counters,
    promotion_row, DraughtsVariant, BLACK, FORCED, KING, PENDING, WHITE,
};
use monumla_core::{
    Choice, ChoiceKind, ChoiceSpec, EntityId, InteractionError, InteractionFlow, InteractionRules,
    PlayerId, PresentationCue, RecordedAction, StateMap, StateValue, TurnSession,
};

const SELECTED: &str = "checkers.selected";
fn entity_id(data: &StateMap, key: &str) -> Option<EntityId> {
    data.get(key)
        .and_then(StateValue::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .map(EntityId::new)
}
fn reject(message: &str) -> InteractionError {
    InteractionError::RuleViolation(message.into())
}

pub(crate) fn active_player(turn: &TurnSession) -> Result<PlayerId, InteractionError> {
    match turn.working.turn.active_players.as_slice() {
        [player] if *player == WHITE || *player == BLACK => Ok(*player),
        _ => Err(reject("draughts requires one active white or black player")),
    }
}

pub struct CheckersInteractionRules;
impl CheckersInteractionRules {
    pub fn selected_entity(draft: &StateMap) -> Option<EntityId> {
        entity_id(draft, SELECTED)
    }
}
impl InteractionRules for CheckersInteractionRules {
    fn choices(
        &self,
        turn: &TurnSession,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let legal = legal_moves(&turn.working, active_player(turn)?)?;
        let selected = forced(&turn.working).or_else(|| Self::selected_entity(draft));
        let mut choices = Vec::new();
        if forced(&turn.working).is_none() {
            for movement in &legal {
                let choice = ChoiceSpec::entity(movement.actor);
                if !choices.contains(&choice) {
                    choices.push(choice);
                }
            }
        }
        for movement in legal {
            if selected.is_some() && selected != Some(movement.actor) {
                continue;
            }
            let mut choice = ChoiceSpec::position(movement.landings[0]);
            choice.data.insert("actor", u64::from(movement.actor.get()));
            if let Some(capture) = movement.captures.first() {
                choice.data.insert("capture", u64::from(capture.get()));
            }
            if !choices.contains(&choice) {
                choices.push(choice);
            }
        }
        Ok(choices)
    }

    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        let player = active_player(turn)?;
        let legal = legal_moves(&turn.working, player)?;
        match choice.kind {
            ChoiceKind::SelectEntity { entity } => {
                if forced(&turn.working).is_some() || !legal.iter().any(|m| m.actor == entity) {
                    return Err(reject("checker cannot be selected"));
                }
                draft.insert(SELECTED, u64::from(entity.get()));
                Ok(InteractionFlow::Continue)
            }
            ChoiceKind::SelectPosition { position } => {
                let actor =
                    entity_id(&choice.data, "actor").ok_or_else(|| reject("move has no actor"))?;
                if Self::selected_entity(draft).is_some_and(|selected| selected != actor) {
                    return Err(reject("destination belongs to another checker"));
                }
                let movement = legal
                    .iter()
                    .find(|m| {
                        m.actor == actor
                            && m.landings[0] == position
                            && m.captures.first().copied() == entity_id(&choice.data, "capture")
                    })
                    .ok_or_else(|| reject("illegal draughts move"))?;
                let victim = movement.captures.first().copied();
                let before = turn.before.clone();
                let was_king = is_king(before.entity(actor)?);
                let variant = DraughtsVariant::from_state(&turn.working);
                let mut action = RecordedAction::new("checkers.move");
                action.data = choice.data.clone();
                action.data.insert("to_x", u64::from(position.x));
                action.data.insert("to_y", u64::from(position.y));
                let continues = turn
                    .apply_transaction(action, |tx| -> Result<bool, InteractionError> {
                        tx.move_entity(actor, position)?;
                        let mut captured = pending(tx.state());
                        if let Some(victim) = victim {
                            captured.push(victim);
                            tx.raw_state_mut().ruleset_state.insert(
                                PENDING,
                                StateValue::List(
                                    captured
                                        .iter()
                                        .map(|id| StateValue::U64(u64::from(id.get())))
                                        .collect(),
                                ),
                            );
                            tx.raw_state_mut()
                                .ruleset_state
                                .insert(FORCED, u64::from(actor.get()));
                        }
                        // Russian men become flying kings during the capture sequence.
                        if variant == DraughtsVariant::Russian {
                            promote(tx, actor)?;
                        }
                        let continues =
                            victim.is_some() && !legal_moves(tx.state(), player)?.is_empty();
                        if !continues {
                            for victim in &captured {
                                tx.remove_entity(*victim)?;
                            }
                            tx.raw_state_mut().ruleset_state.remove(PENDING);
                            tx.raw_state_mut().ruleset_state.remove(FORCED);
                            // International men crown only if they finish on the last row.
                            promote(tx, actor)?;
                            let promoted = !was_king && is_king(tx.state().entity(actor)?);
                            update_draw_counters(
                                &before,
                                tx.raw_state_mut(),
                                player,
                                was_king,
                                !captured.is_empty(),
                                promoted,
                            );
                            tx.raw_state_mut()
                                .set_active_players(vec![if player == WHITE {
                                    BLACK
                                } else {
                                    WHITE
                                }])?;
                        }
                        Ok(continues)
                    })?
                    .value;
                if continues {
                    draft.insert(SELECTED, u64::from(actor.get()));
                    Ok(InteractionFlow::Continue)
                } else {
                    draft.remove(SELECTED);
                    Ok(InteractionFlow::FinishTurn)
                }
            }
            _ => Err(reject("unexpected draughts choice")),
        }
    }
}

fn promote(tx: &mut monumla_core::Transaction, actor: EntityId) -> Result<(), InteractionError> {
    let entity = tx.state().entity(actor)?;
    if !is_king(entity) && promotion_row(tx.state(), entity) {
        tx.entity_mut(actor)?.state.insert(KING, true);
        let mut data = StateMap::new();
        data.insert("entity", u64::from(actor.get()));
        tx.present(PresentationCue::new("checkers.promote").with_data(data));
    }
    Ok(())
}
