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
    fn choices(&self, _: &TurnSession, _: &StateMap) -> Result<Vec<ChoiceSpec>, InteractionError> {
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
