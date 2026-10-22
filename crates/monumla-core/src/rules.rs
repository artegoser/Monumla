use crate::{
    AbilityId, Choice, ChoiceSpec, CoreError, EntityId, EntityState, EntityTypeId, GameState,
    History, InteractionFlow, PlayerId, RecordedAction, StateMap, TeamId, Transaction, TurnSession,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameOutcome {
    /// Stable ruleset-defined reason key, e.g. `chess.checkmate`.
    pub key: String,
    pub winners: Vec<PlayerId>,
    pub losers: Vec<PlayerId>,
    pub winning_teams: Vec<TeamId>,
    pub losing_teams: Vec<TeamId>,
    pub data: StateMap,
}

impl GameOutcome {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            winners: Vec::new(),
            losers: Vec::new(),
            winning_teams: Vec::new(),
            losing_teams: Vec::new(),
            data: StateMap::new(),
        }
    }

    pub fn with_winner(mut self, player: PlayerId) -> Self {
        self.winners.push(player);
        self
    }

    pub fn with_loser(mut self, player: PlayerId) -> Self {
        self.losers.push(player);
        self
    }

    pub fn with_winning_team(mut self, team: TeamId) -> Self {
        self.winning_teams.push(team);
        self
    }

    pub fn with_losing_team(mut self, team: TeamId) -> Self {
        self.losing_teams.push(team);
        self
    }

    pub fn with_data(mut self, data: StateMap) -> Self {
        self.data = data;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityPresentation {
    /// Stable presentation key resolved by the frontend, e.g. `chess/white/knight`.
    pub asset_key: String,
    pub label: Option<String>,
    pub data: StateMap,
}

impl EntityPresentation {
    pub fn new(asset_key: impl Into<String>) -> Self {
        Self {
            asset_key: asset_key.into(),
            label: None,
            data: StateMap::new(),
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_data(mut self, data: StateMap) -> Self {
        self.data = data;
        self
    }
}

#[derive(Clone, Copy)]
pub struct RuleContext<'a> {
    state: &'a GameState,
    history: Option<&'a History>,
    turn: Option<&'a TurnSession>,
}

impl<'a> RuleContext<'a> {
    pub fn from_state(state: &'a GameState, history: Option<&'a History>) -> Self {
        Self {
            state,
            history,
            turn: None,
        }
    }

    pub fn from_turn(turn: &'a TurnSession, history: Option<&'a History>) -> Self {
        Self {
            state: &turn.working,
            history,
            turn: Some(turn),
        }
    }

    pub fn state(self) -> &'a GameState {
        self.state
    }

    pub fn history(self) -> Option<&'a History> {
        self.history
    }

    pub fn turn(self) -> Option<&'a TurnSession> {
        self.turn
    }

    pub fn entity(self, entity: EntityId) -> Result<&'a EntityState, CoreError> {
        self.state.entity(entity)
    }

    pub fn entity_context(self, entity: EntityId) -> Result<EntityRuleContext<'a>, CoreError> {
        Ok(EntityRuleContext {
            world: self,
            entity: self.entity(entity)?,
        })
    }
}

#[derive(Clone, Copy)]
pub struct EntityRuleContext<'a> {
    world: RuleContext<'a>,
    entity: &'a EntityState,
}

impl<'a> EntityRuleContext<'a> {
    pub fn world(self) -> RuleContext<'a> {
        self.world
    }

    pub fn state(self) -> &'a GameState {
        self.world.state()
    }

    pub fn history(self) -> Option<&'a History> {
        self.world.history()
    }

    pub fn turn(self) -> Option<&'a TurnSession> {
        self.world.turn()
    }

    pub fn entity(self) -> &'a EntityState {
        self.entity
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RuleError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("entity rule {0} is already registered")]
    DuplicateEntityRule(EntityTypeId),
    #[error("entity rule {0} is not registered")]
    EntityRuleNotFound(EntityTypeId),
    #[error("ability rule {0} is already registered")]
    DuplicateAbilityRule(AbilityId),
    #[error("ability rule {0} is not registered")]
    AbilityRuleNotFound(AbilityId),
    #[error("rule rejected operation: {0}")]
    Rejected(String),
}

/// Generic behavior attached to an entity type. Game-specific crates may add
/// richer traits on top of this one without teaching core those concepts.
pub trait EntityRule {
    fn presentation(&self, context: EntityRuleContext<'_>)
        -> Result<EntityPresentation, RuleError>;

    fn choices(
        &self,
        _context: EntityRuleContext<'_>,
        _draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        Ok(Vec::new())
    }
}

/// Reserved extension point for explicit abilities. An ability may mutate the
/// whole transactional state and is not constrained to a closed effect enum.
pub trait AbilityRule {
    fn choices(
        &self,
        _context: RuleContext<'_>,
        _actor: EntityId,
        _draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        Ok(Vec::new())
    }

    fn execute(
        &self,
        context: RuleContext<'_>,
        actor: EntityId,
        transaction: &mut Transaction,
        input: &StateMap,
    ) -> Result<(), RuleError>;
}

/// A terminal ruleset-wide condition. Entity rules may expose local semantics
/// used by an outcome rule, but deciding whether the game is over belongs to
/// the ruleset layer.
///
/// When multiple outcome rules are registered, registry order is precedence:
/// the first rule that returns an outcome wins.
pub trait OutcomeRule {
    fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError>;
}

/// A composable ruleset-wide rule for mechanics that belong to the game/world
/// rather than one entity or ability. Rules may add global choices and then
/// transform the combined choice set produced by local rules. Registration
/// order is execution order, making precedence explicit and deterministic.
pub trait GameRule {
    fn validate(&self, _context: RuleContext<'_>) -> Result<(), RuleError> {
        Ok(())
    }

    fn choices(
        &self,
        _context: RuleContext<'_>,
        _actor: PlayerId,
        _draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        Ok(Vec::new())
    }

    fn transform_choices(
        &self,
        _context: RuleContext<'_>,
        _actor: PlayerId,
        _draft: &StateMap,
        choices: Vec<ChoiceSpec>,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        Ok(choices)
    }

    /// Optionally handle a choice introduced or intercepted by this global
    /// rule. Returning `None` leaves the choice to another rule or the local
    /// entity/ability interaction.
    fn apply_choice(
        &self,
        _history: Option<&History>,
        _turn: &mut TurnSession,
        _draft: &mut StateMap,
        _choice: &Choice,
    ) -> Result<Option<InteractionFlow>, RuleError> {
        Ok(None)
    }

    /// React transactionally after the local action has produced its working
    /// state. `before` is the pre-step state; `transaction.state()` is the
    /// current post-action state and may be further mutated through the open
    /// transaction API. This supports terrain/environment/global reactions
    /// without a closed universal effect enum.
    fn react(
        &self,
        _before: &GameState,
        _action: &RecordedAction,
        _transaction: &mut Transaction,
    ) -> Result<(), RuleError> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct GameRuleSet {
    rules: Vec<Arc<dyn GameRule>>,
}

impl GameRuleSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<R>(&mut self, rule: R)
    where
        R: GameRule + 'static,
    {
        self.rules.push(Arc::new(rule));
    }

    pub fn validate(&self, context: RuleContext<'_>) -> Result<(), RuleError> {
        for rule in &self.rules {
            rule.validate(context)?;
        }
        Ok(())
    }

    pub fn resolve_choices(
        &self,
        context: RuleContext<'_>,
        actor: PlayerId,
        draft: &StateMap,
        mut choices: Vec<ChoiceSpec>,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        self.validate(context)?;
        for rule in &self.rules {
            choices.extend(rule.choices(context, actor, draft)?);
            choices = rule.transform_choices(context, actor, draft, choices)?;
        }
        Ok(choices)
    }

    /// Apply only global constraints/transforms to an already-required local
    /// continuation. This deliberately does not add unrelated top-level game
    /// choices while a forced entity/ability continuation is pending.
    pub fn transform_choices(
        &self,
        context: RuleContext<'_>,
        actor: PlayerId,
        draft: &StateMap,
        mut choices: Vec<ChoiceSpec>,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        self.validate(context)?;
        for rule in &self.rules {
            choices = rule.transform_choices(context, actor, draft, choices)?;
        }
        Ok(choices)
    }

    pub fn apply_choice(
        &self,
        history: Option<&History>,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<Option<InteractionFlow>, RuleError> {
        for rule in &self.rules {
            if let Some(flow) = rule.apply_choice(history, turn, draft, choice)? {
                return Ok(Some(flow));
            }
        }
        Ok(None)
    }

    pub fn react(
        &self,
        before: &GameState,
        action: &RecordedAction,
        transaction: &mut Transaction,
    ) -> Result<(), RuleError> {
        for rule in &self.rules {
            rule.react(before, action, transaction)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[derive(Default)]
pub struct RuleRegistry {
    entity_rules: BTreeMap<EntityTypeId, Box<dyn EntityRule>>,
    ability_rules: BTreeMap<AbilityId, Box<dyn AbilityRule>>,
    outcome_rules: Vec<Box<dyn OutcomeRule>>,
    game_rules: GameRuleSet,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_entity<R>(
        &mut self,
        entity_type: EntityTypeId,
        rule: R,
    ) -> Result<(), RuleError>
    where
        R: EntityRule + 'static,
    {
        if self.entity_rules.contains_key(&entity_type) {
            return Err(RuleError::DuplicateEntityRule(entity_type));
        }
        self.entity_rules.insert(entity_type, Box::new(rule));
        Ok(())
    }

    pub fn entity_rule(&self, entity_type: EntityTypeId) -> Result<&dyn EntityRule, RuleError> {
        self.entity_rules
            .get(&entity_type)
            .map(Box::as_ref)
            .ok_or(RuleError::EntityRuleNotFound(entity_type))
    }

    pub fn presentation(
        &self,
        context: RuleContext<'_>,
        entity: EntityId,
    ) -> Result<EntityPresentation, RuleError> {
        let entity_context = context.entity_context(entity)?;
        self.entity_rule(entity_context.entity().entity_type)?
            .presentation(entity_context)
    }

    pub fn register_ability<R>(&mut self, ability: AbilityId, rule: R) -> Result<(), RuleError>
    where
        R: AbilityRule + 'static,
    {
        if self.ability_rules.contains_key(&ability) {
            return Err(RuleError::DuplicateAbilityRule(ability));
        }
        self.ability_rules.insert(ability, Box::new(rule));
        Ok(())
    }

    pub fn ability_rule(&self, ability: AbilityId) -> Result<&dyn AbilityRule, RuleError> {
        self.ability_rules
            .get(&ability)
            .map(Box::as_ref)
            .ok_or(RuleError::AbilityRuleNotFound(ability))
    }

    pub fn register_outcome<R>(&mut self, rule: R)
    where
        R: OutcomeRule + 'static,
    {
        self.outcome_rules.push(Box::new(rule));
    }

    pub fn outcome(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        for rule in &self.outcome_rules {
            if let Some(outcome) = rule.evaluate(context)? {
                return Ok(Some(outcome));
            }
        }
        Ok(None)
    }

    pub fn outcome_rule_count(&self) -> usize {
        self.outcome_rules.len()
    }

    pub fn register_game<R>(&mut self, rule: R)
    where
        R: GameRule + 'static,
    {
        self.game_rules.register(rule);
    }

    pub fn game_rules(&self) -> &GameRuleSet {
        &self.game_rules
    }

    pub fn game_rule_count(&self) -> usize {
        self.game_rules.len()
    }

    pub fn resolve_choices(
        &self,
        context: RuleContext<'_>,
        actor: PlayerId,
        draft: &StateMap,
        choices: Vec<ChoiceSpec>,
    ) -> Result<Vec<ChoiceSpec>, RuleError> {
        self.game_rules.resolve_choices(context, actor, draft, choices)
    }

    pub fn apply_game_choice(
        &self,
        history: Option<&History>,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<Option<InteractionFlow>, RuleError> {
        self.game_rules.apply_choice(history, turn, draft, choice)
    }

    pub fn react_game_rules(
        &self,
        before: &GameState,
        action: &RecordedAction,
        transaction: &mut Transaction,
    ) -> Result<(), RuleError> {
        self.game_rules.react(before, action, transaction)
    }
}
