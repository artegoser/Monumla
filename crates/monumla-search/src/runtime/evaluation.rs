//! Rule-probed entity utilities and the default feature estimate.
use super::*;

impl RuntimeDomain {
    /// Inferred control utility for diagnostics; no game-specific values.
    pub fn inferred_value(&self, entity: &monumla_core::EntityState) -> Option<i32> {
        self.weights
            .lock()
            .unwrap()
            .get(&entity_features(entity))
            .copied()
    }
    pub(super) fn learn(&self, position: &GameTimeline) -> Result<(), InteractionError> {
        let state = position.current();
        if state.entities.values().all(|e| {
            self.weights
                .lock()
                .unwrap()
                .contains_key(&entity_features(e))
        }) {
            return Ok(());
        }
        let mut critical = HashSet::new();
        for entity in state.entities.values() {
            let mut probe = state.clone();
            probe.remove_entity(entity.id)?;
            // Outcome rules may examine only the active controller. Query the
            // removed entity's controller so every side's essential entities
            // survive capability probes, independently of storage/ID order.
            probe.set_active_players(vec![entity.controller])?;
            let outcome = self.registry.outcome(RuleContext::from_state(&probe, None));
            if outcome.as_ref().map_or(true, |outcome| {
                outcome.as_ref().is_some_and(|o| {
                    o.losers.contains(&entity.controller) || o.losers.contains(&entity.owner)
                })
            }) {
                critical.insert(entity.id);
            }
        }
        let mut weights = self.weights.lock().unwrap().clone();
        for entity in state.entities.values() {
            if weights.contains_key(&entity_features(entity)) {
                continue;
            }
            let mut isolated = state.clone();
            for id in state.entities.keys() {
                if *id != entity.id && !critical.contains(id) {
                    isolated.remove_entity(*id)?;
                }
            }
            isolated.set_active_players(vec![entity.controller])?;
            // Removing other entities may itself end the game. Preserve enough
            // context for ordinary rules to expose the sample's capabilities.
            // Prefer another controller before adding supporting entities.
            let mut helpers: Vec<_> = state
                .entities
                .values()
                .filter(|e| !isolated.entities.contains_key(&e.id))
                .collect();
            helpers.sort_by_key(|e| (e.controller == entity.controller, e.id));
            let mut helpers = helpers.into_iter();
            while !matches!(
                self.registry
                    .outcome(RuleContext::from_state(&isolated, None)),
                Ok(None)
            ) {
                let Some(helper) = helpers.next() else {
                    break;
                };
                isolated.add_entity(helper.clone())?;
            }
            let mut mobility = 0;
            for point in [
                entity.position,
                Position::new(state.board.width() / 2, state.board.height() / 2),
                Position::new(
                    (state.board.width() / 2).saturating_sub(1),
                    (state.board.height() / 2).saturating_sub(1),
                ),
            ] {
                let mut probe = isolated.clone();
                if probe.entity_at(point)?.is_some_and(|e| e.id != entity.id) {
                    continue;
                }
                if probe.move_entity(entity.id, point).is_err() {
                    continue;
                }
                let timeline = GameTimeline::new(probe)?;
                if let Ok(rules) = (self.factory)(&timeline) {
                    if let Ok(turn) = timeline.begin_turn(entity.controller) {
                        if let Ok(choices) = rules.choices(&turn, &monumla_core::StateMap::new()) {
                            let count = choices
                                .iter()
                                .filter(|choice| {
                                    choice
                                        .data
                                        .get("actor")
                                        .and_then(monumla_core::StateValue::as_u64)
                                        == Some(u64::from(entity.id.get()))
                                        && matches!(
                                            choice.kind,
                                            monumla_core::ChoiceKind::SelectPosition { .. }
                                        )
                                })
                                .count();
                            mobility = mobility.max(count);
                        }
                    }
                }
            }
            weights.insert(
                entity_features(entity),
                100 + (mobility.min(32) as i32) * 35,
            );
        }
        *self.weights.lock().unwrap() = weights;
        Ok(())
    }
    pub(super) fn evaluate_features(
        &self,
        position: &GameTimeline,
        perspective: PlayerId,
    ) -> Result<i32, InteractionError> {
        // The default evaluator uses only runtime features, without piece IDs,
        // state-key names, game-specific values, or board-specific movement.
        // Terminal outcomes always dominate these heuristic control estimates.
        if let Some(score) = self
            .registry
            .position_score(
                RuleContext::from_state(position.current(), Some(position.history())),
                perspective,
            )
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))?
        {
            return Ok(score.clamp(-20_000, 20_000));
        }
        self.learn(position)?;
        let state = position.current();
        let mut score = 0_i32;
        let weights = self.weights.lock().unwrap();
        for entity in state.entities.values() {
            let value = weights
                .get(&entity_features(entity))
                .copied()
                .unwrap_or(100);
            let x = i32::from(entity.position.x);
            let y = i32::from(entity.position.y);
            let center = (i32::from(state.board.width())
                - 1
                - (2 * x - i32::from(state.board.width()) + 1).abs())
                + (i32::from(state.board.height())
                    - 1
                    - (2 * y - i32::from(state.board.height()) + 1).abs());
            let value = value + center * 2;
            score += if allied(state, entity.controller, perspective) {
                value
            } else {
                -value
            };
        }
        drop(weights);
        let rules = (self.factory)(position)?;
        let turn = position.begin_turn(active(state)?)?;
        let choices = rules.choices(&turn, &monumla_core::StateMap::new())?;
        let mobility = (choices.len().min(60) as i32).min(80);
        score += if allied(state, active(state)?, perspective) {
            mobility
        } else {
            -mobility
        };
        Ok(score.clamp(-20_000, 20_000))
    }
}
pub(super) fn entity_features(entity: &monumla_core::EntityState) -> u128 {
    fingerprint(&(entity.entity_type, &entity.state))
}
pub(super) fn allied(state: &GameState, a: PlayerId, b: PlayerId) -> bool {
    a == b
        || state
            .players
            .get(&a)
            .and_then(|p| p.team)
            .is_some_and(|team| state.players.get(&b).and_then(|p| p.team) == Some(team))
}
