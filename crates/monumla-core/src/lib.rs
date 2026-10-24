//! Generic Monumla game runtime primitives.
#![forbid(unsafe_code)]

mod action;
mod board;
mod error;
mod evaluation;
mod history;
mod ids;
mod information;
mod interaction;
mod rules;
mod state;
mod transaction;
mod value;

pub use action::{EntityActionPath, TurnInput};
pub use board::{Board, Position};
pub use error::CoreError;
pub use evaluation::PositionScoreRule;
pub use history::{
    GameTimeline, History, RecordedAction, StepRecord, TransactionResult, TurnRecord, TurnSession,
};
pub use ids::{AbilityId, ChoiceId, EntityId, EntityTypeId, PlayerId, TeamId};
pub use information::InformationRule;
pub use interaction::{
    recorded_step, Choice, ChoiceInput, ChoiceIssuer, ChoiceKind, ChoicePurpose, ChoiceSpec,
    Interaction, InteractionDriver, InteractionError, InteractionFlow, InteractionRules,
    InteractionUpdate,
};
pub use rules::{
    AbilityRule, EntityPresentation, EntityRule, EntityRuleContext, GameOutcome, GameRule,
    GameRuleSet, OutcomeRule, RuleContext, RuleError, RuleRegistry,
};
pub use state::{
    EntityData, EntityState, EntityStore, GameState, PlayerData, PlayerState, PlayerStore,
    RulesetState, TeamData, TeamState, TeamStore, TurnData, TurnState,
};
pub use transaction::{PresentationCue, StateChange, StateDelta, Transaction, TransactionOutcome};
pub use value::{fingerprint, StateMap, StateValue};

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> GameState {
        let mut state = GameState::new(4, 4).unwrap();
        let team = TeamId::new(10);
        let player_a = PlayerId::new(1);
        let player_b = PlayerId::new(2);
        state.add_team(TeamState::new(team)).unwrap();
        state
            .add_player(PlayerState::new(player_a).with_team(team))
            .unwrap();
        state
            .add_player(PlayerState::new(player_b).with_team(team))
            .unwrap();
        state.set_active_players(vec![player_a]).unwrap();
        state
    }

    #[test]
    fn board_rejects_invalid_positions() {
        let board = Board::new(8, 8).unwrap();
        assert!(board.contains(Position::new(7, 7)));
        assert!(!board.contains(Position::new(8, 7)));
        assert_eq!(
            board.entity_at(Position::new(8, 0)),
            Err(CoreError::PositionOutOfBounds(Position::new(8, 0)))
        );
    }

    #[test]
    fn entity_lifecycle_keeps_board_and_store_in_sync() {
        let mut state = sample_state();
        let entity_id = EntityId::new(7);
        let owner = PlayerId::new(1);
        state
            .add_entity(EntityState::new(
                entity_id,
                EntityTypeId::new(3),
                owner,
                Position::new(1, 1),
            ))
            .unwrap();

        assert_eq!(
            state.entity_at(Position::new(1, 1)).unwrap().unwrap().id,
            entity_id
        );
        state.move_entity(entity_id, Position::new(2, 2)).unwrap();
        assert!(state.entity_at(Position::new(1, 1)).unwrap().is_none());
        assert!(state.entity(entity_id).unwrap().state.is_empty());
        assert_eq!(
            state.entity(entity_id).unwrap().position,
            Position::new(2, 2)
        );

        let removed = state.remove_entity(entity_id).unwrap();
        assert_eq!(removed.id, entity_id);
        assert!(state.entity_at(Position::new(2, 2)).unwrap().is_none());
        state.validate().unwrap();
    }

    #[test]
    fn owner_and_controller_are_independent() {
        let mut state = sample_state();
        let entity_id = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                entity_id,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();

        state.set_controller(entity_id, PlayerId::new(2)).unwrap();
        let entity = state.entity(entity_id).unwrap();
        assert_eq!(entity.owner, PlayerId::new(1));
        assert_eq!(entity.controller, PlayerId::new(2));
        state.validate().unwrap();
    }

    #[test]
    fn entity_and_ruleset_state_are_extensible() {
        let mut state = sample_state();
        let mut entity = EntityState::new(
            EntityId::new(7),
            EntityTypeId::new(3),
            PlayerId::new(1),
            Position::new(0, 0),
        );
        entity.state.insert("health", 100_u32);
        state.add_entity(entity).unwrap();
        state.ruleset_state.insert("round", 4_u32);

        assert_eq!(
            state
                .entity(EntityId::new(7))
                .unwrap()
                .state
                .get("health")
                .and_then(StateValue::as_u64),
            Some(100)
        );
        assert_eq!(
            state
                .ruleset_state
                .get("round")
                .and_then(StateValue::as_u64),
            Some(4)
        );
    }

    #[test]
    fn validate_detects_manual_entity_position_corruption() {
        let mut state = sample_state();
        let entity_id = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                entity_id,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();

        state.entity_mut(entity_id).unwrap().position = Position::new(1, 0);
        assert!(matches!(
            state.validate(),
            Err(CoreError::EntityPlacementMismatch { entity, .. }) if entity == entity_id
        ));
    }

    #[test]
    fn duplicate_active_players_are_rejected() {
        let mut state = sample_state();
        let player = PlayerId::new(1);
        assert_eq!(
            state.set_active_players(vec![player, player]),
            Err(CoreError::DuplicateActivePlayer(player))
        );
    }

    #[test]
    fn turn_session_records_multiple_steps_from_updated_state() {
        let mut state = sample_state();
        let entity = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();

        let mut turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        turn.apply_step(RecordedAction::new("move-1"), |state| {
            state.move_entity(entity, Position::new(1, 0))
        })
        .unwrap();
        turn.apply_step(RecordedAction::new("move-2"), |state| {
            assert_eq!(state.entity(entity)?.position, Position::new(1, 0));
            state.move_entity(entity, Position::new(2, 0))
        })
        .unwrap();

        assert_eq!(turn.steps.len(), 2);
        assert_eq!(
            turn.working.entity(entity).unwrap().position,
            Position::new(2, 0)
        );
        assert_eq!(
            turn.before.entity(entity).unwrap().position,
            Position::new(0, 0)
        );
    }

    #[test]
    fn speculative_state_never_enters_history() {
        let state = sample_state();
        let turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        let (candidate, ()) = turn
            .speculate(|state| {
                state.ruleset_state.insert("speculative", true);
                Ok(())
            })
            .unwrap();

        assert_eq!(
            candidate
                .ruleset_state
                .get("speculative")
                .and_then(StateValue::as_bool),
            Some(true)
        );
        assert!(!turn.working.ruleset_state.contains_key("speculative"));
        assert!(turn.steps.is_empty());
    }

    #[test]
    fn timeline_commits_undoes_and_redoes_complete_turns() {
        let mut state = sample_state();
        let entity = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        let mut timeline = GameTimeline::new(state).unwrap();

        let mut turn = timeline.begin_turn(PlayerId::new(1)).unwrap();
        turn.apply_step(RecordedAction::new("move"), |state| {
            state.move_entity(entity, Position::new(1, 0))
        })
        .unwrap();
        timeline.commit_turn(turn).unwrap();

        assert_eq!(
            timeline.current().entity(entity).unwrap().position,
            Position::new(1, 0)
        );
        assert_eq!(timeline.history().len(), 1);
        assert_eq!(timeline.history().last_step().unwrap().action.kind, "move");
        assert_eq!(
            timeline.entity_turns_ago(entity, 1).unwrap().position,
            Position::new(0, 0)
        );

        timeline.undo().unwrap();
        assert_eq!(
            timeline.current().entity(entity).unwrap().position,
            Position::new(0, 0)
        );
        assert!(timeline.can_redo());

        timeline.redo().unwrap();
        assert_eq!(
            timeline.current().entity(entity).unwrap().position,
            Position::new(1, 0)
        );
        assert_eq!(timeline.history().len(), 1);
    }

    #[test]
    fn rolling_back_a_turn_returns_the_original_snapshot() {
        let state = sample_state();
        let mut turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        turn.apply_step(RecordedAction::new("state-change"), |state| {
            state.ruleset_state.insert("changed", true);
            Ok(())
        })
        .unwrap();

        assert_eq!(
            turn.working
                .ruleset_state
                .get("changed")
                .and_then(StateValue::as_bool),
            Some(true)
        );
        let rolled_back = turn.rollback();
        assert_eq!(rolled_back, state);
    }

    struct ForcedChainRules {
        entity: EntityId,
    }

    impl InteractionRules for ForcedChainRules {
        fn choices(
            &self,
            turn: &TurnSession,
            _draft: &StateMap,
        ) -> Result<Vec<ChoiceSpec>, InteractionError> {
            let position = turn.working.entity(self.entity)?.position;
            let choices = match position.x {
                0 => vec![ChoiceSpec::position(Position::new(1, 0))],
                1 => vec![ChoiceSpec::position(Position::new(2, 0))],
                2 => vec![ChoiceSpec::finish_turn()],
                _ => {
                    return Err(InteractionError::RuleViolation(
                        "unexpected chain state".into(),
                    ))
                }
            };
            Ok(choices)
        }

        fn apply_choice(
            &self,
            turn: &mut TurnSession,
            _draft: &mut StateMap,
            choice: &Choice,
        ) -> Result<InteractionFlow, InteractionError> {
            match choice.kind {
                ChoiceKind::SelectPosition { position } => {
                    let entity = self.entity;
                    turn.apply_step(recorded_step("forced-jump"), |state| {
                        state.move_entity(entity, position)
                    })?;
                    Ok(InteractionFlow::Continue)
                }
                ChoiceKind::FinishTurn => Ok(InteractionFlow::FinishTurn),
                _ => Err(InteractionError::RuleViolation("unexpected choice".into())),
            }
        }
    }

    struct AbilityFlowRules {
        actor: EntityId,
        target: EntityId,
        ability: AbilityId,
    }

    impl InteractionRules for AbilityFlowRules {
        fn choices(
            &self,
            turn: &TurnSession,
            draft: &StateMap,
        ) -> Result<Vec<ChoiceSpec>, InteractionError> {
            if !draft.contains_key("moved") {
                return Ok(vec![ChoiceSpec::position(Position::new(1, 0))]);
            }
            if !draft.contains_key("ability") {
                return Ok(vec![
                    ChoiceSpec::ability(self.ability).with_label("Fireball"),
                    ChoiceSpec::finish_turn(),
                ]);
            }
            if !draft.contains_key("target") {
                return Ok(vec![ChoiceSpec::entity(self.target)]);
            }
            if !draft.contains_key("mode") {
                return Ok(vec![ChoiceSpec::option("normal")]);
            }

            assert_eq!(
                turn.working.entity(self.actor)?.position,
                Position::new(1, 0)
            );
            Ok(vec![ChoiceSpec::finish_turn()])
        }

        fn apply_choice(
            &self,
            turn: &mut TurnSession,
            draft: &mut StateMap,
            choice: &Choice,
        ) -> Result<InteractionFlow, InteractionError> {
            match &choice.kind {
                ChoiceKind::SelectPosition { position } => {
                    let actor = self.actor;
                    turn.apply_step(recorded_step("move"), |state| {
                        state.move_entity(actor, *position)
                    })?;
                    draft.insert("moved", true);
                    Ok(InteractionFlow::Continue)
                }
                ChoiceKind::SelectAbility { ability } if *ability == self.ability => {
                    draft.insert("ability", u64::from(ability.get()));
                    Ok(InteractionFlow::Continue)
                }
                ChoiceKind::SelectEntity { entity } if *entity == self.target => {
                    draft.insert("target", u64::from(entity.get()));
                    Ok(InteractionFlow::Continue)
                }
                ChoiceKind::SelectOption { key } if key == "normal" => {
                    draft.insert("mode", key.as_str());
                    let target = self.target;
                    turn.apply_step(recorded_step("fireball"), |state| {
                        state.entity_mut(target)?.state.insert("hit", true);
                        Ok(())
                    })?;
                    Ok(InteractionFlow::Continue)
                }
                ChoiceKind::FinishTurn => Ok(InteractionFlow::FinishTurn),
                _ => Err(InteractionError::RuleViolation("unexpected choice".into())),
            }
        }
    }

    #[test]
    fn forced_continuation_requeries_from_the_updated_working_state() {
        let mut state = sample_state();
        let entity = EntityId::new(20);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(1),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        let turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        let mut driver = InteractionDriver::new(ForcedChainRules { entity }, turn).unwrap();

        let first = driver.interaction().choices[0].clone();
        assert!(
            matches!(&first.kind, ChoiceKind::SelectPosition { position } if *position == Position::new(1, 0))
        );
        driver.choose(first.id).unwrap();

        let second = driver.interaction().choices[0].clone();
        assert!(
            matches!(&second.kind, ChoiceKind::SelectPosition { position } if *position == Position::new(2, 0))
        );
        assert!(!driver
            .interaction()
            .choices
            .iter()
            .any(|choice| matches!(&choice.kind, ChoiceKind::FinishTurn)));
        driver.choose(second.id).unwrap();

        assert_eq!(driver.turn().steps.len(), 2);
        assert!(matches!(
            &driver.interaction().choices[0].kind,
            ChoiceKind::FinishTurn
        ));
        let finish = driver.interaction().choices[0].id;
        assert_eq!(driver.choose(finish).unwrap(), InteractionUpdate::Finished);
        assert!(driver.is_finished());
    }

    #[test]
    fn interaction_supports_move_then_ability_then_target_and_option() {
        let mut state = sample_state();
        let actor = EntityId::new(20);
        let target = EntityId::new(21);
        state
            .add_entity(EntityState::new(
                actor,
                EntityTypeId::new(1),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        state
            .add_entity(EntityState::new(
                target,
                EntityTypeId::new(2),
                PlayerId::new(2),
                Position::new(3, 0),
            ))
            .unwrap();
        let turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        let ability = AbilityId::new(9);
        let mut driver = InteractionDriver::new(
            AbilityFlowRules {
                actor,
                target,
                ability,
            },
            turn,
        )
        .unwrap();

        // A simple move is offered directly; there is no pointless ability menu first.
        let move_choice = driver.interaction().choices[0].clone();
        assert!(matches!(
            &move_choice.kind,
            ChoiceKind::SelectPosition { .. }
        ));
        driver.choose(move_choice.id).unwrap();

        let ability_choice = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(&choice.kind, ChoiceKind::SelectAbility { .. }))
            .unwrap()
            .clone();
        driver.choose(ability_choice.id).unwrap();

        let target_choice = driver.interaction().choices[0].clone();
        assert!(
            matches!(&target_choice.kind, ChoiceKind::SelectEntity { entity } if *entity == target)
        );
        driver.choose(target_choice.id).unwrap();

        let option_choice = driver.interaction().choices[0].clone();
        assert!(matches!(&option_choice.kind, ChoiceKind::SelectOption { key } if key == "normal"));
        driver.choose(option_choice.id).unwrap();

        assert_eq!(driver.turn().steps.len(), 2);
        assert_eq!(
            driver
                .turn()
                .working
                .entity(target)
                .unwrap()
                .state
                .get("hit")
                .and_then(StateValue::as_bool),
            Some(true)
        );
    }

    #[test]
    fn stale_choice_ids_are_rejected_after_interaction_refresh() {
        let mut state = sample_state();
        let entity = EntityId::new(20);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(1),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        let turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        let mut driver = InteractionDriver::new(ForcedChainRules { entity }, turn).unwrap();

        let stale = driver.interaction().choices[0].id;
        driver.choose(stale).unwrap();
        assert_eq!(
            driver.choose(stale),
            Err(InteractionError::StaleOrInvalidChoice(stale))
        );
    }

    #[test]
    fn transaction_traces_structural_changes_and_presentation() {
        let mut state = sample_state();
        let entity = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();

        let mut transaction = Transaction::new(&state);
        transaction
            .move_entity(entity, Position::new(1, 0))
            .unwrap();
        transaction.entity_mut(entity).unwrap().entity_type = EntityTypeId::new(4);
        transaction
            .entity_mut(entity)
            .unwrap()
            .state
            .insert("health", 80_u32);
        transaction.present(PresentationCue::new("test_cast"));
        let outcome = transaction.finish().unwrap();

        assert_eq!(
            outcome.state.entity(entity).unwrap().position,
            Position::new(1, 0)
        );
        assert!(outcome.delta.changes.iter().any(|change| matches!(
            change,
            StateChange::EntityMoved { entity: changed, from, to }
                if *changed == entity
                    && *from == Position::new(0, 0)
                    && *to == Position::new(1, 0)
        )));
        assert!(outcome.delta.changes.iter().any(|change| matches!(
            change,
            StateChange::EntityTypeChanged { entity: changed, from, to }
                if *changed == entity
                    && *from == EntityTypeId::new(3)
                    && *to == EntityTypeId::new(4)
        )));
        assert!(outcome.delta.changes.iter().any(|change| matches!(
            change,
            StateChange::EntityStateChanged { entity: changed, .. } if *changed == entity
        )));
        assert_eq!(
            outcome.presentation,
            vec![PresentationCue::new("test_cast")]
        );
        assert_eq!(state.entity(entity).unwrap().position, Position::new(0, 0));
    }

    #[test]
    fn invalid_transaction_never_changes_turn_working_state() {
        let mut state = sample_state();
        let entity = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        let mut turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();

        let result: Result<TransactionResult<()>, CoreError> =
            turn.apply_transaction(RecordedAction::new("corrupt"), |transaction| {
                transaction.entity_mut(entity)?.position = Position::new(2, 0);
                Ok(())
            });

        assert!(result.is_err());
        assert_eq!(turn.working, state);
        assert!(turn.steps.is_empty());
    }

    #[test]
    fn transaction_traces_spawn_and_remove() {
        let mut state = sample_state();
        let removed = EntityId::new(7);
        state
            .add_entity(EntityState::new(
                removed,
                EntityTypeId::new(3),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        let added = EntityId::new(8);

        let mut transaction = Transaction::new(&state);
        transaction.remove_entity(removed).unwrap();
        transaction
            .spawn_entity(EntityState::new(
                added,
                EntityTypeId::new(4),
                PlayerId::new(2),
                Position::new(3, 3),
            ))
            .unwrap();
        let outcome = transaction.finish().unwrap();

        assert!(outcome.delta.changes.iter().any(|change| matches!(
            change,
            StateChange::EntityRemoved { entity } if entity.id == removed
        )));
        assert!(outcome.delta.changes.iter().any(|change| matches!(
            change,
            StateChange::EntityAdded { entity } if entity.id == added
        )));
    }

    struct TestEntityRule;

    impl EntityRule for TestEntityRule {
        fn presentation(
            &self,
            context: EntityRuleContext<'_>,
        ) -> Result<EntityPresentation, RuleError> {
            let charged = context
                .entity()
                .state
                .get("charged")
                .and_then(StateValue::as_bool)
                .unwrap_or(false);
            let history_len = context.history().map(History::len).unwrap_or(0);
            let turn_steps = context.turn().map(|turn| turn.steps.len()).unwrap_or(0);
            let mut data = StateMap::new();
            data.insert("history_len", history_len as u64);
            data.insert("turn_steps", turn_steps as u64);
            Ok(
                EntityPresentation::new(if charged { "test/charged" } else { "test/idle" })
                    .with_data(data),
            )
        }
    }

    struct FlagOutcomeRule {
        key: &'static str,
        flag: &'static str,
        winner: Option<PlayerId>,
    }

    impl OutcomeRule for FlagOutcomeRule {
        fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
            let active = context
                .state()
                .ruleset_state
                .get(self.flag)
                .and_then(StateValue::as_bool)
                .unwrap_or(false);
            if !active {
                return Ok(None);
            }
            let mut outcome = GameOutcome::new(self.key);
            if let Some(winner) = self.winner {
                outcome = outcome.with_winner(winner);
            }
            Ok(Some(outcome))
        }
    }

    struct AddGlobalChoiceRule;

    impl GameRule for AddGlobalChoiceRule {
        fn choices(
            &self,
            _context: RuleContext<'_>,
            _actor: PlayerId,
            _draft: &StateMap,
        ) -> Result<Vec<ChoiceSpec>, RuleError> {
            Ok(vec![ChoiceSpec::option("global")])
        }

        fn apply_choice(
            &self,
            _history: Option<&History>,
            _turn: &mut TurnSession,
            draft: &mut StateMap,
            choice: &Choice,
        ) -> Result<Option<InteractionFlow>, RuleError> {
            if matches!(&choice.kind, ChoiceKind::SelectOption { key } if key == "global") {
                draft.insert("global_used", true);
                return Ok(Some(InteractionFlow::Continue));
            }
            Ok(None)
        }
    }

    struct EnvironmentReactionRule {
        entity: EntityId,
    }

    impl GameRule for EnvironmentReactionRule {
        fn react(
            &self,
            before: &GameState,
            _action: &RecordedAction,
            transaction: &mut Transaction,
        ) -> Result<(), RuleError> {
            let before_position = before.entity(self.entity)?.position;
            let after_position = transaction.entity(self.entity)?.position;
            if before_position != after_position && after_position == Position::new(1, 0) {
                transaction
                    .entity_mut(self.entity)?
                    .state
                    .insert("environment_hit", true);
            }
            Ok(())
        }
    }

    struct RemoveBlockedChoiceRule;

    impl GameRule for RemoveBlockedChoiceRule {
        fn transform_choices(
            &self,
            _context: RuleContext<'_>,
            _actor: PlayerId,
            _draft: &StateMap,
            choices: Vec<ChoiceSpec>,
        ) -> Result<Vec<ChoiceSpec>, RuleError> {
            Ok(choices
                .into_iter()
                .filter(|choice| {
                    !matches!(&choice.kind, ChoiceKind::SelectOption { key } if key == "blocked")
                })
                .collect())
        }
    }

    struct TestAbilityRule;

    impl AbilityRule for TestAbilityRule {
        fn execute(
            &self,
            context: RuleContext<'_>,
            actor: EntityId,
            transaction: &mut Transaction,
            _input: &StateMap,
        ) -> Result<(), RuleError> {
            // Proves the rule can read the source world while mutating an
            // independent transaction working copy.
            let old_position = context.entity(actor)?.position;
            transaction.entity_mut(actor)?.state.insert("used", true);
            transaction
                .entity_mut(actor)?
                .state
                .insert("old_x", u64::from(old_position.x));
            Ok(())
        }
    }

    #[test]
    fn registry_accepts_non_chess_entity_and_state_dependent_presentation() {
        let mut state = sample_state();
        let entity = EntityId::new(40);
        let entity_type = EntityTypeId::new(99);
        let mut test_entity =
            EntityState::new(entity, entity_type, PlayerId::new(1), Position::new(1, 1));
        test_entity.state.insert("charged", true);
        state.add_entity(test_entity).unwrap();

        let mut registry = RuleRegistry::new();
        registry
            .register_entity(entity_type, TestEntityRule)
            .unwrap();

        let presentation = registry
            .presentation(RuleContext::from_state(&state, None), entity)
            .unwrap();
        assert_eq!(presentation.asset_key, "test/charged");
        assert_eq!(
            presentation
                .data
                .get("history_len")
                .and_then(StateValue::as_u64),
            Some(0)
        );
        assert_eq!(
            registry.register_entity(entity_type, TestEntityRule),
            Err(RuleError::DuplicateEntityRule(entity_type))
        );
    }

    #[test]
    fn rule_context_exposes_history_and_current_turn_steps() {
        let mut state = sample_state();
        let entity = EntityId::new(41);
        let entity_type = EntityTypeId::new(100);
        state
            .add_entity(EntityState::new(
                entity,
                entity_type,
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();

        let mut timeline = GameTimeline::new(state).unwrap();
        let mut first = timeline.begin_turn(PlayerId::new(1)).unwrap();
        first
            .apply_step(RecordedAction::new("move"), |state| {
                state.move_entity(entity, Position::new(1, 0))
            })
            .unwrap();
        timeline.commit_turn(first).unwrap();

        let mut current_turn = timeline.begin_turn(PlayerId::new(1)).unwrap();
        current_turn
            .apply_step(RecordedAction::new("mark"), |state| {
                state.entity_mut(entity)?.state.insert("charged", true);
                Ok(())
            })
            .unwrap();

        let mut registry = RuleRegistry::new();
        registry
            .register_entity(entity_type, TestEntityRule)
            .unwrap();
        let presentation = registry
            .presentation(
                RuleContext::from_turn(&current_turn, Some(timeline.history())),
                entity,
            )
            .unwrap();

        assert_eq!(
            presentation
                .data
                .get("history_len")
                .and_then(StateValue::as_u64),
            Some(1)
        );
        assert_eq!(
            presentation
                .data
                .get("turn_steps")
                .and_then(StateValue::as_u64),
            Some(1)
        );
    }

    #[test]
    fn outcome_registry_is_ruleset_wide_and_uses_registration_precedence() {
        let mut state = sample_state();
        state.ruleset_state.insert("fallback_finished", true);
        state.ruleset_state.insert("primary_finished", true);

        let mut registry = RuleRegistry::new();
        registry.register_outcome(FlagOutcomeRule {
            key: "test.primary",
            flag: "primary_finished",
            winner: Some(PlayerId::new(1)),
        });
        registry.register_outcome(FlagOutcomeRule {
            key: "test.fallback",
            flag: "fallback_finished",
            winner: Some(PlayerId::new(2)),
        });

        assert_eq!(registry.outcome_rule_count(), 2);
        let outcome = registry
            .outcome(RuleContext::from_state(&state, None))
            .unwrap()
            .unwrap();
        assert_eq!(outcome.key, "test.primary");
        assert_eq!(outcome.winners, vec![PlayerId::new(1)]);
        assert!(outcome.losers.is_empty());
    }

    #[test]
    fn choice_input_keeps_semantics_but_drops_ephemeral_presentation() {
        let mut spec = ChoiceSpec::option("upgrade")
            .with_label("Upgrade now")
            .with_asset_key("test/upgrade");
        spec.data.insert("level", 2_u64);
        let input = ChoiceInput::from(&spec);
        let mut issued = ChoiceIssuer::default().issue(vec![spec]);
        let choice = issued.choices.remove(0);
        let issued_input = ChoiceInput::from(&choice);

        assert_eq!(
            input.kind,
            ChoiceKind::SelectOption {
                key: "upgrade".into()
            }
        );
        assert_eq!(
            input.data.get("level").and_then(StateValue::as_u64),
            Some(2)
        );
        assert_eq!(choice.label.as_deref(), Some("Upgrade now"));
        assert_eq!(choice.asset_key.as_deref(), Some("test/upgrade"));
        assert_eq!(issued_input, input);
        assert!(!input.data.contains_key("asset_key"));
    }

    #[test]
    fn game_rules_compose_global_choices_and_transforms_in_registration_order() {
        let state = sample_state();
        let mut registry = RuleRegistry::new();
        registry.register_game(AddGlobalChoiceRule);
        registry.register_game(RemoveBlockedChoiceRule);

        let choices = registry
            .resolve_choices(
                RuleContext::from_state(&state, None),
                PlayerId::new(1),
                &StateMap::new(),
                vec![ChoiceSpec::option("allowed"), ChoiceSpec::option("blocked")],
            )
            .unwrap();

        assert_eq!(registry.game_rule_count(), 2);
        let keys = choices
            .iter()
            .filter_map(|choice| match &choice.kind {
                ChoiceKind::SelectOption { key } => Some(key.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["allowed", "global"]);

        let forced = registry
            .game_rules()
            .transform_choices(
                RuleContext::from_state(&state, None),
                PlayerId::new(1),
                &StateMap::new(),
                vec![ChoiceSpec::option("allowed"), ChoiceSpec::option("blocked")],
            )
            .unwrap();
        let forced_keys = forced
            .iter()
            .filter_map(|choice| match &choice.kind {
                ChoiceKind::SelectOption { key } => Some(key.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(forced_keys, vec!["allowed"]);

        let global_spec = choices
            .into_iter()
            .find(|choice| matches!(&choice.kind, ChoiceKind::SelectOption { key } if key == "global"))
            .unwrap();
        let mut issued = ChoiceIssuer::default().issue(vec![global_spec]);
        let global_choice = issued.choices.remove(0);
        let mut turn = TurnSession::new(&state, PlayerId::new(1)).unwrap();
        let mut draft = StateMap::new();
        assert_eq!(
            registry
                .apply_game_choice(None, &mut turn, &mut draft, &global_choice)
                .unwrap(),
            Some(InteractionFlow::Continue)
        );
        assert_eq!(
            draft.get("global_used").and_then(StateValue::as_bool),
            Some(true)
        );
    }

    #[test]
    fn game_rule_reactions_share_the_authoritative_transaction() {
        let mut state = sample_state();
        let entity = EntityId::new(50);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(102),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();

        let mut registry = RuleRegistry::new();
        registry.register_game(EnvironmentReactionRule { entity });
        let before = state.clone();
        let action = RecordedAction::new("move");
        let mut transaction = Transaction::new(&state);
        transaction
            .move_entity(entity, Position::new(1, 0))
            .unwrap();
        registry
            .react_game_rules(&before, &action, &mut transaction)
            .unwrap();
        let outcome = transaction.finish().unwrap();

        assert_eq!(
            outcome
                .state
                .entity(entity)
                .unwrap()
                .state
                .get("environment_hit")
                .and_then(StateValue::as_bool),
            Some(true)
        );
        assert!(state.entity(entity).unwrap().state.is_empty());
    }

    #[test]
    fn registered_ability_can_mutate_transactional_world() {
        let mut state = sample_state();
        let entity = EntityId::new(42);
        state
            .add_entity(EntityState::new(
                entity,
                EntityTypeId::new(101),
                PlayerId::new(1),
                Position::new(2, 0),
            ))
            .unwrap();
        let ability = AbilityId::new(12);
        let mut registry = RuleRegistry::new();
        registry.register_ability(ability, TestAbilityRule).unwrap();

        let context = RuleContext::from_state(&state, None);
        let mut transaction = Transaction::new(&state);
        registry
            .ability_rule(ability)
            .unwrap()
            .execute(context, entity, &mut transaction, &StateMap::new())
            .unwrap();
        let outcome = transaction.finish().unwrap();

        assert_eq!(
            outcome
                .state
                .entity(entity)
                .unwrap()
                .state
                .get("used")
                .and_then(StateValue::as_bool),
            Some(true)
        );
    }
}
