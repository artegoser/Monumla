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
    let observed = sea::FleetInformation
        .observe(r.timeline.current(), sea::FIRST)
        .unwrap();
    let replacement = sea::FleetInformation
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
    for _ in 0..9 {
        if let Some(outcome) = game.domain().outcome(game.timeline()).unwrap() {
            assert!(outcome.winners.is_empty());
            return;
        }
        let report = game
            .analyze_position(
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 9,
                        max_nodes: Some(1_000_000),
                    },
                    ..Default::default()
                },
                || false,
                |_| {},
            )
            .unwrap();
        assert_eq!(report.depth, 9);
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
