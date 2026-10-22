#[cfg(test)]
mod tests {
    use crate::{pieces::pawn::Pawn, ChessRules, ChessSide, BISHOP, KNIGHT, PAWN, QUEEN, ROOK, WHITE_PLAYER};
    use monumla_core::{Position, TurnSession};

    #[test]
    fn every_standard_promotion_type_executes() {
        let rules = ChessRules::standard();
        for promotion in [QUEEN, ROOK, BISHOP, KNIGHT] {
            let imported = rules
                .from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1")
                .unwrap();
            let state = imported.timeline.current().clone();
            let pawn = state.entity_at(Position::new(0, 6)).unwrap().unwrap().id;
            assert_eq!(state.entity(pawn).unwrap().entity_type, PAWN);
            let movement = rules
                .legal_moves_with_history(&state, Some(imported.timeline.history()), pawn)
                .unwrap()
                .into_iter()
                .find(|movement| movement.to == Position::new(0, 7))
                .unwrap();
            let mut turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
            let input = Pawn::promotion_input(promotion).unwrap();
            rules
                .execute_move(
                    &mut turn,
                    Some(imported.timeline.history()),
                    movement,
                    Some(&input),
                )
                .unwrap();
            let promoted = turn.working.entity(pawn).unwrap();
            assert_eq!(promoted.entity_type, promotion);
            if promotion == ROOK {
                assert!(crate::piece::has_moved(promoted));
            } else {
                assert!(!promoted.state.contains_key(crate::piece::HAS_MOVED_STATE));
            }
        }
    }

    #[test]
    fn undo_redo_restores_castling_relevant_entity_state() {
        let rules = ChessRules::standard();
        let mut timeline = rules
            .from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1")
            .unwrap()
            .timeline;
        let initial_fen = rules.to_fen(timeline.current(), timeline.history()).unwrap();
        let rook = timeline
            .current()
            .entity_at(Position::new(0, 0))
            .unwrap()
            .unwrap()
            .id;
        let history = timeline.history().clone();
        let movement = rules
            .legal_moves_with_history(timeline.current(), Some(&history), rook)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(0, 1))
            .unwrap();
        let mut turn = timeline.begin_turn(ChessSide::White.player()).unwrap();
        rules
            .execute_move(&mut turn, Some(&history), movement, None)
            .unwrap();
        timeline.commit_turn(turn).unwrap();
        let moved_fen = rules.to_fen(timeline.current(), timeline.history()).unwrap();
        assert_ne!(moved_fen, initial_fen);
        assert!(crate::piece::has_moved(timeline.current().entity(rook).unwrap()));

        timeline.undo().unwrap();
        assert_eq!(rules.to_fen(timeline.current(), timeline.history()).unwrap(), initial_fen);
        assert!(!crate::piece::has_moved(timeline.current().entity(rook).unwrap()));

        timeline.redo().unwrap();
        assert_eq!(rules.to_fen(timeline.current(), timeline.history()).unwrap(), moved_fen);
        assert!(crate::piece::has_moved(timeline.current().entity(rook).unwrap()));
    }
}
