//! A draughts adapter for the game-independent core search.
use crate::{is_king, legal_moves, play_move, CheckersOutcomeRule, DraughtsMove, WHITE};
use monumla_core::{
    search::{NodeRole, SearchDomain},
    GameTimeline, InteractionError, OutcomeRule, PlayerId, RuleContext,
};

#[derive(Default)]
pub struct DraughtsSearch;
impl SearchDomain for DraughtsSearch {
    type Position = GameTimeline;
    type Action = DraughtsMove;
    type Perspective = PlayerId;
    type Error = InteractionError;

    fn terminal_score(
        &self,
        position: &GameTimeline,
        perspective: PlayerId,
    ) -> Result<Option<i32>, Self::Error> {
        let outcome = CheckersOutcomeRule
            .evaluate(RuleContext::from_state(
                position.current(),
                Some(position.history()),
            ))
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))?;
        Ok(outcome.map(|outcome| {
            if outcome.winners.contains(&perspective) {
                100_000
            } else if outcome.losers.contains(&perspective) {
                -100_000
            } else {
                0
            }
        }))
    }
    fn evaluate(&self, position: &GameTimeline, perspective: PlayerId) -> Result<i32, Self::Error> {
        // Material and advancement belong to the game, never to the search core.
        Ok(position
            .current()
            .entities
            .values()
            .map(|entity| {
                let value = if is_king(entity) {
                    300
                } else {
                    let progress = if entity.owner == WHITE {
                        entity.position.y
                    } else {
                        position.current().board.height() - 1 - entity.position.y
                    };
                    100 + i32::from(progress) * 3
                };
                if entity.owner == perspective {
                    value
                } else {
                    -value
                }
            })
            .sum())
    }
    fn role(
        &self,
        position: &GameTimeline,
        perspective: PlayerId,
    ) -> Result<NodeRole, Self::Error> {
        Ok(if active(position)? == perspective {
            NodeRole::Maximize
        } else {
            NodeRole::Minimize
        })
    }
    fn actions(&self, position: &GameTimeline) -> Result<Vec<DraughtsMove>, Self::Error> {
        if self.terminal_score(position, active(position)?)?.is_some() {
            return Ok(Vec::new());
        }
        legal_moves(position.current(), active(position)?)
    }
    fn play(
        &self,
        position: &GameTimeline,
        action: &DraughtsMove,
    ) -> Result<GameTimeline, Self::Error> {
        let turn = play_move(position.current(), action)?;
        let mut child = position.clone();
        child.commit_turn(turn)?;
        Ok(child)
    }
}
fn active(position: &GameTimeline) -> Result<PlayerId, InteractionError> {
    match position.current().turn.active_players.as_slice() {
        [player] => Ok(*player),
        _ => Err(InteractionError::RuleViolation(
            "draughts requires one active player".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{state_for_variant, DraughtsVariant, BLACK, CHECKER};
    use monumla_core::{
        search::{search, SearchLimits},
        EntityId, EntityState, Position,
    };

    #[test]
    fn search_returns_a_complete_winning_capture_in_both_variants_without_mutating_root() {
        for variant in [DraughtsVariant::Russian, DraughtsVariant::International] {
            let mut state = state_for_variant(variant);
            for id in state.entities.keys().copied().collect::<Vec<_>>() {
                state.remove_entity(id).unwrap();
            }
            for (id, side, x, y) in [(1, WHITE, 0, 0), (2, BLACK, 1, 1), (3, BLACK, 3, 3)] {
                state
                    .add_entity(EntityState::new(
                        EntityId::new(id),
                        CHECKER,
                        side,
                        Position::new(x, y),
                    ))
                    .unwrap();
            }
            let timeline = GameTimeline::new(state).unwrap();
            let before = timeline.clone();
            let result = search(
                &DraughtsSearch,
                &timeline,
                WHITE,
                SearchLimits {
                    max_depth: 2,
                    max_nodes: Some(100),
                },
            )
            .unwrap();
            let movement = result.best_action.unwrap();
            assert_eq!(movement.captures.len(), 2);
            assert_eq!(result.score, Some(100_000));
            let child = DraughtsSearch.play(&timeline, &movement).unwrap();
            assert_eq!(child.history().len(), 1);
            assert_eq!(child.history().previous_turn().unwrap().steps.len(), 2);
            assert_eq!(timeline, before);
        }
    }
    #[test]
    fn search_respects_history_dependent_draws() {
        let mut state = state_for_variant(DraughtsVariant::Russian);
        for id in state.entities.keys().copied().collect::<Vec<_>>() {
            state.remove_entity(id).unwrap();
        }
        for (id, side, x, y) in [(1, WHITE, 0, 0), (2, BLACK, 7, 5)] {
            let mut entity =
                EntityState::new(EntityId::new(id), CHECKER, side, Position::new(x, y));
            entity.state.insert(crate::KING, true);
            state.add_entity(entity).unwrap();
        }
        let mut timeline = GameTimeline::new(state).unwrap();
        for _ in 0..2 {
            for to in [
                Position::new(1, 1),
                Position::new(6, 4),
                Position::new(0, 0),
                Position::new(7, 5),
            ] {
                let movement = DraughtsSearch
                    .actions(&timeline)
                    .unwrap()
                    .into_iter()
                    .find(|m| m.landings == [to])
                    .unwrap();
                timeline = DraughtsSearch.play(&timeline, &movement).unwrap();
            }
        }
        let result = search(&DraughtsSearch, &timeline, WHITE, SearchLimits::default()).unwrap();
        assert_eq!(result.best_action, None);
        assert_eq!(result.score, Some(0));
        assert_eq!(
            result.stop_reason,
            monumla_core::search::StopReason::Terminal
        );
    }
}
