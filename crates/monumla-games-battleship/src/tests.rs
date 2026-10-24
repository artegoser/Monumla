use super::*;
fn choose(t: &mut GameTimeline, predicate: impl Fn(&Choice) -> bool) {
    let actor = t.current().turn.active_players[0];
    let mut d = InteractionDriver::new(BattleshipRules, t.begin_turn(actor).unwrap()).unwrap();
    let c = d
        .interaction()
        .choices
        .iter()
        .find(|c| predicate(c))
        .expect("legal choice")
        .id;
    d.choose(c).unwrap();
    t.commit_turn(d.into_turn().unwrap()).unwrap();
}
fn option(t: &mut GameTimeline, key: &str) {
    choose(
        t,
        |c| matches!(&c.kind,ChoiceKind::SelectOption{key:k} if k==key),
    )
}
fn shoot(t: &mut GameTimeline, target: Position) {
    choose(
        t,
        |c| matches!(c.kind,ChoiceKind::SelectPosition{position} if position==target),
    )
}
fn battle(seed: u64) -> GameTimeline {
    let mut t = GameTimeline::new(initial_state(seed)).unwrap();
    option(&mut t, "arrange");
    option(&mut t, "ready");
    option(&mut t, "arrange");
    option(&mut t, "ready");
    t
}
#[test]
fn manual_placement_rejects_edges_overlap_and_diagonal_touch() {
    let mut s = initial_state(1);
    place_ship(&mut s, FIRST, 0, 0, false).unwrap();
    assert!(place_ship(&mut s, FIRST, 1, 14, false).is_err());
    assert!(place_ship(&mut s, FIRST, 1, 0, true).is_err());
    assert!(place_ship(&mut s, FIRST, 1, 98, false).is_err());
    assert!(place_ship(&mut s, FIRST, 1, 20, false).is_ok());
    assert!(validate_fleet(&s, FIRST).is_err());
}
#[test]
fn random_fleets_are_complete_and_different() {
    let mut layouts = std::collections::BTreeSet::new();
    for seed in 0..100 {
        let t = battle(seed);
        for p in [FIRST, SECOND] {
            validate_fleet(t.current(), p).unwrap();
        }
        layouts.insert(format!("{:?}", fleet(t.current(), FIRST)));
    }
    assert!(layouts.len() > 90);
}
#[test]
fn miss_passes_turn_hit_retains_and_sunk_exposes_only_known_hull() {
    let mut t = battle(4);
    let state = t.current();
    let miss = (0..100)
        .map(|c| Position::new(10 + (c % 10) as u16, (c / 10) as u16))
        .find(|p| state.entity_at(*p).unwrap().is_none())
        .unwrap();
    shoot(&mut t, miss);
    assert_eq!(t.current().turn.active_players, vec![SECOND]);
    let parts = fleet(t.current(), FIRST)
        .into_iter()
        .find(|(_, c)| c.len() == 4)
        .unwrap()
        .1;
    for (i, c) in parts.iter().enumerate() {
        shoot(&mut t, Position::new((c % 10) as u16, (c / 10) as u16));
        assert_eq!(t.current().turn.active_players, vec![SECOND]);
        assert_eq!(
            shot(t.current(), FIRST, *c),
            if i == 3 { "sunk" } else { "hit" }
        );
    }
    assert!(parts.iter().all(|c| shot(t.current(), FIRST, *c) == "sunk"));
    let old = t.current().clone();
    t.undo().unwrap();
    assert_eq!(shot(t.current(), FIRST, parts[3]), "");
    t.redo().unwrap();
    assert_eq!(*t.current(), old);
}
#[test]
fn sinking_twenty_decks_finishes_game_and_repeated_shots_are_unavailable() {
    let mut t = battle(7);
    let targets = t
        .current()
        .entities
        .values()
        .filter(|e| e.owner == SECOND)
        .map(|e| e.position)
        .collect::<Vec<_>>();
    for target in targets {
        shoot(&mut t, target);
    }
    assert_eq!(phase(t.current()), "finished");
    assert_eq!(number(t.current(), "winner"), 1);
    assert!(BattleshipRules
        .choices(&t.begin_turn(FIRST).unwrap(), &StateMap::new())
        .unwrap()
        .is_empty());
}
#[test]
fn observation_is_independent_of_enemy_layout_seeds_and_history() {
    let t = battle(15);
    let mut other = t.current().clone();
    let replacement = generate(&other, SECOND, 999, false).unwrap();
    install(&mut other, SECOND, &replacement).unwrap();
    other
        .players
        .get_mut(&FIRST)
        .unwrap()
        .state
        .insert("seed", 123_u64);
    let a = FleetInformation::default()
        .observe(t.current(), FIRST)
        .unwrap();
    let b = FleetInformation::default().observe(&other, FIRST).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.entities.len(), 20);
    assert!(a.players.values().all(|p| p.state.is_empty()));
    assert!(FleetInformation::default().observe(&a, SECOND).is_err());
    assert_eq!(
        FleetInformation::default().sample(&a, FIRST, 1).unwrap(),
        FleetInformation::default().sample(&b, FIRST, 1).unwrap()
    );
}
#[test]
fn samples_preserve_all_public_results_and_hit_constraints() {
    let mut t = battle(66);
    let targets = t
        .current()
        .entities
        .values()
        .filter(|e| e.owner == SECOND)
        .map(|e| e.position)
        .collect::<Vec<_>>();
    for target in targets.iter().take(7) {
        shoot(&mut t, *target);
    }
    let observation = FleetInformation::default()
        .observe(t.current(), FIRST)
        .unwrap();
    for seed in 0..30 {
        let s = FleetInformation::default()
            .sample(&observation, FIRST, seed)
            .unwrap();
        validate_fleet(&s, SECOND).unwrap();
        for c in 0..100 {
            let occupied = s
                .entity_at(Position::new(10 + (c % 10) as u16, (c / 10) as u16))
                .unwrap()
                .is_some();
            match shot(&s, SECOND, c) {
                "hit" | "sunk" => assert!(occupied),
                "miss" | "water" => assert!(!occupied),
                _ => {}
            }
        }
    }
}
#[test]
fn setup_and_actual_shots_have_readable_notation() {
    let mut t = battle(3);
    assert_eq!(turn_notation(t.history().previous_turn().unwrap()), "Ready");
    let target = t
        .current()
        .entities
        .values()
        .find(|e| e.owner == SECOND)
        .unwrap()
        .position;
    shoot(&mut t, target);
    let text = turn_notation(t.history().previous_turn().unwrap());
    assert!(text.contains(" · hit") || text.contains(" · sunk"));
}

#[test]
fn placed_ship_rotation_checks_borders_and_neighbors_atomically() {
    let mut s = initial_state(1);
    place_ship(&mut s, FIRST, 0, 0, false).unwrap();
    rotate_ship(&mut s, FIRST, 0).unwrap();
    assert_eq!(fleet(&s, FIRST)[0].1, vec![0, 10, 20, 30]);
    rotate_ship(&mut s, FIRST, 0).unwrap();
    assert_eq!(fleet(&s, FIRST)[0].1, vec![0, 1, 2, 3]);
    place_ship(&mut s, FIRST, 1, 20, false).unwrap();
    let before = s.clone();
    assert!(rotate_ship(&mut s, FIRST, 0).is_err());
    assert_eq!(s, before);
}

#[test]
fn fleets_reveal_only_at_game_end_and_hide_again_on_undo() {
    let mut t = battle(9);
    assert_eq!(
        FleetInformation::default()
            .observe(t.current(), SECOND)
            .unwrap()
            .entities
            .len(),
        20
    );
    let positions = t
        .current()
        .entities
        .values()
        .filter(|e| e.owner == SECOND)
        .map(|e| e.position)
        .collect::<Vec<_>>();
    for p in positions {
        shoot(&mut t, p);
    }
    let final_view = FleetInformation::default()
        .observe(t.current(), SECOND)
        .unwrap();
    assert_eq!(final_view.entities.len(), 40);
    assert!(final_view.players.values().all(|p| p.state.is_empty()));
    assert_eq!(
        FleetInformation::default()
            .sample(&final_view, SECOND, 1)
            .unwrap()
            .entities,
        t.current().entities
    );
    t.undo().unwrap();
    assert_eq!(
        FleetInformation::default()
            .observe(t.current(), SECOND)
            .unwrap()
            .entities
            .len(),
        20
    );
    t.redo().unwrap();
    assert_eq!(
        FleetInformation::default()
            .observe(t.current(), SECOND)
            .unwrap()
            .entities
            .len(),
        40
    );
}
#[test]
fn battle_score_has_no_setup_offset() {
    let t = battle(4);
    for p in [FIRST, SECOND] {
        let observation = FleetInformation::default().observe(t.current(), p).unwrap();
        assert_eq!(
            registry()
                .position_score(RuleContext::from_state(&observation, None), p)
                .unwrap(),
            Some(0)
        );
    }
}

#[test]
fn hit_continuation_is_not_biased_to_the_longest_remaining_ship() {
    let mut s = battle(12).current().clone();
    for (cell, result) in [(37, "hit"), (47, "hit"), (57, "miss")] {
        s.ruleset_state.insert(format!("shot.2.{cell}"), result);
    }
    let info = FleetInformation::default();
    let observed = info.observe(&s, FIRST).unwrap();
    let mut near = 0;
    let mut far = 0;
    for seed in 1..=300 {
        let sample = info.sample(&observed, FIRST, seed).unwrap();
        validate_fleet(&sample, SECOND).unwrap();
        if sample.entity_at(Position::new(17, 2)).unwrap().is_some() {
            near += 1;
        }
        if sample.entity_at(Position::new(17, 1)).unwrap().is_some() {
            far += 1;
        }
    }
    assert_eq!(
        near, 300,
        "H3 must continue the damaged hull when H6 is water"
    );
    assert!(
        far > 0 && far < 290,
        "H2 is occupied only by a compatible four-deck hull: {far}"
    );
}
