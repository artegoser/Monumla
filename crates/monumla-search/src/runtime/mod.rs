//! One search binding for every game using the Monumla interaction protocol.
//! Games supply their normal interaction rules and outcome registry. They do not
//! implement SearchDomain, move ordering, or a search adapter. Optional position
//! estimates and hidden-information constraints are ordinary ruleset knowledge.
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

mod evaluation;
mod generation;
use evaluation::{allied, entity_features};

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
impl RuntimeAction {
    pub fn from_input(input: TurnInput) -> Self {
        Self {
            input,
            turn: None,
            position_key: None,
        }
    }
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
    drivers: Mutex<HashMap<u128, InteractionDriver<Rc<dyn InteractionRules>>>>,
    outcomes: Mutex<HashMap<u128, Option<GameOutcome>>>,
    cache: Mutex<HashMap<u128, Vec<RuntimeAction>>>,
    weights: Mutex<HashMap<u128, i32>>,
    pub information_memory: std::cell::RefCell<crate::InformationMemory<RuntimeAction>>,
}
impl RuntimeDomain {
    pub fn new(
        registry: RuleRegistry,
        factory: impl Fn(&GameTimeline) -> Result<Rc<dyn InteractionRules>, InteractionError> + 'static,
    ) -> Self {
        Self {
            registry,
            factory: Rc::new(factory),
            drivers: Mutex::new(HashMap::new()),
            outcomes: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            weights: Mutex::new(HashMap::new()),
            information_memory: std::cell::RefCell::new(crate::InformationMemory::new()),
        }
    }
    pub fn outcome(
        &self,
        position: &GameTimeline,
    ) -> Result<Option<GameOutcome>, InteractionError> {
        let key = position.semantic_digest();
        if let Some(outcome) = self.outcomes.lock().unwrap().get(&key) {
            return Ok(outcome.clone());
        }
        let outcome = self
            .registry
            .outcome(RuleContext::from_state(
                position.current(),
                Some(position.history()),
            ))
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))?;
        let mut outcomes = self.outcomes.lock().unwrap();
        if outcomes.len() >= 256 {
            outcomes.clear();
        }
        outcomes.insert(key, outcome.clone());
        Ok(outcome)
    }
    fn driver(
        &self,
        position: &GameTimeline,
    ) -> Result<InteractionDriver<Rc<dyn InteractionRules>>, InteractionError> {
        let key = position.semantic_digest();
        if let Some(driver) = self.drivers.lock().unwrap().get(&key) {
            return Ok(driver.clone());
        }
        let player = active(position.current())?;
        let driver =
            InteractionDriver::new((self.factory)(position)?, position.begin_turn(player)?)?;
        let mut drivers = self.drivers.lock().unwrap();
        if drivers.len() >= 16 {
            drivers.clear();
        }
        drivers.insert(key, driver.clone());
        Ok(driver)
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
            if action.position_key == Some(position.semantic_digest()) {
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
        if self.has_hidden_information() {
            return Err(InteractionError::RuleViolation("Hidden-information rules require observation sampling, not perfect-information search".into()));
        }
        if self.registry.has_position_score() {
            Ok(())
        } else {
            self.learn(position)
        }
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
        self.evaluate_features(position, perspective)
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
        let key = position.semantic_digest();
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
        Some(position.semantic_digest())
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
            let weights = self.weights.lock().unwrap();
            let utility = |entity: &monumla_core::EntityState| {
                weights
                    .get(&entity_features(entity))
                    .copied()
                    .unwrap_or(100)
            };
            let mut gain = 0;
            let mut cost = 0;
            for change in turn.steps.iter().flat_map(|step| &step.delta.changes) {
                match change {
                    StateChange::EntityRemoved { entity } => {
                        gain += if allied(&turn.before, entity.controller, turn.actor) {
                            -utility(entity)
                        } else {
                            utility(entity)
                        };
                    }
                    StateChange::EntityMoved { entity, .. } => {
                        if let Some(entity) = turn.before.entities.get(entity) {
                            cost = cost.max(utility(entity));
                        }
                    }
                    StateChange::EntityTypeChanged { .. } => {
                        gain += 100;
                    }
                    _ => {}
                }
            }
            if gain == 0 {
                0
            } else {
                gain * 16 - cost
            }
        })
    }
    fn can_reduce(&self, _position: &GameTimeline, action: &RuntimeAction) -> bool {
        !Self::tactical(action)
            && action.turn.as_ref().is_some_and(|turn| {
                turn.steps.len() == 1
                    && !turn.working.turn.active_players.contains(&turn.actor)
                    && turn.before.entities != turn.working.entities
            })
    }
    fn allows_stand_pat(&self, position: &GameTimeline) -> Result<Option<bool>, Self::Error> {
        let cached = self
            .cache
            .lock()
            .unwrap()
            .get(&position.semantic_digest())
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
            self.cache_actions(position, position.semantic_digest(), complete)?;
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
fn active(state: &GameState) -> Result<PlayerId, InteractionError> {
    state
        .turn
        .active_players
        .first()
        .copied()
        .ok_or_else(|| InteractionError::RuleViolation("Position has no active player".into()))
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
mod tests;

impl RuntimeDomain {
    pub fn has_hidden_information(&self) -> bool {
        self.registry.information().is_some()
    }
    pub fn observation(
        &self,
        position: &GameTimeline,
        viewer: PlayerId,
    ) -> Result<GameTimeline, InteractionError> {
        if let Some(rule) = self.registry.information() {
            let state = rule
                .observe(position.current(), viewer)
                .map_err(|e| InteractionError::RuleViolation(e.to_string()))?;
            // No historical authority or redo snapshots cross this boundary.
            Ok(GameTimeline::new(state)?)
        } else {
            Ok(position.clone())
        }
    }
}
impl crate::InformationDomain for RuntimeDomain {
    fn observe(
        &self,
        position: &GameTimeline,
        viewer: PlayerId,
    ) -> Result<GameTimeline, InteractionError> {
        self.observation(position, viewer)
    }
    fn sample(
        &self,
        position: &GameTimeline,
        viewer: PlayerId,
        seed: u64,
    ) -> Result<GameTimeline, InteractionError> {
        let rule = self
            .registry
            .information()
            .ok_or_else(|| InteractionError::RuleViolation("No information rule".into()))?;
        let state = rule
            .sample(position.current(), viewer, seed)
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))?;
        Ok(GameTimeline::new(state)?)
    }
}
