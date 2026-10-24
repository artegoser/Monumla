use super::*;
fn play(t: &mut GameTimeline, p: Position) {
    let actor = t.current().turn.active_players[0];
    let mut d = InteractionDriver::new(TicTacToeRules, t.begin_turn(actor).unwrap()).unwrap();
    let id = d
        .interaction()
        .choices
        .iter()
        .find(|c| matches!(c.kind,ChoiceKind::SelectPosition{position} if position==p))
        .unwrap()
        .id;
    d.choose(id).unwrap();
    t.commit_turn(d.into_turn().unwrap()).unwrap();
}
#[test]
fn config_rejects_impossible_boards() {
    for (w, h, k) in [(2, 3, 3), (10, 3, 3), (3, 3, 4), (4, 4, 2)] {
        assert!(Config::new(w, h, k).is_err())
    }
    assert!(Config::new(7, 5, 5).is_ok());
}
#[test]
fn horizontal_vertical_diagonal_wins_for_every_board() {
    for config in [
        Config::standard(),
        Config::new(4, 4, 4).unwrap(),
        Config::new(5, 5, 4).unwrap(),
        Config::new(7, 5, 5).unwrap(),
    ] {
        for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
            let mut s = initial_state(config).unwrap();
            for i in 0..config.connect {
                let y = if dy < 0 {
                    config.connect - 1 - i
                } else {
                    i * (dy as u16)
                };
                let pos = Position::new(i * (dx as u16), y);
                s.add_entity(EntityState::new(
                    EntityId::new(u32::from(i) + 1),
                    MARK,
                    CROSS,
                    pos,
                ))
                .unwrap();
            }
            assert_eq!(outcome(&s).unwrap().winners, vec![CROSS]);
        }
    }
}
#[test]
fn draw_and_history_are_reversible() {
    let mut t = GameTimeline::new(initial_state(Config::standard()).unwrap()).unwrap();
    for (x, y) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (0, 1),
        (0, 2),
        (2, 0),
        (1, 0),
        (1, 2),
        (2, 1),
    ] {
        play(&mut t, Position::new(x, y));
    }
    assert!(outcome(t.current()).unwrap().winners.is_empty());
    let end = t.current().clone();
    t.undo().unwrap();
    assert!(outcome(t.current()).is_none());
    t.redo().unwrap();
    assert_eq!(*t.current(), end);
    assert_eq!(turn_notation(t.history().previous_turn().unwrap()), "C2");
}
