use crate::*;
use monumla_core::{
    EntityId, EntityPresentation, EntityRule, EntityRuleContext, EntityState, EntityTypeId,
    Position, RuleError,
};

#[test]
fn single_square_queries_match_attack_lists_on_every_origin_and_blocker_square() {
    let rules = ChessRules::standard();
    for kind in [PAWN, KNIGHT, BISHOP, ROOK, QUEEN, KING] {
        let rule = rules.piece_rule(kind).unwrap();
        for side in [ChessSide::White, ChessSide::Black] {
            for origin in 0..64_u16 {
                let from = Position::new(origin % 8, origin / 8);
                let mut base = empty_chess_state().unwrap();
                base.add_entity(EntityState::new(
                    EntityId::new(1),
                    kind,
                    side.player(),
                    from,
                ))
                .unwrap();
                for blocker in 0..=128_u16 {
                    let mut state = base.clone();
                    if blocker != 128 {
                        let point = Position::new(blocker % 8, blocker % 64 / 8);
                        if point == from {
                            continue;
                        }
                        let owner = if blocker < 64 { side } else { side.opponent() };
                        state
                            .add_entity(EntityState::new(
                                EntityId::new(2),
                                PAWN,
                                owner.player(),
                                point,
                            ))
                            .unwrap();
                    }
                    let context = ChessPieceContext::new(&state, EntityId::new(1)).unwrap();
                    let expected = rule.attacks(context).unwrap();
                    for target in state.board.positions().chain([
                        Position::new(8, 0),
                        Position::new(0, 8),
                        Position::new(u16::MAX, u16::MAX),
                    ]) {
                        assert_eq!(
                            rule.attacks_square(context, target).unwrap(),
                            expected.contains(&target),
                            "kind={kind:?} side={side:?} from={from:?} blocker={blocker} target={target:?}"
                        );
                    }
                }
            }
        }
    }
}

// Existing custom pieces need only supply their full attack list.
struct CustomPiece;
impl EntityRule for CustomPiece {
    fn presentation(&self, _: EntityRuleContext<'_>) -> Result<EntityPresentation, RuleError> {
        Ok(EntityPresentation::new("custom"))
    }
}
impl ChessPieceRule for CustomPiece {
    fn entity_type(&self) -> EntityTypeId {
        EntityTypeId::new(77)
    }
    fn pseudo_moves(&self, _: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        Ok(Vec::new())
    }
    fn attacks(&self, _: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        Ok(vec![Position::new(2, 2)])
    }
}

#[test]
fn custom_pieces_keep_the_default_single_square_query() {
    let mut state = empty_chess_state().unwrap();
    state
        .add_entity(EntityState::new(
            EntityId::new(1),
            CustomPiece.entity_type(),
            ChessSide::White.player(),
            Position::new(0, 0),
        ))
        .unwrap();
    let context = ChessPieceContext::new(&state, EntityId::new(1)).unwrap();
    assert!(CustomPiece
        .attacks_square(context, Position::new(2, 2))
        .unwrap());
    assert!(!CustomPiece
        .attacks_square(context, Position::new(1, 1))
        .unwrap());
}
