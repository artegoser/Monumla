use crate::{is_king, promotion_row, DraughtsVariant, CHECKER, FORCED, KING, PENDING, WHITE};
use monumla_core::{EntityId, GameState, InteractionError, PlayerId, Position, StateValue};

const DIAGONALS: [(i16, i16); 4] = [(-1, 1), (1, 1), (-1, -1), (1, -1)];

/// One complete turn, including every landing and victim in capture order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraughtsMove {
    pub actor: EntityId,
    pub landings: Vec<Position>,
    pub captures: Vec<EntityId>,
}

pub(crate) fn pending(state: &GameState) -> Vec<EntityId> {
    match state.ruleset_state.get(PENDING) {
        Some(StateValue::List(values)) => values
            .iter()
            .filter_map(|v| {
                v.as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .map(EntityId::new)
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn forced(state: &GameState) -> Option<EntityId> {
    state
        .ruleset_state
        .get(FORCED)
        .and_then(StateValue::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .map(EntityId::new)
}

fn offset(state: &GameState, position: Position, dx: i16, dy: i16) -> Option<Position> {
    let x = i32::from(position.x) + i32::from(dx);
    let y = i32::from(position.y) + i32::from(dy);
    if x < 0 || y < 0 || x >= i32::from(state.board.width()) || y >= i32::from(state.board.height())
    {
        return None;
    }
    Some(Position::new(x as u16, y as u16))
}

/// Victims remain occupied until the complete turn ends (the Turkish strike rule).
fn capture_steps(
    state: &GameState,
    actor: EntityId,
    captured: &[EntityId],
) -> Result<Vec<(Position, EntityId)>, InteractionError> {
    let entity = state.entity(actor)?;
    let flying = is_king(entity);
    let mut steps = Vec::new();
    for (dx, dy) in DIAGONALS {
        let mut at = entity.position;
        let mut victim = None;
        while let Some(to) = offset(state, at, dx, dy) {
            if let Some(other) = state.entity_at(to)? {
                if victim.is_some() || other.owner == entity.owner || captured.contains(&other.id) {
                    break;
                }
                victim = Some(other.id);
            } else if let Some(victim) = victim {
                steps.push((to, victim));
                if !flying {
                    break;
                }
            } else if !flying {
                break;
            }
            at = to;
        }
    }
    Ok(steps)
}

fn capture_paths(
    state: &GameState,
    path: DraughtsMove,
    captured: &mut Vec<EntityId>,
    output: &mut Vec<DraughtsMove>,
) -> Result<(), InteractionError> {
    let steps = capture_steps(state, path.actor, captured)?;
    if steps.is_empty() {
        if !path.captures.is_empty() {
            output.push(path);
        }
        return Ok(());
    }
    for (to, victim) in steps {
        let mut next = state.clone();
        next.move_entity(path.actor, to)?;
        if DraughtsVariant::from_state(state) == DraughtsVariant::Russian
            && promotion_row(&next, next.entity(path.actor)?)
        {
            next.entity_mut(path.actor)?.state.insert(KING, true);
        }
        let mut branch = path.clone();
        branch.landings.push(to);
        branch.captures.push(victim);
        captured.push(victim);
        capture_paths(&next, branch, captured, output)?;
        captured.pop();
    }
    Ok(())
}

/// Legal complete turns. International draughts keeps only longest capture paths;
/// Russian draughts permits every complete path, regardless of material captured.
pub fn legal_moves(
    state: &GameState,
    player: PlayerId,
) -> Result<Vec<DraughtsMove>, InteractionError> {
    let forced = forced(state);
    let mut captured = pending(state);
    let actors: Vec<_> = state
        .entities
        .values()
        .filter(|e| {
            e.controller == player
                && e.entity_type == CHECKER
                && !captured.contains(&e.id)
                && forced.map_or(true, |id| id == e.id)
        })
        .map(|e| e.id)
        .collect();
    let mut captures = Vec::new();
    for &actor in &actors {
        capture_paths(
            state,
            DraughtsMove {
                actor,
                landings: Vec::new(),
                captures: Vec::new(),
            },
            &mut captured,
            &mut captures,
        )?;
    }
    if !captures.is_empty() {
        if DraughtsVariant::from_state(state) == DraughtsVariant::International {
            let maximum = captures.iter().map(|m| m.captures.len()).max().unwrap_or(0);
            captures.retain(|m| m.captures.len() == maximum);
        }
        return Ok(captures);
    }
    if forced.is_some() {
        return Ok(Vec::new());
    }
    let mut moves = Vec::new();
    for actor in actors {
        let entity = state.entity(actor)?;
        for (dx, dy) in DIAGONALS {
            if !is_king(entity) && dy != if entity.owner == WHITE { 1 } else { -1 } {
                continue;
            }
            let mut at = entity.position;
            while let Some(to) = offset(state, at, dx, dy) {
                if state.entity_at(to)?.is_some() {
                    break;
                }
                moves.push(DraughtsMove {
                    actor,
                    landings: vec![to],
                    captures: Vec::new(),
                });
                if !is_king(entity) {
                    break;
                }
                at = to;
            }
        }
    }
    Ok(moves)
}

/// Execute a complete legal turn through the same authority used by browser input.
/// Invalid or incomplete paths never mutate the supplied position.
pub fn play_move(
    state: &GameState,
    movement: &DraughtsMove,
) -> Result<monumla_core::TurnSession, InteractionError> {
    use monumla_core::{ChoiceKind, InteractionDriver, TurnSession};
    let player = state.entity(movement.actor)?.controller;
    if state.turn.active_players.as_slice() != [player]
        || !legal_moves(state, player)?.contains(movement)
    {
        return Err(InteractionError::RuleViolation(
            "illegal complete draughts turn".into(),
        ));
    }
    let mut driver = InteractionDriver::new(
        crate::CheckersInteractionRules,
        TurnSession::new(state, player)?,
    )?;
    for &landing in &movement.landings {
        let choice = driver
            .interaction()
            .choices
            .iter()
            .find(|c| {
                matches!(c.kind, ChoiceKind::SelectPosition { position } if position == landing)
                    && c.data.get("actor").and_then(StateValue::as_u64)
                        == Some(u64::from(movement.actor.get()))
            })
            .ok_or_else(|| {
                InteractionError::RuleViolation("capture path cannot continue".into())
            })?;
        driver.choose(choice.id)?;
    }
    driver.into_turn()
}
