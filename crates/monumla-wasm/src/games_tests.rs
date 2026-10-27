use super::*;
use monumla_core::{InformationRule, PlayerId};
use monumla_games_battleship as sea;
use monumla_search::{SearchLimits, SearchOptions};
fn option(game: &mut GameHandle, key: &str) {
    let Runtime::Rules(r) = &mut game.inner else {
        panic!()
    };
    let id = r
        .interaction
        .as_ref()
        .unwrap()
        .interaction()
        .choices
        .iter()
        .find(|c| matches!(&c.kind,ChoiceKind::SelectOption{key:k} if k==key))
        .unwrap()
        .id;
    r.choose(id).unwrap();
}
fn prepared(seed: u64) -> GameHandle {
    let mut game = GameHandle {
        inner: Runtime::Rules(
            RulesRuntime::new(Kind::Battleship, sea::initial_state(seed)).unwrap(),
        ),
    };
    for _ in 0..2 {
        option(&mut game, "arrange");
        option(&mut game, "ready");
    }
    game
}
fn options() -> SearchOptions {
    SearchOptions {
        limits: SearchLimits {
            max_depth: 1,
            max_nodes: Some(500),
        },
        multi_pv: 3,
        ..Default::default()
    }
}
#[test]
fn worker_snapshot_drops_enemy_fleet_and_all_private_history() {
    let game = prepared(10);
    let snapshot = game.snapshot().unwrap();
    assert!(snapshot.timeline.history().is_empty());
    assert_eq!(snapshot.timeline.current().entities.len(), 20);
    assert!(snapshot
        .timeline
        .current()
        .entities
        .values()
        .all(|e| e.owner == PlayerId::new(1)));
    assert!(snapshot
        .timeline
        .current()
        .players
        .values()
        .all(|p| p.state.is_empty()));
    let restored = GameHandle::restore_snapshot(snapshot).unwrap();
    assert_eq!(
        game.analysis_position_key(),
        restored.analysis_position_key()
    );
}
#[test]
fn sampled_search_is_independent_of_real_enemy_fleet() {
    let a = prepared(10);
    let mut b = prepared(10);
    let Runtime::Rules(r) = &mut b.inner else {
        panic!()
    };
    let observed = sea::FleetInformation::default()
        .observe(r.timeline.current(), sea::FIRST)
        .unwrap();
    let replacement = sea::FleetInformation::default()
        .sample(&observed, sea::FIRST, 999)
        .unwrap();
    r.timeline = GameTimeline::new(replacement).unwrap();
    r.rebuild_interaction().unwrap();
    let view_a = a.analyze_position(options(), || false, |_| {}).unwrap();
    let view_b = b.analyze_position(options(), || false, |_| {}).unwrap();
    assert_eq!(
        view_a
            .lines
            .iter()
            .map(|l| (&l.moves, l.score))
            .collect::<Vec<_>>(),
        view_b
            .lines
            .iter()
            .map(|l| (&l.moves, l.score))
            .collect::<Vec<_>>()
    );
    assert!(view_a.samples >= 5);
    assert_eq!(view_a.depth, 1);
}
#[test]
fn both_fleets_play_via_redacted_worker_and_results_replay_on_authority() {
    let mut game = prepared(25);
    for _ in 0..12 {
        let worker = GameHandle::restore_snapshot(game.snapshot().unwrap()).unwrap();
        let report = worker
            .analyze_position(options(), || false, |_| {})
            .unwrap();
        assert_eq!(
            report.perspective,
            game.timeline().current().turn.active_players[0].get()
        );
        let action = report.best_action.unwrap();
        game.apply_analyzed_action(action).unwrap();
    }
    assert_eq!(game.timeline().history().len(), 16);
}
#[test]
fn setup_bot_places_and_confirms_without_endless_rearranging() {
    let mut game = GameHandle::new_ruleset("battleship").unwrap();
    for _ in 0..4 {
        let worker = GameHandle::restore_snapshot(game.snapshot().unwrap()).unwrap();
        let report = worker
            .analyze_position(
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 1,
                        max_nodes: Some(200),
                    },
                    ..Default::default()
                },
                || false,
                |_| {},
            )
            .unwrap();
        game.apply_analyzed_action(report.best_action.unwrap())
            .unwrap();
    }
    assert_eq!(sea::phase(game.timeline().current()), "battle");
}
#[test]
fn tic_tac_toe_bot_finishes_winning_line() {
    let config = monumla_games_tictactoe::Config::standard();
    let mut state = monumla_games_tictactoe::initial_state(config).unwrap();
    for (id, owner, x, y) in [(1, 1, 0, 0), (2, 1, 1, 0), (3, 2, 0, 1), (4, 2, 1, 1)] {
        state
            .add_entity(EntityState::new(
                monumla_core::EntityId::new(id),
                monumla_games_tictactoe::MARK,
                PlayerId::new(owner),
                Position::new(x, y),
            ))
            .unwrap();
    }
    let mut game = GameHandle {
        inner: Runtime::Rules(RulesRuntime::new(Kind::TicTacToe, state).unwrap()),
    };
    let result = game.analyze_position(options(), || false, |_| {}).unwrap();
    game.apply_analyzed_action(result.best_action.unwrap())
        .unwrap();
    assert_eq!(
        game.domain()
            .outcome(game.timeline())
            .unwrap()
            .unwrap()
            .winners,
        vec![PlayerId::new(1)]
    );
}

#[test]
fn sampled_battleship_match_reaches_a_real_terminal_result() {
    let mut game = prepared(86);
    for _ in 0..200 {
        if game.domain().outcome(game.timeline()).unwrap().is_some() {
            return;
        }
        let worker = GameHandle::restore_snapshot(game.snapshot().unwrap()).unwrap();
        let report = worker
            .analyze_position(
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 1,
                        max_nodes: Some(120),
                    },
                    ..Default::default()
                },
                || false,
                |_| {},
            )
            .unwrap();
        game.apply_analyzed_action(report.best_action.unwrap())
            .unwrap();
    }
    panic!("A finite no-repeat-shot game must finish");
}
#[test]
fn solved_classic_tic_tac_toe_self_play_is_a_draw() {
    let mut game = GameHandle::new_ruleset("tictactoe").unwrap();
    for ply in 0..9 {
        if let Some(outcome) = game.domain().outcome(game.timeline()).unwrap() {
            assert!(outcome.winners.is_empty());
            return;
        }
        let report = game
            .analyze_position(
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 32,
                        max_nodes: Some(1_000_000),
                    },
                    multi_pv: 3,
                    ..Default::default()
                },
                || false,
                |_| {},
            )
            .unwrap();
        assert_eq!(report.stop_reason, "solved");
        assert_eq!(report.score, Some(0));
        assert!(report.depth <= 9 - ply);
        assert!(report.selective_depth <= 9 - ply);
        assert!(report
            .lines
            .iter()
            .all(|v| v.moves.len() <= usize::from(9 - ply)));
        if ply == 0 {
            assert_eq!(report.lines.len(), 3);
            assert!(report.lines.iter().all(|v| v.score == 0));
        }
        game.apply_analyzed_action(report.best_action.unwrap())
            .unwrap();
    }
    assert!(game
        .domain()
        .outcome(game.timeline())
        .unwrap()
        .unwrap()
        .winners
        .is_empty());
}

#[test]
fn hidden_review_sends_only_pre_move_knowledge_and_does_not_grade_luck() {
    let mut a = prepared(21);
    let mut b = prepared(21);
    let r = match &mut b.inner {
        Runtime::Rules(r) => r,
        _ => panic!(),
    };
    let information = sea::FleetInformation::default();
    let observed = information
        .observe(r.timeline.current(), sea::FIRST)
        .unwrap();
    r.timeline =
        GameTimeline::new(information.sample(&observed, sea::FIRST, 999).unwrap()).unwrap();
    r.rebuild_interaction().unwrap();
    let target = (0..100)
        .map(|i| Position::new(10 + i % 10, i / 10))
        .find(|p| {
            a.timeline().current().entity_at(*p).unwrap().is_some()
                != b.timeline().current().entity_at(*p).unwrap().is_some()
        })
        .unwrap();
    for game in [&mut a, &mut b] {
        let Runtime::Rules(r) = &mut game.inner else {
            panic!()
        };
        let id = r
            .interaction
            .as_ref()
            .unwrap()
            .interaction()
            .choices
            .iter()
            .find(|c| matches!(c.kind,ChoiceKind::SelectPosition{position} if position==target))
            .unwrap()
            .id;
        r.choose(id).unwrap();
    }
    let index_a = a.timeline().history().len() - 1;
    let index_b = b.timeline().history().len() - 1;
    let snapshot_a = a.snapshot_for_review(index_a).unwrap();
    let snapshot_b = b.snapshot_for_review(index_b).unwrap();
    // Reviewing a retained future move must expose the same pre-move observation,
    // without changing the actual board or leaking its private outcome.
    let Runtime::Rules(runtime) = &mut a.inner else { panic!() };
    runtime.timeline.undo().unwrap();
    runtime.rebuild_interaction().unwrap();
    let current = a.timeline().clone();
    let future = a.snapshot_for_review(index_a).unwrap();
    assert_eq!(future.timeline, snapshot_a.timeline);
    assert_eq!(future.review_input, snapshot_a.review_input);
    assert_eq!(a.timeline(), &current);
    assert_eq!(snapshot_a.timeline, snapshot_b.timeline);
    assert_eq!(snapshot_a.review_input, snapshot_b.review_input);
    assert!(snapshot_a.timeline.history().is_empty());
    assert_eq!(snapshot_a.timeline.current().entities.len(), 20);
    assert_ne!(
        sea::shot(
            a.timeline().current(),
            sea::SECOND,
            usize::from(target.y) * 10 + usize::from(target.x - 10)
        ),
        sea::shot(
            b.timeline().current(),
            sea::SECOND,
            usize::from(target.y) * 10 + usize::from(target.x - 10)
        )
    );
    let result_a = GameHandle::restore_snapshot(snapshot_a)
        .unwrap()
        .assess_turn(index_a, options(), 60_000.0)
        .unwrap();
    let result_b = GameHandle::restore_snapshot(snapshot_b)
        .unwrap()
        .assess_turn(index_b, options(), 60_000.0)
        .unwrap();
    assert_eq!(result_a.best_score, result_b.best_score);
    assert_eq!(result_a.played_score, result_b.played_score);
    assert_eq!(result_a.symbol, result_b.symbol);
    assert!(result_a.samples >= 128);
}
