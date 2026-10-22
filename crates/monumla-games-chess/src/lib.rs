//! Standard chess rules implemented on top of `monumla-core`.
#![forbid(unsafe_code)]

#[cfg(test)]
mod correctness;
mod error;
mod fen;
mod game;
mod interaction;
mod notation;
mod outcome;
mod perft;
mod piece;
mod pieces;
pub mod search;

pub use error::ChessError;
pub use fen::{FenGame, STANDARD_FEN};
pub use game::{
    empty_chess_state, standard_chess_state, ChessRules, BLACK_PLAYER, BLACK_TEAM, WHITE_PLAYER,
    WHITE_TEAM,
};
pub use interaction::ChessInteractionRules;
pub use notation::PgnGame;
pub use outcome::{ChessDrawClaim, ChessOutcome, ChessOutcomeRule, ChessStatus, PositionKey};
pub use piece::{
    ChessMoveKind, ChessPieceContext, ChessPieceKind, ChessPieceRule, PseudoMove, BISHOP, KING, KNIGHT, PAWN,
    QUEEN, ROOK,
};
pub use pieces::{Bishop, King, Knight, Pawn, Queen, Rook};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChessSide {
    White,
    Black,}

#[cfg(test)]
mod tests {
    use super::*;
    use monumla_core::{
        EntityId, EntityRule, EntityState, EntityTypeId, PlayerId, Position, RuleContext,
        RuleRegistry,
    };
    use std::collections::BTreeSet;

    fn add_piece(
        state: &mut monumla_core::GameState,
        id: u32,
        entity_type: EntityTypeId,
        side: ChessSide,
        position: Position,
    ) -> EntityId {
        let id = EntityId::new(id);
        state
            .add_entity(EntityState::new(id, entity_type, side.player(), position))
            .unwrap();
        id
    }

    fn destinations(moves: &[PseudoMove]) -> BTreeSet<Position> {
        moves.iter().map(|movement| movement.to).collect()
    }

    struct KnightOnlyPromotionRule;

    impl monumla_core::GameRule for KnightOnlyPromotionRule {
        fn transform_choices(
            &self,
            _context: monumla_core::RuleContext<'_>,
            _actor: PlayerId,
            _draft: &monumla_core::StateMap,
            choices: Vec<monumla_core::ChoiceSpec>,
        ) -> Result<Vec<monumla_core::ChoiceSpec>, monumla_core::RuleError> {
            Ok(choices
                .into_iter()
                .filter(|choice| {
                    if !matches!(&choice.kind, monumla_core::ChoiceKind::SelectOption { .. }) {
                        return true;
                    }
                    choice
                        .data
                        .get("entity_type")
                        .and_then(monumla_core::StateValue::as_u64)
                        == Some(u64::from(KNIGHT.get()))
                })
                .collect())
        }
    }

    struct MarkMovedEntityRule;

    impl monumla_core::GameRule for MarkMovedEntityRule {
        fn react(
            &self,
            _before: &monumla_core::GameState,
            action: &monumla_core::RecordedAction,
            transaction: &mut monumla_core::Transaction,
        ) -> Result<(), monumla_core::RuleError> {
            if action.kind != "chess_move" {
                return Ok(());
            }
            let Some(actor) = action
                .data
                .get("actor")
                .and_then(monumla_core::StateValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .map(EntityId::new)
            else {
                return Ok(());
            };
            transaction.entity_mut(actor)?.state.insert("game_rule_reacted", true);
            Ok(())
        }
    }

    #[test]
    fn standard_setup_contains_all_pieces_and_players() {
        let state = standard_chess_state().unwrap();
        assert_eq!(state.board.width(), 8);
        assert_eq!(state.board.height(), 8);
        assert_eq!(state.entities.len(), 32);
        assert_eq!(state.turn.active_players, vec![WHITE_PLAYER]);
        assert_eq!(
            state
                .entity_at(Position::new(4, 0))
                .unwrap()
                .unwrap()
                .entity_type,
            KING
        );
        assert_eq!(
            state
                .entity_at(Position::new(3, 7))
                .unwrap()
                .unwrap()
                .entity_type,
            QUEEN
        );
        assert_eq!(
            state
                .entity_at(Position::new(0, 1))
                .unwrap()
                .unwrap()
                .entity_type,
            PAWN
        );
        assert_eq!(
            state
                .entity_at(Position::new(7, 6))
                .unwrap()
                .unwrap()
                .entity_type,
            PAWN
        );
        state.validate().unwrap();
    }

    #[test]
    fn standard_movement_history_state_is_sparse_and_piece_local() {
        let state = standard_chess_state().unwrap();
        for entity in state.entities.values() {
            if entity.entity_type == KING || entity.entity_type == ROOK {
                assert_eq!(
                    entity
                        .state
                        .get(crate::piece::HAS_MOVED_STATE)
                        .and_then(monumla_core::StateValue::as_bool),
                    Some(false)
                );
            } else {
                assert!(!entity.state.contains_key(crate::piece::HAS_MOVED_STATE));
            }
        }
    }

    #[test]
    fn standard_registry_contains_all_six_piece_rules() {
        let rules = ChessRules::standard();
        for entity_type in [PAWN, KNIGHT, BISHOP, ROOK, QUEEN, KING] {
            assert!(rules.piece_rule(entity_type).is_ok());
        }
    }

    #[test]
    fn pawn_moves_attacks_captures_and_blocking_are_distinct() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let pawn = add_piece(&mut state, 1, PAWN, ChessSide::White, Position::new(3, 1));
        let enemy = add_piece(&mut state, 2, KNIGHT, ChessSide::Black, Position::new(4, 2));

        let moves = rules.pseudo_moves(&state, pawn).unwrap();
        assert_eq!(
            destinations(&moves),
            BTreeSet::from([
                Position::new(3, 2),
                Position::new(3, 3),
                Position::new(4, 2),
            ])
        );
        assert_eq!(
            moves
                .iter()
                .find(|movement| movement.to == Position::new(4, 2))
                .unwrap()
                .capture,
            Some(enemy)
        );
        assert_eq!(
            rules
                .attacks(&state, pawn)
                .unwrap()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([Position::new(2, 2), Position::new(4, 2)])
        );

        add_piece(&mut state, 3, BISHOP, ChessSide::White, Position::new(3, 2));
        let blocked = rules.pseudo_moves(&state, pawn).unwrap();
        assert_eq!(
            destinations(&blocked),
            BTreeSet::from([Position::new(4, 2)])
        );
    }

    #[test]
    fn pawn_double_move_is_derived_from_start_rank() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let pawn = add_piece(&mut state, 1, PAWN, ChessSide::White, Position::new(3, 3));
        assert_eq!(
            destinations(&rules.pseudo_moves(&state, pawn).unwrap()),
            BTreeSet::from([Position::new(3, 4)])
        );

        state.remove_entity(pawn).unwrap();
        let pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(3, 1));
        assert_eq!(
            destinations(&rules.pseudo_moves(&state, pawn).unwrap()),
            BTreeSet::from([Position::new(3, 2), Position::new(3, 3)])
        );
    }

    #[test]
    fn knight_generates_leaps_and_ignores_intervening_pieces() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let knight = add_piece(&mut state, 1, KNIGHT, ChessSide::White, Position::new(1, 0));
        add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(1, 1));
        assert_eq!(
            destinations(&rules.pseudo_moves(&state, knight).unwrap()),
            BTreeSet::from([
                Position::new(0, 2),
                Position::new(2, 2),
                Position::new(3, 1)
            ])
        );
    }

    #[test]
    fn sliding_piece_stops_at_first_occupied_square() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let rook = add_piece(&mut state, 1, ROOK, ChessSide::White, Position::new(3, 3));
        add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(3, 5));
        let enemy = add_piece(&mut state, 3, PAWN, ChessSide::Black, Position::new(5, 3));

        let moves = rules.pseudo_moves(&state, rook).unwrap();
        assert!(moves
            .iter()
            .any(|movement| movement.to == Position::new(3, 4)));
        assert!(!moves
            .iter()
            .any(|movement| movement.to == Position::new(3, 5)));
        assert!(!moves
            .iter()
            .any(|movement| movement.to == Position::new(3, 6)));
        assert_eq!(
            moves
                .iter()
                .find(|movement| movement.to == Position::new(5, 3))
                .unwrap()
                .capture,
            Some(enemy)
        );
        assert!(!moves
            .iter()
            .any(|movement| movement.to == Position::new(6, 3)));

        let attacks = rules.attacks(&state, rook).unwrap();
        assert!(attacks.contains(&Position::new(3, 5)));
        assert!(attacks.contains(&Position::new(5, 3)));
        assert!(!attacks.contains(&Position::new(3, 6)));
        assert!(!attacks.contains(&Position::new(6, 3)));
    }

    #[test]
    fn bishop_queen_and_king_generate_expected_geometry() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let bishop = add_piece(&mut state, 1, BISHOP, ChessSide::White, Position::new(3, 3));
        let queen = add_piece(&mut state, 2, QUEEN, ChessSide::White, Position::new(0, 0));
        let king = add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(7, 7));

        let bishop_moves = destinations(&rules.pseudo_moves(&state, bishop).unwrap());
        assert!(bishop_moves.contains(&Position::new(4, 4)));
        assert!(bishop_moves.contains(&Position::new(2, 4)));
        assert!(!bishop_moves.contains(&Position::new(3, 4)));

        let queen_moves = destinations(&rules.pseudo_moves(&state, queen).unwrap());
        assert!(queen_moves.contains(&Position::new(0, 6)));
        assert!(queen_moves.contains(&Position::new(6, 0)));
        assert!(queen_moves.contains(&Position::new(2, 2)));

        assert_eq!(rules.attacks(&state, king).unwrap().len(), 3);
    }

    #[test]
    fn standard_piece_presentation_is_state_context_driven() {
        let state = standard_chess_state().unwrap();
        let pawn = state.entity_at(Position::new(0, 1)).unwrap().unwrap();
        let rule = Pawn;
        let context = RuleContext::from_state(&state, None)
            .entity_context(pawn.id)
            .unwrap();
        let presentation = rule.presentation(context).unwrap();
        assert_eq!(presentation.asset_key, "chess/white/pawn");
    }

    #[test]
    fn side_mapping_is_chess_local() {
        assert_eq!(ChessSide::White.player(), WHITE_PLAYER);
        assert_eq!(ChessSide::Black.player(), BLACK_PLAYER);
        assert_eq!(ChessSide::from_player(WHITE_PLAYER), Some(ChessSide::White));
        assert_eq!(ChessSide::from_player(PlayerId::new(99)), None);
    }

    #[test]
    fn black_pawn_uses_the_opposite_forward_direction() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let pawn = add_piece(&mut state, 1, PAWN, ChessSide::Black, Position::new(4, 6));
        assert_eq!(
            destinations(&rules.pseudo_moves(&state, pawn).unwrap()),
            BTreeSet::from([Position::new(4, 4), Position::new(4, 5)])
        );
        assert_eq!(
            rules
                .attacks(&state, pawn)
                .unwrap()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([Position::new(3, 5), Position::new(5, 5)])
        );
    }

    fn legal_test_state() -> monumla_core::GameState {
        empty_chess_state().unwrap()
    }

    #[test]
    fn attack_maps_detect_check_and_pinned_piece_moves_are_filtered() {
        let rules = ChessRules::standard();
        let mut state = legal_test_state();
        let white_king = add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let pinned_rook = add_piece(&mut state, 2, ROOK, ChessSide::White, Position::new(4, 1));
        add_piece(&mut state, 3, ROOK, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 4, KING, ChessSide::Black, Position::new(7, 7));

        assert!(!rules.in_check(&state, ChessSide::White).unwrap());
        assert!(rules.is_square_attacked(&state, ChessSide::Black, Position::new(4, 1)).unwrap());

        let pseudo = destinations(&rules.pseudo_moves(&state, pinned_rook).unwrap());
        assert!(pseudo.contains(&Position::new(3, 1)));
        let legal = destinations(&rules.legal_moves(&state, pinned_rook).unwrap());
        assert!(!legal.contains(&Position::new(3, 1)));
        assert!(!legal.contains(&Position::new(5, 1)));
        assert!(legal.contains(&Position::new(4, 2)));
        assert_eq!(rules.king(&state, ChessSide::White).unwrap(), white_king);
    }

    #[test]
    fn king_cannot_move_or_capture_into_resulting_attack() {
        let rules = ChessRules::standard();
        let mut state = legal_test_state();
        let king = add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let victim = add_piece(&mut state, 2, ROOK, ChessSide::Black, Position::new(4, 1));
        add_piece(&mut state, 3, ROOK, ChessSide::Black, Position::new(7, 1));
        add_piece(&mut state, 4, KING, ChessSide::Black, Position::new(7, 7));

        let pseudo = rules.pseudo_moves(&state, king).unwrap();
        assert!(pseudo.iter().any(|movement| movement.capture == Some(victim)));
        let legal = rules.legal_moves(&state, king).unwrap();
        assert!(!legal.iter().any(|movement| movement.to == Position::new(4, 1)));
    }

    #[test]
    fn double_check_leaves_only_king_actions() {
        let rules = ChessRules::standard();
        let mut state = legal_test_state();
        let king = add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        add_piece(&mut state, 2, ROOK, ChessSide::White, Position::new(0, 0));
        add_piece(&mut state, 3, ROOK, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 4, BISHOP, ChessSide::Black, Position::new(1, 3));
        add_piece(&mut state, 5, KING, ChessSide::Black, Position::new(7, 7));

        assert!(rules.in_check(&state, ChessSide::White).unwrap());
        let legal = rules.legal_moves_for_side(&state, ChessSide::White).unwrap();
        assert!(!legal.is_empty());
        assert!(legal.iter().all(|movement| movement.actor == king));
    }

    #[test]
    fn chess_interaction_exposes_only_legal_destinations() {
        use monumla_core::{ChoiceKind, InteractionDriver, TurnSession};

        let rules = ChessRules::standard();
        let mut state = legal_test_state();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let rook = add_piece(&mut state, 2, ROOK, ChessSide::White, Position::new(4, 1));
        add_piece(&mut state, 3, ROOK, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 4, KING, ChessSide::Black, Position::new(7, 7));

        let turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let mut driver = InteractionDriver::new(ChessInteractionRules::new(&rules), turn).unwrap();
        let rook_choice = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectEntity { entity } if entity == rook))
            .unwrap()
            .id;
        driver.choose(rook_choice).unwrap();

        let is_rook_destination = |choice: &monumla_core::Choice, target: Position| {
            matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == target)
                && choice
                    .data
                    .get("actor")
                    .and_then(monumla_core::StateValue::as_u64)
                    == Some(u64::from(rook.get()))
        };
        assert!(!driver
            .interaction()
            .choices
            .iter()
            .any(|choice| is_rook_destination(choice, Position::new(3, 1))));
        assert!(driver
            .interaction()
            .choices
            .iter()
            .any(|choice| is_rook_destination(choice, Position::new(4, 2))));
    }

    #[test]
    fn chess_destinations_are_actor_bound_and_can_execute_without_click_selection() {
        use monumla_core::{ChoiceKind, InteractionDriver, InteractionUpdate, TurnSession};

        let rules = ChessRules::standard();
        let state = standard_chess_state().unwrap();
        let pawn = state.entity_at(Position::new(4, 1)).unwrap().unwrap().id;
        let turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let mut driver = InteractionDriver::new(ChessInteractionRules::new(&rules), turn).unwrap();

        let destination = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| {
                matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(4, 3))
                    && choice
                        .data
                        .get("actor")
                        .and_then(monumla_core::StateValue::as_u64)
                        == Some(u64::from(pawn.get()))
            })
            .unwrap()
            .id;

        assert_eq!(driver.choose(destination).unwrap(), InteractionUpdate::Finished);
        assert_eq!(
            driver.turn().working.entity(pawn).unwrap().position,
            Position::new(4, 3)
        );
    }

    #[test]
    fn selecting_the_same_piece_twice_clears_chess_selection() {
        use monumla_core::{ChoiceKind, InteractionDriver, TurnSession};

        let rules = ChessRules::standard();
        let state = standard_chess_state().unwrap();
        let pawn = state.entity_at(Position::new(4, 1)).unwrap().unwrap().id;
        let turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let mut driver = InteractionDriver::new(ChessInteractionRules::new(&rules), turn).unwrap();

        for expected_selected in [Some(pawn), None] {
            let choice = driver
                .interaction()
                .choices
                .iter()
                .find(|choice| matches!(choice.kind, ChoiceKind::SelectEntity { entity } if entity == pawn))
                .unwrap()
                .id;
            driver.choose(choice).unwrap();
            assert_eq!(ChessInteractionRules::selected_entity(driver.draft()), expected_selected);
        }
    }

    #[test]
    fn en_passant_is_derived_from_the_previous_committed_turn() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let white_pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(4, 4));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let black_pawn = add_piece(&mut state, 4, PAWN, ChessSide::Black, Position::new(3, 6));
        state.set_active_players(vec![BLACK_PLAYER]).unwrap();

        let mut timeline = GameTimeline::new(state).unwrap();
        let mut black_turn = timeline.begin_turn(BLACK_PLAYER).unwrap();
        let double = rules
            .legal_moves(&black_turn.working, black_pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(3, 4))
            .unwrap();
        rules.execute_move(&mut black_turn, None, double, None).unwrap();
        timeline.commit_turn(black_turn).unwrap();

        let ep = rules
            .legal_moves_with_history(timeline.current(), Some(timeline.history()), white_pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(3, 5))
            .unwrap();
        assert!(matches!(ep.kind, ChessMoveKind::EnPassant { victim } if victim == black_pawn));

        let mut white_turn = timeline.begin_turn(WHITE_PLAYER).unwrap();
        rules
            .execute_move(&mut white_turn, Some(timeline.history()), ep, None)
            .unwrap();
        assert!(!white_turn.working.entities.contains_key(&black_pawn));
        assert_eq!(
            white_turn.working.entity(white_pawn).unwrap().position,
            Position::new(3, 5)
        );
    }

    #[test]
    fn black_can_en_passant_after_white_double_move() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let white_pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(3, 1));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let black_pawn = add_piece(&mut state, 4, PAWN, ChessSide::Black, Position::new(4, 3));

        let mut timeline = GameTimeline::new(state).unwrap();
        let mut white_turn = timeline.begin_turn(WHITE_PLAYER).unwrap();
        let double = rules
            .legal_moves(&white_turn.working, white_pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(3, 3))
            .unwrap();
        rules.execute_move(&mut white_turn, None, double, None).unwrap();
        timeline.commit_turn(white_turn).unwrap();

        let moves = rules
            .legal_moves_with_history(timeline.current(), Some(timeline.history()), black_pawn)
            .unwrap();
        assert!(moves.iter().any(|movement| {
            movement.to == Position::new(3, 2)
                && matches!(movement.kind, ChessMoveKind::EnPassant { victim } if victim == white_pawn)
        }));
    }

    #[test]
    fn en_passant_expires_after_the_immediate_reply() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let white_king = add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let white_pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(4, 4));
        let black_king = add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let black_pawn = add_piece(&mut state, 4, PAWN, ChessSide::Black, Position::new(3, 6));
        state.set_active_players(vec![BLACK_PLAYER]).unwrap();

        let mut timeline = GameTimeline::new(state).unwrap();
        let mut black_turn = timeline.begin_turn(BLACK_PLAYER).unwrap();
        let double = rules
            .legal_moves(&black_turn.working, black_pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(3, 4))
            .unwrap();
        rules.execute_move(&mut black_turn, None, double, None).unwrap();
        timeline.commit_turn(black_turn).unwrap();

        let mut white_turn = timeline.begin_turn(WHITE_PLAYER).unwrap();
        let king_move = rules
            .legal_moves_with_history(&white_turn.working, Some(timeline.history()), white_king)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(5, 0))
            .unwrap();
        rules
            .execute_move(&mut white_turn, Some(timeline.history()), king_move, None)
            .unwrap();
        timeline.commit_turn(white_turn).unwrap();

        let mut black_turn = timeline.begin_turn(BLACK_PLAYER).unwrap();
        let king_move = rules
            .legal_moves_with_history(&black_turn.working, Some(timeline.history()), black_king)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(5, 7))
            .unwrap();
        rules
            .execute_move(&mut black_turn, Some(timeline.history()), king_move, None)
            .unwrap();
        timeline.commit_turn(black_turn).unwrap();

        let legal = rules
            .legal_moves_with_history(timeline.current(), Some(timeline.history()), white_pawn)
            .unwrap();
        assert!(!legal.iter().any(|movement| matches!(movement.kind, ChessMoveKind::EnPassant { .. })));
    }

    #[test]
    fn en_passant_is_rejected_when_it_exposes_the_king() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(7, 4));
        let white_pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(6, 4));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(7, 7));
        add_piece(&mut state, 4, ROOK, ChessSide::Black, Position::new(0, 4));
        let black_pawn = add_piece(&mut state, 5, PAWN, ChessSide::Black, Position::new(5, 6));
        state.set_active_players(vec![BLACK_PLAYER]).unwrap();

        let mut timeline = GameTimeline::new(state).unwrap();
        let mut black_turn = timeline.begin_turn(BLACK_PLAYER).unwrap();
        let double = rules
            .legal_moves(&black_turn.working, black_pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(5, 4))
            .unwrap();
        rules.execute_move(&mut black_turn, None, double, None).unwrap();
        timeline.commit_turn(black_turn).unwrap();

        let legal = rules
            .legal_moves_with_history(timeline.current(), Some(timeline.history()), white_pawn)
            .unwrap();
        assert!(!legal.iter().any(|movement| movement.to == Position::new(5, 5)));
    }

    #[test]
    fn castling_is_derived_from_king_and_rook_entity_state() {
        use monumla_core::TurnSession;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let king = add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let queen_rook = add_piece(&mut state, 2, ROOK, ChessSide::White, Position::new(0, 0));
        let king_rook = add_piece(&mut state, 3, ROOK, ChessSide::White, Position::new(7, 0));
        add_piece(&mut state, 4, KING, ChessSide::Black, Position::new(4, 7));

        let legal = rules.legal_moves(&state, king).unwrap();
        assert!(legal.iter().any(|movement| {
            movement.to == Position::new(6, 0)
                && matches!(movement.kind, ChessMoveKind::Castle { rook, .. } if rook == king_rook)
        }));
        assert!(legal.iter().any(|movement| {
            movement.to == Position::new(2, 0)
                && matches!(movement.kind, ChessMoveKind::Castle { rook, .. } if rook == queen_rook)
        }));

        let castle = legal
            .into_iter()
            .find(|movement| movement.to == Position::new(6, 0))
            .unwrap();
        let mut turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        rules.execute_move(&mut turn, None, castle, None).unwrap();
        assert_eq!(turn.working.entity(king).unwrap().position, Position::new(6, 0));
        assert_eq!(turn.working.entity(king_rook).unwrap().position, Position::new(5, 0));
        assert!(crate::piece::has_moved(turn.working.entity(king).unwrap()));
        assert!(crate::piece::has_moved(turn.working.entity(king_rook).unwrap()));
    }

    #[test]
    fn castling_rejects_attacked_transit_and_previously_moved_pieces() {
        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        let king = add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let rook = add_piece(&mut state, 2, ROOK, ChessSide::White, Position::new(7, 0));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 4, ROOK, ChessSide::Black, Position::new(5, 7));
        assert!(!rules
            .legal_moves(&state, king)
            .unwrap()
            .iter()
            .any(|movement| movement.to == Position::new(6, 0)));

        state.remove_entity(EntityId::new(4)).unwrap();
        state.move_entity(rook, Position::new(7, 1)).unwrap();
        state.move_entity(rook, Position::new(7, 0)).unwrap();
        crate::piece::set_has_moved(state.entity_mut(rook).unwrap(), true);
        assert!(!rules
            .legal_moves(&state, king)
            .unwrap()
            .iter()
            .any(|movement| movement.to == Position::new(6, 0)));

        crate::piece::set_has_moved(state.entity_mut(rook).unwrap(), false);
        state.move_entity(king, Position::new(4, 1)).unwrap();
        state.move_entity(king, Position::new(4, 0)).unwrap();
        crate::piece::set_has_moved(state.entity_mut(king).unwrap(), true);
        assert!(!rules
            .legal_moves(&state, king)
            .unwrap()
            .iter()
            .any(|movement| movement.to == Position::new(6, 0)));
    }

    #[test]
    fn promotion_waits_for_an_explicit_interaction_choice_and_supports_underpromotion() {
        use monumla_core::{ChoiceKind, InteractionDriver, TurnSession};

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(0, 6));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));

        let turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let mut driver = InteractionDriver::new(ChessInteractionRules::new(&rules), turn).unwrap();
        let pawn_choice = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectEntity { entity } if entity == pawn))
            .unwrap()
            .id;
        driver.choose(pawn_choice).unwrap();
        let destination = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(0, 7)))
            .unwrap()
            .id;
        driver.choose(destination).unwrap();

        let option_keys = driver
            .interaction()
            .choices
            .iter()
            .filter_map(|choice| match &choice.kind {
                ChoiceKind::SelectOption { key } => Some(key.as_str()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(option_keys, BTreeSet::from(["bishop", "knight", "queen", "rook"]));
        assert_eq!(driver.turn().steps.len(), 0);

        let knight = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(&choice.kind, ChoiceKind::SelectOption { key } if key == "knight"))
            .unwrap()
            .id;
        driver.choose(knight).unwrap();
        assert!(driver.is_finished());
        assert_eq!(driver.turn().steps.len(), 1);
        assert_eq!(driver.turn().working.entity(pawn).unwrap().entity_type, KNIGHT);
    }

    #[test]
    fn game_rule_can_filter_piece_owned_promotion_choices_without_redefining_pawn() {
        use monumla_core::{ChoiceKind, InteractionDriver, TurnSession};

        let mut rules = ChessRules::standard();
        rules.register_game_rule(KnightOnlyPromotionRule);
        assert_eq!(rules.game_rule_count(), 1);

        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(0, 6));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));

        let turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let mut driver = InteractionDriver::new(ChessInteractionRules::new(&rules), turn).unwrap();
        let pawn_choice = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectEntity { entity } if entity == pawn))
            .unwrap()
            .id;
        driver.choose(pawn_choice).unwrap();
        let destination = driver
            .interaction()
            .choices
            .iter()
            .find(|choice| matches!(choice.kind, ChoiceKind::SelectPosition { position } if position == Position::new(0, 7)))
            .unwrap()
            .id;
        driver.choose(destination).unwrap();

        let options = driver
            .interaction()
            .choices
            .iter()
            .filter_map(|choice| match &choice.kind {
                ChoiceKind::SelectOption { key } => Some(key.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(options, vec!["knight"]);
    }

    #[test]
    fn direct_execution_cannot_bypass_game_rule_filtered_piece_input() {
        use monumla_core::TurnSession;

        let mut rules = ChessRules::standard();
        rules.register_game_rule(KnightOnlyPromotionRule);

        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(0, 6));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let movement = rules
            .legal_moves(&state, pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(0, 7))
            .unwrap();

        let queen = pieces::pawn::Pawn::promotion_input(QUEEN).unwrap();
        let mut rejected = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        assert_eq!(
            rules.execute_move(&mut rejected, None, movement, Some(&queen)),
            Err(ChessError::MoveInputRejected(pawn))
        );
        assert!(rejected.steps.is_empty());
        assert_eq!(rejected.working.entity(pawn).unwrap().entity_type, PAWN);

        let knight = pieces::pawn::Pawn::promotion_input(KNIGHT).unwrap();
        let mut accepted = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        rules
            .execute_move(&mut accepted, None, movement, Some(&knight))
            .unwrap();
        assert_eq!(accepted.working.entity(pawn).unwrap().entity_type, KNIGHT);
    }

    #[test]
    fn chess_execution_runs_registered_game_rule_reactions_in_same_transaction() {
        use monumla_core::TurnSession;

        let mut rules = ChessRules::standard();
        rules.register_game_rule(MarkMovedEntityRule);
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let knight = add_piece(&mut state, 2, KNIGHT, ChessSide::White, Position::new(1, 0));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        // K+N vs K is an automatic dead position, so keep unrelated mating material
        // on the board. This fixture must exercise the game-rule reaction path,
        // not the terminal-outcome guard.
        add_piece(&mut state, 4, PAWN, ChessSide::Black, Position::new(0, 6));
        let movement = rules
            .legal_moves(&state, knight)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(2, 2))
            .unwrap();
        let mut turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();

        rules.execute_move(&mut turn, None, movement, None).unwrap();

        assert_eq!(
            turn.working
                .entity(knight)
                .unwrap()
                .state
                .get("game_rule_reacted")
                .and_then(monumla_core::StateValue::as_bool),
            Some(true)
        );
        assert!(state.entity(knight).unwrap().state.is_empty());
    }

    #[test]
    fn capture_promotion_removes_the_target_and_changes_type() {
        use monumla_core::TurnSession;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let pawn = add_piece(&mut state, 2, PAWN, ChessSide::White, Position::new(1, 6));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let victim = add_piece(&mut state, 4, ROOK, ChessSide::Black, Position::new(2, 7));
        let movement = rules
            .legal_moves(&state, pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(2, 7))
            .unwrap();
        let mut turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let input = pieces::pawn::Pawn::promotion_input(BISHOP).unwrap();
        rules.execute_move(&mut turn, None, movement, Some(&input)).unwrap();
        assert!(!turn.working.entities.contains_key(&victim));
        assert_eq!(turn.working.entity(pawn).unwrap().entity_type, BISHOP);
    }

    fn commit_to(
        timeline: &mut monumla_core::GameTimeline,
        rules: &ChessRules,
        entity: EntityId,
        to: Position,
    ) {
        let actor = timeline.current().turn.active_players[0];
        let mut turn = timeline.begin_turn(actor).unwrap();
        let movement = rules
            .legal_moves_with_history(&turn.working, Some(timeline.history()), entity)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == to)
            .unwrap();
        rules
            .execute_move(&mut turn, Some(timeline.history()), movement, None)
            .unwrap();
        timeline.commit_turn(turn).unwrap();
    }

    #[test]
    fn status_detects_checkmate_and_stalemate() {
        use monumla_core::History;

        let rules = ChessRules::standard();
        let history = History::default();

        let mut mate = empty_chess_state().unwrap();
        add_piece(&mut mate, 1, KING, ChessSide::White, Position::new(5, 5));
        add_piece(&mut mate, 2, QUEEN, ChessSide::White, Position::new(6, 6));
        add_piece(&mut mate, 3, KING, ChessSide::Black, Position::new(7, 7));
        mate.set_active_players(vec![BLACK_PLAYER]).unwrap();
        let status = rules.status(&mate, &history).unwrap();
        assert!(status.in_check);
        assert_eq!(
            status.outcome,
            Some(ChessOutcome::Checkmate {
                winner: WHITE_PLAYER,
                loser: BLACK_PLAYER,
            })
        );
        let mut generic_rules = RuleRegistry::new();
        generic_rules.register_outcome(rules.outcome_rule());
        let generic = generic_rules
            .outcome(RuleContext::from_state(&mate, Some(&history)))
            .unwrap()
            .unwrap();
        assert_eq!(generic.key, "chess.checkmate");
        assert_eq!(generic.winners, vec![WHITE_PLAYER]);
        assert_eq!(generic.losers, vec![BLACK_PLAYER]);

        let mut stalemate = empty_chess_state().unwrap();
        add_piece(&mut stalemate, 1, KING, ChessSide::White, Position::new(5, 6));
        add_piece(&mut stalemate, 2, QUEEN, ChessSide::White, Position::new(6, 5));
        add_piece(&mut stalemate, 3, KING, ChessSide::Black, Position::new(7, 7));
        stalemate.set_active_players(vec![BLACK_PLAYER]).unwrap();
        let status = rules.status(&stalemate, &history).unwrap();
        assert!(!status.in_check);
        assert_eq!(status.outcome, Some(ChessOutcome::Stalemate));
    }

    #[test]
    fn resignation_and_draw_agreement_are_persistent_terminal_outcomes() {
        use monumla_core::History;

        let rules = ChessRules::standard();
        let history = History::default();
        let mut resignation = standard_chess_state().unwrap();
        assert_eq!(
            rules.resign(&mut resignation, &history, WHITE_PLAYER).unwrap(),
            ChessOutcome::Resignation {
                winner: BLACK_PLAYER,
                loser: WHITE_PLAYER,
            }
        );
        assert_eq!(
            rules.status(&resignation, &history).unwrap().outcome,
            Some(ChessOutcome::Resignation {
                winner: BLACK_PLAYER,
                loser: WHITE_PLAYER,
            })
        );
        assert_eq!(
            rules.agree_draw(&mut resignation, &history),
            Err(ChessError::GameFinished)
        );

        let initial = standard_chess_state().unwrap();
        let mut too_early = initial.clone();
        assert_eq!(
            rules.agree_draw(&mut too_early, &history),
            Err(ChessError::DrawAgreementUnavailable)
        );

        let white_pawn = initial.entity_at(Position::new(4, 1)).unwrap().unwrap().id;
        let black_pawn = initial.entity_at(Position::new(4, 6)).unwrap().unwrap().id;
        let mut timeline = monumla_core::GameTimeline::new(initial).unwrap();
        commit_to(&mut timeline, &rules, white_pawn, Position::new(4, 3));
        let mut after_white = timeline.current().clone();
        assert_eq!(
            rules.agree_draw(&mut after_white, timeline.history()),
            Err(ChessError::DrawAgreementUnavailable)
        );
        commit_to(&mut timeline, &rules, black_pawn, Position::new(4, 4));
        let mut agreement = timeline.current().clone();
        assert_eq!(
            rules.agree_draw(&mut agreement, timeline.history()).unwrap(),
            ChessOutcome::DrawAgreement
        );
        assert_eq!(
            rules.status(&agreement, timeline.history()).unwrap().outcome,
            Some(ChessOutcome::DrawAgreement)
        );
    }

    #[test]
    fn repetition_key_tracks_side_castling_and_effective_en_passant() {
        use monumla_core::{GameTimeline, History};

        let rules = ChessRules::standard();
        let mut with_rights = empty_chess_state().unwrap();
        add_piece(&mut with_rights, 1, KING, ChessSide::White, Position::new(4, 0));
        let rook = add_piece(&mut with_rights, 2, ROOK, ChessSide::White, Position::new(7, 0));
        add_piece(&mut with_rights, 3, KING, ChessSide::Black, Position::new(4, 7));
        let no_history = History::default();
        let key_with_rights = rules.position_key(&with_rights, &no_history).unwrap();
        let mut without_rights = with_rights.clone();
        crate::piece::set_has_moved(without_rights.entity_mut(rook).unwrap(), true);
        let key_without_rights = rules.position_key(&without_rights, &no_history).unwrap();
        assert_ne!(key_with_rights, key_without_rights);

        let mut black_to_move = with_rights.clone();
        black_to_move.set_active_players(vec![BLACK_PLAYER]).unwrap();
        let black_key = rules.position_key(&black_to_move, &no_history).unwrap();
        assert_ne!(key_with_rights, black_key);

        let mut ep_state = empty_chess_state().unwrap();
        add_piece(&mut ep_state, 10, KING, ChessSide::White, Position::new(4, 0));
        add_piece(&mut ep_state, 11, PAWN, ChessSide::White, Position::new(4, 4));
        add_piece(&mut ep_state, 12, KING, ChessSide::Black, Position::new(4, 7));
        let black_pawn = add_piece(&mut ep_state, 13, PAWN, ChessSide::Black, Position::new(3, 6));
        ep_state.set_active_players(vec![BLACK_PLAYER]).unwrap();
        let mut timeline = GameTimeline::new(ep_state).unwrap();
        commit_to(&mut timeline, &rules, black_pawn, Position::new(3, 4));
        let with_ep = rules.position_key(timeline.current(), timeline.history()).unwrap();
        let without_ep = rules.position_key(timeline.current(), &History::default()).unwrap();
        assert_ne!(with_ep, without_ep);
    }

    #[test]
    fn threefold_is_claimable_and_fivefold_is_automatic() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let white_knight = add_piece(&mut state, 2, KNIGHT, ChessSide::White, Position::new(1, 0));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let black_knight = add_piece(&mut state, 4, KNIGHT, ChessSide::Black, Position::new(1, 7));
        let mut timeline = GameTimeline::new(state).unwrap();

        for _ in 0..2 {
            commit_to(&mut timeline, &rules, white_knight, Position::new(2, 2));
            commit_to(&mut timeline, &rules, black_knight, Position::new(2, 5));
            commit_to(&mut timeline, &rules, white_knight, Position::new(1, 0));
            commit_to(&mut timeline, &rules, black_knight, Position::new(1, 7));
        }
        let status = rules.status(timeline.current(), timeline.history()).unwrap();
        assert_eq!(status.repetition_count, 3);
        assert!(status.can_claim_threefold_repetition);
        assert_eq!(status.outcome, None);

        let mut claimed = timeline.current().clone();
        assert_eq!(
            rules
                .claim_draw(
                    &mut claimed,
                    timeline.history(),
                    ChessDrawClaim::ThreefoldRepetition,
                )
                .unwrap(),
            ChessOutcome::ThreefoldRepetition
        );

        for _ in 0..2 {
            commit_to(&mut timeline, &rules, white_knight, Position::new(2, 2));
            commit_to(&mut timeline, &rules, black_knight, Position::new(2, 5));
            commit_to(&mut timeline, &rules, white_knight, Position::new(1, 0));
            commit_to(&mut timeline, &rules, black_knight, Position::new(1, 7));
        }
        let status = rules.status(timeline.current(), timeline.history()).unwrap();
        assert_eq!(status.repetition_count, 5);
        assert_eq!(status.outcome, Some(ChessOutcome::FivefoldRepetition));
    }

    #[test]
    fn terminal_metadata_does_not_create_an_extra_repetition_occurrence() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let state = standard_chess_state().unwrap();
        let white_knight = state.entity_at(Position::new(6, 0)).unwrap().unwrap().id;
        let black_knight = state.entity_at(Position::new(6, 7)).unwrap().unwrap().id;
        let mut timeline = GameTimeline::new(state).unwrap();

        commit_to(&mut timeline, &rules, white_knight, Position::new(5, 2));
        commit_to(&mut timeline, &rules, black_knight, Position::new(5, 5));
        commit_to(&mut timeline, &rules, white_knight, Position::new(6, 0));
        commit_to(&mut timeline, &rules, black_knight, Position::new(6, 7));

        let before = rules
            .repetition_count(timeline.current(), timeline.history())
            .unwrap();
        assert_eq!(before, 2);

        let mut terminal = timeline.current().clone();
        rules
            .record_result(&mut terminal, Some(WHITE_PLAYER))
            .unwrap();
        assert_eq!(
            rules.repetition_count(&terminal, timeline.history()).unwrap(),
            before
        );
    }

    #[test]
    fn terminal_only_history_does_not_duplicate_the_initial_repetition() {
        use monumla_core::{GameTimeline, RecordedAction};

        let rules = ChessRules::standard();
        let mut timeline = GameTimeline::new(standard_chess_state().unwrap()).unwrap();
        let actor = timeline.current().turn.active_players[0];
        let mut turn = timeline.begin_turn(actor).unwrap();
        turn.apply_transaction(
            RecordedAction::new("chess_recorded_result"),
            |transaction| -> Result<(), ChessError> {
                rules.record_result(transaction.raw_state_mut(), Some(BLACK_PLAYER))?;
                Ok(())
            },
        )
        .unwrap();
        timeline.commit_turn(turn).unwrap();

        assert_eq!(
            rules
                .repetition_count(timeline.current(), timeline.history())
                .unwrap(),
            1
        );
    }

    #[test]
    fn threefold_can_be_claimed_by_declaring_the_repeating_move() {
        use monumla_core::GameTimeline;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let white_knight = add_piece(&mut state, 2, KNIGHT, ChessSide::White, Position::new(1, 0));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        let black_knight = add_piece(&mut state, 4, KNIGHT, ChessSide::Black, Position::new(1, 7));
        let mut timeline = GameTimeline::new(state).unwrap();

        commit_to(&mut timeline, &rules, white_knight, Position::new(2, 2));
        commit_to(&mut timeline, &rules, black_knight, Position::new(2, 5));
        commit_to(&mut timeline, &rules, white_knight, Position::new(1, 0));
        commit_to(&mut timeline, &rules, black_knight, Position::new(1, 7));
        commit_to(&mut timeline, &rules, white_knight, Position::new(2, 2));
        commit_to(&mut timeline, &rules, black_knight, Position::new(2, 5));
        commit_to(&mut timeline, &rules, white_knight, Position::new(1, 0));

        let status = rules.status(timeline.current(), timeline.history()).unwrap();
        assert_eq!(status.repetition_count, 2);
        assert!(!status.can_claim_threefold_repetition);

        let intended = rules
            .legal_moves_with_history(timeline.current(), Some(timeline.history()), black_knight)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(1, 7))
            .unwrap();
        assert!(rules
            .claimable_draw_moves(
                timeline.current(),
                timeline.history(),
                ChessDrawClaim::ThreefoldRepetition,
            )
            .unwrap()
            .iter()
            .any(|(movement, promotion)| *movement == intended && promotion.is_none()));

        let mut claimed = timeline.current().clone();
        assert_eq!(
            rules
                .claim_draw_after_move(
                    &mut claimed,
                    timeline.history(),
                    ChessDrawClaim::ThreefoldRepetition,
                    intended,
                    None,
                )
                .unwrap(),
            ChessOutcome::ThreefoldRepetition
        );
        assert_eq!(claimed.entities, timeline.current().entities);
    }

    #[test]
    fn fifty_move_rule_can_be_claimed_by_declaring_the_hundredth_halfmove() {
        use monumla_core::History;

        let rules = ChessRules::standard();
        let history = History::default();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let knight = add_piece(&mut state, 2, KNIGHT, ChessSide::White, Position::new(1, 0));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 4, ROOK, ChessSide::Black, Position::new(7, 7));
        rules.set_halfmove_clock(&mut state, 99);

        let movement = rules
            .legal_moves_with_history(&state, Some(&history), knight)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(2, 2))
            .unwrap();
        assert!(!rules.status(&state, &history).unwrap().can_claim_fifty_move_rule);
        assert!(rules
            .claimable_draw_moves(&state, &history, ChessDrawClaim::FiftyMoveRule)
            .unwrap()
            .iter()
            .any(|(candidate, promotion)| *candidate == movement && promotion.is_none()));

        let before = state.clone();
        assert_eq!(
            rules
                .claim_draw_after_move(
                    &mut state,
                    &history,
                    ChessDrawClaim::FiftyMoveRule,
                    movement,
                    None,
                )
                .unwrap(),
            ChessOutcome::FiftyMoveRule
        );
        assert_eq!(state.entities, before.entities);
        assert_eq!(rules.halfmove_clock(&state), 99);
    }

    #[test]
    fn halfmove_clock_resets_and_fifty_seventy_five_rules_are_exposed() {
        use monumla_core::{History, TurnSession};

        let rules = ChessRules::standard();
        let history = History::default();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let white_knight = add_piece(&mut state, 2, KNIGHT, ChessSide::White, Position::new(1, 0));
        let white_pawn = add_piece(&mut state, 3, PAWN, ChessSide::White, Position::new(0, 1));
        add_piece(&mut state, 4, KING, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 5, PAWN, ChessSide::Black, Position::new(0, 6));

        let mut turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let movement = rules
            .legal_moves(&turn.working, white_knight)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(2, 2))
            .unwrap();
        rules.execute_move(&mut turn, None, movement, None).unwrap();
        assert_eq!(rules.halfmove_clock(&turn.working), 1);

        let mut pawn_state = state.clone();
        rules.set_halfmove_clock(&mut pawn_state, 99);
        let mut turn = TurnSession::new(&pawn_state, WHITE_PLAYER).unwrap();
        let movement = rules
            .legal_moves(&turn.working, white_pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(0, 2))
            .unwrap();
        rules.execute_move(&mut turn, None, movement, None).unwrap();
        assert_eq!(rules.halfmove_clock(&turn.working), 0);

        let mut threshold = state.clone();
        rules.set_halfmove_clock(&mut threshold, 100);
        let status = rules.status(&threshold, &history).unwrap();
        assert!(status.can_claim_fifty_move_rule);
        assert_eq!(status.outcome, None);
        let mut claimed = threshold.clone();
        assert_eq!(
            rules
                .claim_draw(&mut claimed, &history, ChessDrawClaim::FiftyMoveRule)
                .unwrap(),
            ChessOutcome::FiftyMoveRule
        );
        rules.set_halfmove_clock(&mut threshold, 150);
        assert_eq!(
            rules.status(&threshold, &history).unwrap().outcome,
            Some(ChessOutcome::SeventyFiveMoveRule)
        );
    }

    #[test]
    fn capture_resets_halfmove_clock() {
        use monumla_core::TurnSession;

        let rules = ChessRules::standard();
        let mut state = empty_chess_state().unwrap();
        add_piece(&mut state, 1, KING, ChessSide::White, Position::new(4, 0));
        let rook = add_piece(&mut state, 2, ROOK, ChessSide::White, Position::new(0, 0));
        add_piece(&mut state, 3, KING, ChessSide::Black, Position::new(4, 7));
        add_piece(&mut state, 4, KNIGHT, ChessSide::Black, Position::new(0, 7));
        rules.set_halfmove_clock(&mut state, 99);
        let mut turn = TurnSession::new(&state, WHITE_PLAYER).unwrap();
        let capture = rules
            .legal_moves(&turn.working, rook)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(0, 7))
            .unwrap();
        rules.execute_move(&mut turn, None, capture, None).unwrap();
        assert_eq!(rules.halfmove_clock(&turn.working), 0);
    }

    #[test]
    fn dead_positions_are_automatic_and_checkmate_precedes_75_move_rule() {
        use monumla_core::History;

        let rules = ChessRules::standard();
        let history = History::default();
        let mut dead = empty_chess_state().unwrap();
        add_piece(&mut dead, 1, KING, ChessSide::White, Position::new(0, 0));
        add_piece(&mut dead, 2, KING, ChessSide::Black, Position::new(7, 7));
        assert_eq!(
            rules.status(&dead, &history).unwrap().outcome,
            Some(ChessOutcome::DeadPosition)
        );

        let mut mate = empty_chess_state().unwrap();
        add_piece(&mut mate, 1, KING, ChessSide::White, Position::new(5, 5));
        add_piece(&mut mate, 2, QUEEN, ChessSide::White, Position::new(6, 6));
        add_piece(&mut mate, 3, KING, ChessSide::Black, Position::new(7, 7));
        mate.set_active_players(vec![BLACK_PLAYER]).unwrap();
        rules.set_halfmove_clock(&mut mate, 150);
        assert!(matches!(
            rules.status(&mate, &history).unwrap().outcome,
            Some(ChessOutcome::Checkmate { .. })
        ));
    }

}
