use super::*;
use monumla_core::{
    ChoiceKind, GameOutcome, GameTimeline, InteractionDriver, InteractionUpdate, OutcomeRule,
    RuleContext, TurnSession,
};

fn empty(variant: DraughtsVariant) -> GameState {
    let mut state = state_for_variant(variant);
    for id in state.entities.keys().copied().collect::<Vec<_>>() {
        state.remove_entity(id).unwrap();
    }
    state
}
fn add(state: &mut GameState, id: u32, owner: PlayerId, x: u16, y: u16, king: bool) -> EntityId {
    let id = EntityId::new(id);
    let mut entity = EntityState::new(id, CHECKER, owner, Position::new(x, y));
    if king {
        entity.state.insert(KING, true);
    }
    state.add_entity(entity).unwrap();
    id
}
fn outcome(state: &GameState) -> Option<GameOutcome> {
    CheckersOutcomeRule
        .evaluate(RuleContext::from_state(state, None))
        .unwrap()
}
fn choose(
    driver: &mut InteractionDriver<CheckersInteractionRules>,
    x: u16,
    y: u16,
) -> InteractionUpdate {
    let id = driver.interaction().choices.iter().find(|c| matches!(c.kind, ChoiceKind::SelectPosition { position } if position == Position::new(x,y))).unwrap().id;
    driver.choose(id).unwrap()
}

#[test]
fn standard_positions_have_correct_sizes_material_dark_squares_and_openings() {
    for (variant, size, men, openings) in [
        (DraughtsVariant::Russian, 8, 12, 7),
        (DraughtsVariant::International, 10, 20, 9),
    ] {
        let state = state_for_variant(variant);
        assert_eq!((state.board.width(), state.board.height()), (size, size));
        assert_eq!(
            state.entities.values().filter(|e| e.owner == WHITE).count(),
            men
        );
        assert_eq!(state.entities.len(), men * 2);
        assert!(state
            .entities
            .values()
            .all(|e| (e.position.x + e.position.y) % 2 == 0 && !is_king(e)));
        assert_eq!(legal_moves(&state, WHITE).unwrap().len(), openings);
    }
}
#[test]
fn men_capture_backwards_and_captures_are_compulsory_for_both_variants() {
    for variant in [DraughtsVariant::Russian, DraughtsVariant::International] {
        let mut state = empty(variant);
        let actor = add(&mut state, 1, WHITE, 4, 4, false);
        add(&mut state, 2, BLACK, 3, 3, false);
        add(&mut state, 3, WHITE, 0, 0, false);
        let legal = legal_moves(&state, WHITE).unwrap();
        assert_eq!(
            legal,
            vec![EntityActionPath {
                actor,
                landings: vec![Position::new(2, 2)],
                captures: vec![EntityId::new(2)]
            }]
        );
    }
}
#[test]
fn men_cannot_move_backwards_quietly() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, WHITE, 2, 2, false);
    assert!(legal_moves(&state, WHITE)
        .unwrap()
        .iter()
        .all(|m| m.landings[0].y == 3));
}
#[test]
fn flying_kings_move_and_land_at_every_free_square_behind_victim() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, WHITE, 0, 0, true);
    assert_eq!(legal_moves(&state, WHITE).unwrap().len(), 7);
    add(&mut state, 2, BLACK, 3, 3, false);
    let legal = legal_moves(&state, WHITE).unwrap();
    assert_eq!(legal.len(), 4);
    assert!(legal
        .iter()
        .all(|m| m.captures == [EntityId::new(2)] && m.landings[0].x >= 4));
    add(&mut state, 3, WHITE, 5, 5, false);
    assert_eq!(
        legal_moves(&state, WHITE).unwrap()[0].landings,
        [Position::new(4, 4)]
    );
}
#[test]
fn international_requires_globally_longest_capture_but_russian_permits_shorter() {
    for variant in [DraughtsVariant::Russian, DraughtsVariant::International] {
        let mut state = empty(variant);
        add(&mut state, 1, WHITE, 2, 2, false);
        add(&mut state, 2, BLACK, 1, 3, false);
        add(&mut state, 3, BLACK, 3, 3, false);
        add(&mut state, 4, BLACK, 5, 5, false);
        let legal = legal_moves(&state, WHITE).unwrap();
        assert!(legal.iter().any(|m| m.captures.len() == 2));
        assert_eq!(
            legal.iter().any(|m| m.captures.len() == 1),
            variant == DraughtsVariant::Russian
        );
    }
}
#[test]
fn international_maximum_is_compared_across_all_actors_without_king_priority() {
    let mut state = empty(DraughtsVariant::International);
    let man = add(&mut state, 1, WHITE, 0, 0, false);
    add(&mut state, 2, BLACK, 1, 1, true);
    add(&mut state, 3, BLACK, 3, 3, false);
    add(&mut state, 4, WHITE, 8, 0, true);
    add(&mut state, 5, BLACK, 7, 1, true);
    add(&mut state, 6, WHITE, 5, 3, false);
    assert!(legal_moves(&state, WHITE)
        .unwrap()
        .iter()
        .all(|m| m.actor == man && m.captures.len() == 2));
}
#[test]
fn captured_pieces_remain_blockers_and_cannot_be_jumped_twice() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, WHITE, 2, 2, true);
    add(&mut state, 2, BLACK, 3, 3, false);
    add(&mut state, 3, BLACK, 1, 1, false);
    let legal = legal_moves(&state, WHITE).unwrap();
    assert!(legal.iter().all(|m| m.captures.len() == 1));
}
#[test]
fn russian_promotion_continues_immediately_as_flying_king() {
    let mut state = empty(DraughtsVariant::Russian);
    let actor = add(&mut state, 1, WHITE, 1, 5, false);
    add(&mut state, 2, BLACK, 2, 6, false);
    add(&mut state, 3, BLACK, 5, 5, false);
    let moves = legal_moves(&state, WHITE).unwrap();
    assert!(moves.iter().all(|m| m.captures.len() == 2));
    let mut driver = InteractionDriver::new(
        CheckersInteractionRules,
        TurnSession::new(&state, WHITE).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        choose(&mut driver, 3, 7),
        InteractionUpdate::Continued(_)
    ));
    assert!(is_king(driver.turn().working.entity(actor).unwrap()));
    assert_eq!(driver.turn().working.entities.len(), 3);
    // Draft cancellation cannot abandon a forced sequence.
    driver.reset_draft().unwrap();
    assert!(driver
        .interaction()
        .choices
        .iter()
        .all(|c| matches!(c.kind, ChoiceKind::SelectPosition { .. })));
    assert_eq!(choose(&mut driver, 6, 4), InteractionUpdate::Finished);
    assert_eq!(driver.turn().working.entities.len(), 1);
    assert!(driver
        .turn()
        .steps
        .iter()
        .any(|s| !s.presentation.is_empty()));
}
#[test]
fn international_promotion_is_deferred_and_passing_last_row_does_not_crown() {
    let mut state = empty(DraughtsVariant::International);
    let actor = add(&mut state, 1, WHITE, 1, 7, false);
    add(&mut state, 2, BLACK, 2, 8, false);
    add(&mut state, 3, BLACK, 4, 8, false);
    let moves = legal_moves(&state, WHITE).unwrap();
    assert_eq!(moves.len(), 1);
    assert_eq!(
        moves[0].landings,
        [Position::new(3, 9), Position::new(5, 7)]
    );
    let turn = play_move(&state, &moves[0]).unwrap();
    assert!(!is_king(turn.working.entity(actor).unwrap()));
    assert!(turn.steps.iter().all(|s| s.presentation.is_empty()));
}
#[test]
fn international_does_not_continue_as_king_on_reaching_last_row() {
    let mut state = empty(DraughtsVariant::International);
    let actor = add(&mut state, 1, WHITE, 1, 7, false);
    add(&mut state, 2, BLACK, 2, 8, false);
    add(&mut state, 3, BLACK, 6, 6, false);
    let moves = legal_moves(&state, WHITE).unwrap();
    assert_eq!(moves[0].captures.len(), 1);
    let turn = play_move(&state, &moves[0]).unwrap();
    assert!(is_king(turn.working.entity(actor).unwrap()));
}
#[test]
fn quiet_promotion_works_for_both_sides_and_variants() {
    for variant in [DraughtsVariant::Russian, DraughtsVariant::International] {
        for side in [WHITE, BLACK] {
            let mut state = empty(variant);
            let y = if side == WHITE { variant.size() - 2 } else { 1 };
            let actor = add(&mut state, 1, side, 2, y, false);
            state.set_active_players(vec![side]).unwrap();
            let turn = play_move(&state, &legal_moves(&state, side).unwrap()[0]).unwrap();
            assert!(is_king(turn.working.entity(actor).unwrap()));
        }
    }
}
#[test]
fn forced_chain_commits_as_one_turn_and_undo_redo_restore_everything() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, WHITE, 0, 0, false);
    add(&mut state, 2, BLACK, 1, 1, false);
    add(&mut state, 3, BLACK, 3, 3, false);
    let mut timeline = GameTimeline::new(state.clone()).unwrap();
    let turn = play_move(&state, &legal_moves(&state, WHITE).unwrap()[0]).unwrap();
    assert_eq!(turn.steps.len(), 2);
    timeline.commit_turn(turn).unwrap();
    let after = timeline.current().clone();
    assert_eq!(after.entities.len(), 1);
    assert_eq!(after.turn.active_players, [BLACK]);
    timeline.undo().unwrap();
    assert_eq!(timeline.current(), &state);
    timeline.redo().unwrap();
    assert_eq!(timeline.current(), &after);
}
#[test]
fn incomplete_or_forged_turns_are_rejected_without_mutation() {
    let state = standard_state();
    let before = state.clone();
    let mut movement = legal_moves(&state, WHITE).unwrap()[0].clone();
    movement.captures.push(EntityId::new(999));
    assert!(play_move(&state, &movement).is_err());
    assert_eq!(state, before);
}
#[test]
fn no_pieces_or_no_legal_moves_is_a_loss() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, BLACK, 0, 0, false);
    assert_eq!(outcome(&state).unwrap().winners, [BLACK]);
    add(&mut state, 2, WHITE, 7, 7, false);
    assert_eq!(outcome(&state).unwrap().winners, [BLACK]);
}
#[test]
fn threefold_uses_occupancy_and_side_to_move_and_survives_undo() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, WHITE, 0, 0, true);
    add(&mut state, 2, BLACK, 7, 5, true);
    let mut timeline = GameTimeline::new(state).unwrap();
    for _ in 0..2 {
        for (side, to) in [
            (WHITE, Position::new(1, 1)),
            (BLACK, Position::new(6, 4)),
            (WHITE, Position::new(0, 0)),
            (BLACK, Position::new(7, 5)),
        ] {
            let movement = legal_moves(timeline.current(), side)
                .unwrap()
                .into_iter()
                .find(|m| m.landings == [to])
                .unwrap();
            let turn = play_move(timeline.current(), &movement).unwrap();
            timeline.commit_turn(turn).unwrap();
        }
    }
    let evaluate = |timeline: &GameTimeline| {
        CheckersOutcomeRule
            .evaluate(RuleContext::from_state(
                timeline.current(),
                Some(timeline.history()),
            ))
            .unwrap()
    };
    assert_eq!(
        evaluate(&timeline).unwrap().key,
        "checkers.threefold_repetition"
    );
    timeline.undo().unwrap();
    assert!(evaluate(&timeline).is_none());
}
#[test]
fn king_move_draw_limits_are_variant_specific_and_win_takes_precedence() {
    for (variant, limit) in [
        (DraughtsVariant::Russian, 15),
        (DraughtsVariant::International, 25),
    ] {
        let mut state = empty(variant);
        add(&mut state, 1, WHITE, 0, 0, true);
        add(&mut state, 2, BLACK, 7, 5, true);
        state
            .ruleset_state
            .insert("checkers.draw.quiet.1", limit - 1_u64);
        state.ruleset_state.insert("checkers.draw.quiet.2", limit);
        let movement = legal_moves(&state, WHITE)
            .unwrap()
            .into_iter()
            .find(|m| m.landings == [Position::new(1, 1)])
            .unwrap();
        let turn = play_move(&state, &movement).unwrap();
        assert_eq!(
            outcome(&turn.working).unwrap().key,
            "checkers.king_moves_draw"
        );
        let mut won = turn.working.clone();
        won.remove_entity(EntityId::new(2)).unwrap();
        assert_eq!(outcome(&won).unwrap().key, "checkers.no_legal_moves");
    }
}
#[test]
fn man_moves_reset_quiet_counter_and_capture_resets_balance_counter() {
    let mut state = empty(DraughtsVariant::Russian);
    add(&mut state, 1, WHITE, 0, 0, false);
    add(&mut state, 2, BLACK, 1, 1, false);
    for key in [
        "checkers.draw.quiet.1",
        "checkers.draw.quiet.2",
        "checkers.draw.balance.1",
        "checkers.draw.balance.2",
    ] {
        state.ruleset_state.insert(key, 10_u64);
    }
    let turn = play_move(&state, &legal_moves(&state, WHITE).unwrap()[0]).unwrap();
    for key in [
        "checkers.draw.quiet.1",
        "checkers.draw.quiet.2",
        "checkers.draw.balance.1",
        "checkers.draw.balance.2",
    ] {
        assert_eq!(
            turn.working
                .ruleset_state
                .get(key)
                .and_then(StateValue::as_u64),
            Some(0)
        );
    }
}
#[test]
fn five_move_ending_counter_advances_after_formation() {
    let mut state = empty(DraughtsVariant::International);
    add(&mut state, 1, WHITE, 0, 0, true);
    add(&mut state, 2, BLACK, 7, 5, true);
    state.ruleset_state.insert("checkers.draw.short.1", 4_u64);
    state.ruleset_state.insert("checkers.draw.short.2", 5_u64);
    let movement = legal_moves(&state, WHITE)
        .unwrap()
        .into_iter()
        .find(|m| m.landings == [Position::new(1, 1)])
        .unwrap();
    let turn = play_move(&state, &movement).unwrap();
    assert_eq!(
        outcome(&turn.working).unwrap().key,
        "checkers.five_move_draw"
    );
}

#[test]
fn international_majority_is_preserved_during_continuation_and_other_pieces_cannot_move() {
    let mut state = empty(DraughtsVariant::International);
    add(&mut state, 1, WHITE, 0, 0, false);
    add(&mut state, 2, BLACK, 1, 1, false);
    add(&mut state, 3, BLACK, 1, 3, false);
    add(&mut state, 4, BLACK, 3, 3, false);
    add(&mut state, 5, BLACK, 5, 5, false);
    add(&mut state, 6, WHITE, 8, 0, false);
    let mut driver = InteractionDriver::new(
        CheckersInteractionRules,
        TurnSession::new(&state, WHITE).unwrap(),
    )
    .unwrap();
    choose(&mut driver, 2, 2);
    assert_eq!(driver.interaction().choices.len(), 1);
    assert!(
        matches!(driver.interaction().choices[0].kind, ChoiceKind::SelectPosition { position } if position == Position::new(4,4))
    );
    assert!(CheckersOutcomeRule
        .evaluate(RuleContext::from_turn(driver.turn(), None))
        .unwrap()
        .is_none());
    assert!(driver
        .turn()
        .working
        .entities
        .contains_key(&EntityId::new(2)));
}
#[test]
fn international_sixteen_move_clock_survives_a_capture_reducing_material() {
    let mut state = empty(DraughtsVariant::International);
    add(&mut state, 1, WHITE, 0, 0, true);
    add(&mut state, 2, WHITE, 2, 2, false);
    add(&mut state, 3, WHITE, 6, 0, false);
    add(&mut state, 4, BLACK, 3, 3, true);
    state.set_active_players(vec![BLACK]).unwrap();
    state.ruleset_state.insert("checkers.draw.long.1", 16_u64);
    state.ruleset_state.insert("checkers.draw.long.2", 15_u64);
    let movement = legal_moves(&state, BLACK)
        .unwrap()
        .into_iter()
        .find(|m| m.landings == [Position::new(1, 1)])
        .unwrap();
    let turn = play_move(&state, &movement).unwrap();
    assert_eq!(turn.working.entities.len(), 3);
    assert_eq!(
        outcome(&turn.working).unwrap().key,
        "checkers.limited_ending_draw"
    );
}
#[test]
fn russian_thirty_and_sixty_move_material_clocks_have_distinct_thresholds() {
    for (total, limit) in [(4, 30_u64), (6, 60)] {
        let mut state = empty(DraughtsVariant::Russian);
        add(&mut state, 1, WHITE, 0, 0, true);
        add(&mut state, 2, BLACK, 7, 5, true);
        add(&mut state, 3, WHITE, 4, 0, false);
        add(&mut state, 4, BLACK, 3, 7, false);
        if total == 6 {
            add(&mut state, 5, WHITE, 2, 0, false);
            add(&mut state, 6, BLACK, 5, 7, false);
        }
        state
            .ruleset_state
            .insert("checkers.draw.balance.1", limit - 1);
        state.ruleset_state.insert("checkers.draw.balance.2", limit);
        let movement = legal_moves(&state, WHITE)
            .unwrap()
            .into_iter()
            .find(|m| m.actor == EntityId::new(3) && m.landings == [Position::new(5, 1)])
            .unwrap();
        let turn = play_move(&state, &movement).unwrap();
        assert_eq!(
            outcome(&turn.working).unwrap().key,
            "checkers.material_balance_draw"
        );
    }
}
#[test]
fn initial_two_ply_counts_match_opening_reference() {
    for (variant, expected) in [
        (DraughtsVariant::Russian, 49),
        (DraughtsVariant::International, 81),
    ] {
        let state = state_for_variant(variant);
        let mut nodes = 0;
        for movement in legal_moves(&state, WHITE).unwrap() {
            let turn = play_move(&state, &movement).unwrap();
            nodes += legal_moves(&turn.working, BLACK).unwrap().len();
        }
        assert_eq!(nodes, expected);
    }
}

#[test]
fn international_ending_clock_preserves_stronger_side_after_capture_to_king_vs_king() {
    let mut state = empty(DraughtsVariant::International);
    add(&mut state, 1, WHITE, 0, 0, true);
    add(&mut state, 2, BLACK, 2, 2, false);
    add(&mut state, 3, BLACK, 9, 7, true);
    state.ruleset_state.insert("checkers.draw.long.1", 15_u64);
    state.ruleset_state.insert("checkers.draw.long.2", 16_u64);
    let movement = legal_moves(&state, WHITE)
        .unwrap()
        .into_iter()
        .find(|m| m.landings == [Position::new(3, 3)])
        .unwrap();
    let turn = play_move(&state, &movement).unwrap();
    assert_eq!(
        outcome(&turn.working).unwrap().key,
        "checkers.limited_ending_draw"
    );
}

#[test]
fn notation_records_every_capture_landing_in_both_variants() {
    for (variant, expected) in [
        (DraughtsVariant::Russian, "b2xd4xf6"),
        (DraughtsVariant::International, "41x32x23"),
    ] {
        let mut state = empty(variant);
        add(&mut state, 1, WHITE, 1, 1, false);
        add(&mut state, 2, BLACK, 2, 2, false);
        add(&mut state, 3, BLACK, 4, 4, false);
        add(&mut state, 4, BLACK, 0, 6, false);
        let movement = legal_moves(&state, WHITE)
            .unwrap()
            .into_iter()
            .find(|m| m.landings == [Position::new(3, 3), Position::new(5, 5)])
            .unwrap();
        let turn = play_move(&state, &movement).unwrap();
        let mut timeline = monumla_core::GameTimeline::new(state).unwrap();
        timeline.commit_turn(turn).unwrap();
        assert_eq!(
            turn_notation(timeline.history().previous_turn().unwrap()),
            expected
        );
    }
}
