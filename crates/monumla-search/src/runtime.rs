//! One search binding for every game using the Monumla interaction protocol.
//! Games supply their normal interaction rules and outcome registry. They do not
//! implement SearchDomain, evaluation, move ordering, or a search adapter.
use crate::{fingerprint, NodeRole, Quiescence, SearchDomain};
use monumla_core::{
    ChoiceInput, GameOutcome, GameState, GameTimeline, InteractionDriver, InteractionError,
    InteractionRules, PlayerId, Position, RuleContext, RuleRegistry, StateChange, TurnInput,
    TurnSession,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
};

type Factory = dyn Fn(&GameTimeline) -> Result<Rc<dyn InteractionRules>, InteractionError>;

#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeAction {
    #[serde(flatten)]
    pub input: TurnInput,
    #[serde(skip)]
    turn: Option<Arc<TurnSession>>,
    #[serde(skip)]
    position_key: Option<u128>,
}
impl PartialEq for RuntimeAction {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input
    }
}
impl std::fmt::Debug for RuntimeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeAction")
            .field("choices", &self.input.choices)
            .finish()
    }
}

/// Rules and choices have exactly the same authority for humans and computers.
/// Caches are bounded and contain complete turns, never frontend selections.
pub struct RuntimeDomain {
    registry: RuleRegistry,
    factory: Rc<Factory>,
    cache: Mutex<HashMap<u128, Vec<RuntimeAction>>>,
    weights: Mutex<HashMap<u128, i32>>,
}
impl RuntimeDomain {
    pub fn new(
        registry: RuleRegistry,
        factory: impl Fn(&GameTimeline) -> Result<Rc<dyn InteractionRules>, InteractionError> + 'static,
    ) -> Self {
        Self {
            registry,
            factory: Rc::new(factory),
            cache: Mutex::new(HashMap::new()),
            weights: Mutex::new(HashMap::new()),
        }
    }
    pub fn outcome(
        &self,
        position: &GameTimeline,
    ) -> Result<Option<GameOutcome>, InteractionError> {
        self.registry
            .outcome(RuleContext::from_state(
                position.current(),
                Some(position.history()),
            ))
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))
    }
    fn driver(
        &self,
        position: &GameTimeline,
    ) -> Result<InteractionDriver<Rc<dyn InteractionRules>>, InteractionError> {
        let player = active(position.current())?;
        InteractionDriver::new((self.factory)(position)?, position.begin_turn(player)?)
    }
    fn replay(
        &self,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<TurnSession, InteractionError> {
        let mut driver = self.driver(position)?;
        for input in &action.input.choices {
            let id = driver
                .interaction()
                .choices
                .iter()
                .find(|choice| ChoiceInput::from(*choice) == *input)
                .ok_or_else(|| {
                    InteractionError::RuleViolation("Action is not legal in this position".into())
                })?
                .id;
            driver.choose(id)?;
        }
        driver.into_turn()
    }
    pub fn action_turn(
        &self,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<TurnSession, InteractionError> {
        if let Some(turn) = &action.turn {
            if action.position_key == Some(fingerprint(position)) {
                return Ok((**turn).clone());
            }
        }
        self.replay(position, action)
    }
    pub fn action_path(
        &self,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<Vec<Position>, InteractionError> {
        let turn = self.action_turn(position, action)?;
        let mut path = Vec::new();
        for step in &turn.steps {
            for change in &step.delta.changes {
                match change {
                    StateChange::EntityMoved { from, to, .. } => {
                        if path.is_empty() {
                            path.push(*from);
                        }
                        path.push(*to);
                    }
                    StateChange::EntityAdded { entity } if path.is_empty() => {
                        path.push(entity.position)
                    }
                    _ => {}
                }
            }
        }
        Ok(path)
    }
    fn enumerate(
        &self,
        position: &GameTimeline,
        mut finish: impl FnMut(RuntimeAction) -> Result<bool, InteractionError>,
    ) -> Result<(), InteractionError> {
        let key = fingerprint(position);
        let mut stack = vec![(self.driver(position)?, Vec::new())];
        let mut visited = HashSet::new();
        let mut outcomes = HashSet::new();
        while let Some((driver, path)) = stack.pop() {
            if path.len() > 128 {
                return Err(InteractionError::RuleViolation(
                    "Turn continuation exceeds the search safety limit".into(),
                ));
            }
            let steps: Vec<_> = driver
                .turn()
                .steps
                .iter()
                .map(|s| (&s.action.kind, &s.action.data))
                .collect();
            let draft_key = fingerprint(&(&driver.turn().working, driver.draft(), steps));
            if !visited.insert(draft_key) {
                continue;
            }
            for choice in &driver.interaction().choices {
                let mut child = driver.clone();
                let mut choices = path.clone();
                choices.push(ChoiceInput::from(choice));
                child.choose(choice.id)?;
                if child.is_finished() {
                    let turn = child.into_turn()?;
                    let semantic: Vec<_> = turn
                        .steps
                        .iter()
                        .map(|step| (&step.action.kind, &step.action.data))
                        .collect();
                    let outcome_key = fingerprint(&(&turn.working, semantic));
                    if outcomes.insert(outcome_key)
                        && !finish(RuntimeAction {
                            input: TurnInput { choices },
                            position_key: Some(key),
                            turn: Some(Arc::new(turn)),
                        })?
                    {
                        return Ok(());
                    }
                } else {
                    stack.push((child, choices));
                }
            }
        }
        Ok(())
    }
    fn losing_declaration(
        &self,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<bool, InteractionError> {
        let turn = action
            .turn
            .as_ref()
            .expect("enumerated action has a cached turn");
        if turn.before.entities != turn.working.entities {
            return Ok(false);
        }
        let outcome = self
            .registry
            .outcome(RuleContext::from_turn(turn, Some(position.history())))
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))?;
        Ok(outcome.is_some_and(|o| o.losers.contains(&turn.actor)))
    }
    fn cache_actions(
        &self,
        position: &GameTimeline,
        key: u128,
        results: Vec<RuntimeAction>,
    ) -> Result<Vec<RuntimeAction>, InteractionError> {
        // Immediate losing declarations (for example resignation) are dominated
        // when an ordinary legal continuation exists. This uses outcomes only.
        let mut losing = Vec::new();
        let mut live = Vec::new();
        for action in results {
            if self.losing_declaration(position, &action)? {
                losing.push(action);
            } else {
                live.push(action);
            }
        }
        let results = if live.is_empty() { losing } else { live };
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= 16 || cache.values().map(Vec::len).sum::<usize>() + results.len() > 2048 {
            cache.clear();
        }
        cache.insert(key, results.clone());
        Ok(results)
    }
    fn learn(&self, position: &GameTimeline) -> Result<(), InteractionError> {
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
            let outcome = self.registry.outcome(RuleContext::from_state(&probe, None));
            if outcome.as_ref().map_or(true, |outcome| {
                outcome
                    .as_ref()
                    .is_some_and(|o| o.losers.contains(&entity.owner))
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
    fn tactical(action: &RuntimeAction) -> bool {
        action.turn.as_ref().is_some_and(|turn| {
            turn.steps
                .iter()
                .flat_map(|step| &step.delta.changes)
                .any(|change| {
                    matches!(
                        change,
                        StateChange::EntityRemoved { .. }
                            | StateChange::EntityTypeChanged { .. }
                            | StateChange::EntityControllerChanged { .. }
                    )
                })
        })
    }
}
impl SearchDomain for RuntimeDomain {
    type Position = GameTimeline;
    type Action = RuntimeAction;
    type Perspective = PlayerId;
    type Error = InteractionError;
    fn prepare(&self, position: &GameTimeline) -> Result<(), Self::Error> {
        self.learn(position)
    }
    fn terminal_score(
        &self,
        position: &GameTimeline,
        perspective: PlayerId,
    ) -> Result<Option<i32>, Self::Error> {
        Ok(self.outcome(position)?.map(|outcome| {
            let team = position
                .current()
                .players
                .get(&perspective)
                .and_then(|p| p.team);
            if outcome.winners.contains(&perspective)
                || team.is_some_and(|t| outcome.winning_teams.contains(&t))
            {
                100_000
            } else if outcome.losers.contains(&perspective)
                || team.is_some_and(|t| outcome.losing_teams.contains(&t))
            {
                -100_000
            } else {
                0
            }
        }))
    }
    fn evaluate(&self, position: &GameTimeline, perspective: PlayerId) -> Result<i32, Self::Error> {
        // The default evaluator uses only runtime features, without piece IDs,
        // state-key names, game-specific values, or board-specific movement.
        // Terminal outcomes always dominate these heuristic control estimates.
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
    fn role(
        &self,
        position: &GameTimeline,
        perspective: PlayerId,
    ) -> Result<NodeRole, Self::Error> {
        Ok(
            if allied(position.current(), active(position.current())?, perspective) {
                NodeRole::Maximize
            } else {
                NodeRole::Minimize
            },
        )
    }
    fn actions(&self, position: &GameTimeline) -> Result<Vec<RuntimeAction>, Self::Error> {
        let key = fingerprint(position);
        if let Some(actions) = self.cache.lock().unwrap().get(&key) {
            return Ok(actions.clone());
        }
        if self.outcome(position)?.is_some() {
            return Ok(Vec::new());
        }
        let mut results = Vec::new();
        self.enumerate(position, |action| {
            results.push(action);
            Ok(true)
        })?;
        self.cache_actions(position, key, results)
    }

    fn play(
        &self,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<GameTimeline, Self::Error> {
        let turn = self.action_turn(position, action)?;
        let mut child = position.clone();
        child.commit_turn(turn)?;
        Ok(child)
    }
    fn position_key(&self, position: &GameTimeline) -> Option<u128> {
        Some(fingerprint(position))
    }
    fn action_key(&self, action: &RuntimeAction) -> Option<u64> {
        let parts: Vec<_> = action
            .input
            .choices
            .iter()
            .map(|c| (&c.data, format!("{:?}", c.kind)))
            .collect();
        Some(fingerprint(&parts) as u64)
    }
    fn action_priority(&self, _position: &GameTimeline, action: &RuntimeAction) -> i32 {
        action.turn.as_ref().map_or(0, |turn| {
            let removed = turn
                .steps
                .iter()
                .flat_map(|s| &s.delta.changes)
                .filter(|c| matches!(c, StateChange::EntityRemoved { .. }))
                .count() as i32;
            let changes = turn
                .steps
                .iter()
                .flat_map(|s| &s.delta.changes)
                .filter(|c| matches!(c, StateChange::EntityTypeChanged { .. }))
                .count() as i32;
            removed * 1000 + changes * 100
        })
    }
    fn allows_stand_pat(&self, position: &GameTimeline) -> Result<Option<bool>, Self::Error> {
        let cached = self
            .cache
            .lock()
            .unwrap()
            .get(&fingerprint(position))
            .cloned();
        if let Some(actions) = cached {
            for action in &actions {
                if !Self::tactical(action) && !self.losing_declaration(position, action)? {
                    return Ok(Some(true));
                }
            }
            return Ok(Some(false));
        }
        let mut witness = false;
        let mut complete = Vec::new();
        self.enumerate(position, |action| {
            if !Self::tactical(&action) && !self.losing_declaration(position, &action)? {
                witness = true;
                Ok(false)
            } else {
                complete.push(action);
                Ok(true)
            }
        })?;
        if !witness {
            self.cache_actions(position, fingerprint(position), complete)?;
        }
        Ok(Some(witness))
    }
    fn quiescence(
        &self,
        position: &GameTimeline,
    ) -> Result<Option<Quiescence<RuntimeAction>>, Self::Error> {
        let all = self.actions(position)?;
        let count = all.len();
        let actions: Vec<_> = all.into_iter().filter(Self::tactical).collect();
        if actions.is_empty() {
            Ok(None)
        } else {
            let stand_pat = actions.len() != count;
            Ok(Some(Quiescence { actions, stand_pat }))
        }
    }
    fn terminal_score_at_ply(
        &self,
        position: &GameTimeline,
        perspective: PlayerId,
        ply: u16,
    ) -> Result<Option<i32>, Self::Error> {
        Ok(self.terminal_score(position, perspective)?.map(|s| {
            if s > 0 {
                s - i32::from(ply)
            } else if s < 0 {
                s + i32::from(ply)
            } else {
                0
            }
        }))
    }
    fn score_to_cache(&self, s: i32, p: u16) -> i32 {
        if s > 90_000 {
            s + i32::from(p)
        } else if s < -90_000 {
            s - i32::from(p)
        } else {
            s
        }
    }
    fn score_from_cache(&self, s: i32, p: u16) -> i32 {
        if s > 90_000 {
            s - i32::from(p)
        } else if s < -90_000 {
            s + i32::from(p)
        } else {
            s
        }
    }
}
fn entity_features(entity: &monumla_core::EntityState) -> u128 {
    fingerprint(&(entity.entity_type, &entity.state))
}
fn active(state: &GameState) -> Result<PlayerId, InteractionError> {
    state
        .turn
        .active_players
        .first()
        .copied()
        .ok_or_else(|| InteractionError::RuleViolation("Position has no active player".into()))
}
fn allied(state: &GameState, a: PlayerId, b: PlayerId) -> bool {
    a == b
        || state
            .players
            .get(&a)
            .and_then(|p| p.team)
            .is_some_and(|team| state.players.get(&b).and_then(|p| p.team) == Some(team))
}

impl From<TurnInput> for RuntimeAction {
    fn from(input: TurnInput) -> Self {
        Self {
            input,
            turn: None,
            position_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze, SearchLimits, SearchOptions};
    use monumla_core::{
        Choice, ChoiceSpec, InteractionFlow, OutcomeRule, PlayerState, RecordedAction, RuleError,
        StateMap, StateValue,
    };

    // A new game provides only the ordinary rules used by a human interface.
    // It has no entities, movement types, evaluator, or search implementation.
    struct TakeAway;
    impl InteractionRules for TakeAway {
        fn choices(
            &self,
            turn: &TurnSession,
            draft: &StateMap,
        ) -> Result<Vec<ChoiceSpec>, InteractionError> {
            if draft.contains_key("take") {
                return Ok(vec![
                    ChoiceSpec::option("confirm"),
                    ChoiceSpec::option("cancel"),
                ]);
            }
            let remaining = turn
                .working
                .ruleset_state
                .get("remaining")
                .and_then(StateValue::as_u64)
                .unwrap();
            Ok((1..=remaining.min(2))
                .map(|n| ChoiceSpec::option(n.to_string()))
                .collect())
        }
        fn apply_choice(
            &self,
            turn: &mut TurnSession,
            draft: &mut StateMap,
            choice: &Choice,
        ) -> Result<InteractionFlow, InteractionError> {
            let monumla_core::ChoiceKind::SelectOption { key } = &choice.kind else {
                unreachable!()
            };
            if key == "cancel" {
                draft.remove("take");
                return Ok(InteractionFlow::Continue);
            }
            if key != "confirm" {
                draft.insert("take", key.parse::<u64>().unwrap());
                return Ok(InteractionFlow::Continue);
            }
            let take = draft.get("take").and_then(StateValue::as_u64).unwrap();
            let actor = turn.actor;
            turn.apply_step(RecordedAction::new("take"), |state| {
                let remaining = state
                    .ruleset_state
                    .get("remaining")
                    .and_then(StateValue::as_u64)
                    .unwrap();
                state.ruleset_state.insert("remaining", remaining - take);
                state.ruleset_state.insert("last", u64::from(actor.get()));
                state.set_active_players(vec![PlayerId::new(if actor.get() == 1 { 2 } else { 1 })])
            })?;
            Ok(InteractionFlow::FinishTurn)
        }
    }
    impl OutcomeRule for TakeAway {
        fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
            let state = context.state();
            if state
                .ruleset_state
                .get("remaining")
                .and_then(StateValue::as_u64)
                != Some(0)
            {
                return Ok(None);
            }
            let winner = state
                .ruleset_state
                .get("last")
                .and_then(StateValue::as_u64)
                .unwrap() as u32;
            Ok(Some(
                GameOutcome::new("last-take")
                    .with_winner(PlayerId::new(winner))
                    .with_loser(PlayerId::new(if winner == 1 { 2 } else { 1 })),
            ))
        }
    }
    fn game() -> (RuntimeDomain, GameTimeline) {
        let mut state = GameState::new(1, 1).unwrap();
        for id in 1..=2 {
            state
                .add_player(PlayerState::new(PlayerId::new(id)))
                .unwrap();
        }
        state.set_active_players(vec![PlayerId::new(1)]).unwrap();
        state.ruleset_state.insert("remaining", 4_u64);
        let mut registry = RuleRegistry::new();
        registry.register_outcome(TakeAway);
        (
            RuntimeDomain::new(registry, |_| Ok(Rc::new(TakeAway))),
            GameTimeline::new(state).unwrap(),
        )
    }
    #[test]
    fn searches_new_rules_without_any_game_search_code_and_resolves_drafts() {
        let (domain, position) = game();
        let before = position.clone();
        let actions = domain.actions(&position).unwrap();
        assert_eq!(actions.len(), 2); // Cancel/selection cycles do not duplicate turns.
        assert!(actions.iter().all(|a| a.input.choices.len() == 2));
        let result = analyze(
            &domain,
            &position,
            PlayerId::new(1),
            SearchOptions {
                limits: SearchLimits {
                    max_depth: 4,
                    max_nodes: Some(1000),
                },
                multi_pv: 2,
                ..Default::default()
            },
            || false,
            |_| {},
        )
        .unwrap();
        assert!(result.score.unwrap() > 90_000);
        assert_eq!(result.variations.len(), 2);
        assert!(result.variations[0].score > result.variations[1].score);
        let best = result.best_action.unwrap();
        let replayable = RuntimeAction::from(best.input.clone());
        let next = domain.play(&position, &replayable).unwrap();
        assert_eq!(
            next.current()
                .ruleset_state
                .get("remaining")
                .and_then(StateValue::as_u64),
            Some(3)
        );
        assert_eq!(position, before);
        assert_eq!(domain.play(&position, &best).unwrap(), next);
    }
    #[test]
    fn same_board_with_different_history_has_different_identity() {
        let (domain, position) = game();
        let mut with_history = position.clone();
        let mut turn = position.begin_turn(PlayerId::new(1)).unwrap();
        turn.apply_step(RecordedAction::new("history-relevant-event"), |_| Ok(()))
            .unwrap();
        with_history.commit_turn(turn).unwrap();
        assert_eq!(with_history.current(), position.current());
        assert_ne!(
            domain.position_key(&with_history),
            domain.position_key(&position)
        );
    }
    struct WitnessRules {
        calls: Rc<std::cell::Cell<usize>>,
        quiet: bool,
    }
    impl InteractionRules for WitnessRules {
        fn choices(
            &self,
            _: &TurnSession,
            _: &StateMap,
        ) -> Result<Vec<ChoiceSpec>, InteractionError> {
            let mut choices = vec![ChoiceSpec::option(if self.quiet {
                "quiet"
            } else {
                "resign"
            })];
            choices.extend((0..40).map(|n| ChoiceSpec::option(format!("capture-{n}"))));
            Ok(choices)
        }
        fn apply_choice(
            &self,
            turn: &mut TurnSession,
            _: &mut StateMap,
            choice: &Choice,
        ) -> Result<InteractionFlow, InteractionError> {
            self.calls.set(self.calls.get() + 1);
            let monumla_core::ChoiceKind::SelectOption { key } = &choice.kind else {
                unreachable!()
            };
            turn.apply_step(RecordedAction::new(key.clone()), |state| {
                if key.starts_with("capture-") {
                    state.remove_entity(monumla_core::EntityId::new(2))?;
                } else if key == "resign" {
                    state.ruleset_state.insert("loser", 1_u64);
                }
                state.set_active_players(vec![PlayerId::new(2)])
            })?;
            Ok(InteractionFlow::FinishTurn)
        }
    }
    struct WitnessOutcome;
    impl OutcomeRule for WitnessOutcome {
        fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
            Ok(context
                .state()
                .ruleset_state
                .contains_key("loser")
                .then(|| {
                    GameOutcome::new("declaration")
                        .with_loser(PlayerId::new(1))
                        .with_winner(PlayerId::new(2))
                }))
        }
    }
    fn witness_game(quiet: bool) -> (RuntimeDomain, GameTimeline, Rc<std::cell::Cell<usize>>) {
        let (_, position) = game();
        let mut state = position.current().clone();
        state.board = monumla_core::Board::new(2, 2).unwrap();
        for id in 1..=2 {
            state
                .add_entity(monumla_core::EntityState::new(
                    monumla_core::EntityId::new(id),
                    monumla_core::EntityTypeId::new(1),
                    PlayerId::new(id),
                    Position::new((id - 1) as u16, (id - 1) as u16),
                ))
                .unwrap();
        }
        let calls = Rc::new(std::cell::Cell::new(0));
        let count = calls.clone();
        let mut registry = RuleRegistry::new();
        registry.register_outcome(WitnessOutcome);
        (
            RuntimeDomain::new(registry, move |_| {
                Ok(Rc::new(WitnessRules {
                    calls: count.clone(),
                    quiet,
                }))
            }),
            GameTimeline::new(state).unwrap(),
            calls,
        )
    }
    #[test]
    fn quiet_witness_stops_early_and_never_caches_an_incomplete_action_list() {
        let (domain, position, calls) = witness_game(true);
        let before = position.clone();
        assert_eq!(domain.allows_stand_pat(&position).unwrap(), Some(true));
        assert_eq!(calls.get(), 1);
        assert_eq!(domain.actions(&position).unwrap().len(), 41);
        assert_eq!(calls.get(), 42);
        assert_eq!(domain.allows_stand_pat(&position).unwrap(), Some(true));
        assert_eq!(calls.get(), 42);
        assert_eq!(position, before);
    }
    #[test]
    fn a_losing_declaration_is_not_a_quiet_witness_for_forced_captures() {
        let (domain, position, _) = witness_game(false);
        assert_eq!(domain.allows_stand_pat(&position).unwrap(), Some(false));
        assert_eq!(domain.actions(&position).unwrap().len(), 40);
        assert!(!domain.quiescence(&position).unwrap().unwrap().stand_pat);
        assert_eq!(domain.allows_stand_pat(&position).unwrap(), Some(false));
    }
}
