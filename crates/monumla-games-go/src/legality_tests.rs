
use super::*;
use monumla_core::{GameTimeline, InteractionDriver};
#[test]
fn group_facts_match_full_transaction_legality_during_play() {
    for size in [3, 5, 9, 19] {
        let mut timeline = GameTimeline::new(empty_state(size)).unwrap();
        for ply in 0..40 {
            let s = timeline.current();
            let p = active_player(s).unwrap();
            let seen = historical_play_positions(Some(timeline.history())).unwrap();
            let facts = legality::PlacementFacts::new(s).unwrap();
            for point in s.board.positions() {
                assert_eq!(
                    facts.legal(s, &seen, p, point).unwrap(),
                    placement_is_legal(s, &seen, p, point).unwrap(),
                    "size={size} ply={ply} point={point:?}"
                );
            }
            let rules = GoInteractionRules::with_history(timeline.history()).unwrap();
            let mut driver =
                InteractionDriver::new(rules, timeline.begin_turn(p).unwrap()).unwrap();
            let moves = driver
                .interaction()
                .choices
                .iter()
                .filter(|c| matches!(c.kind, ChoiceKind::SelectPosition { .. }))
                .collect::<Vec<_>>();
            if moves.is_empty() {
                break;
            }
            let id = moves[(ply * 7919 + size as usize * 17) % moves.len()].id;
            driver.choose(id).unwrap();
            timeline.commit_turn(driver.into_turn().unwrap()).unwrap();
        }
    }
}
