use crate::{
    piece::{ray_attacks, ray_attacks_square, ray_moves, standard_presentation},
    ChessError, ChessPieceContext, ChessPieceKind, ChessPieceRule, PseudoMove, BISHOP,
};
use monumla_core::{
    EntityPresentation, EntityRule, EntityRuleContext, EntityTypeId, Position, RuleError,
};

const DIRECTIONS: [(i16, i16); 4] = [(1, 1), (1, -1), (-1, -1), (-1, 1)];

pub struct Bishop;

impl EntityRule for Bishop {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        standard_presentation(context, ChessPieceKind::Bishop)
    }
}

impl ChessPieceRule for Bishop {
    fn entity_type(&self) -> EntityTypeId {
        BISHOP
    }

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        ray_moves(context, &DIRECTIONS)
    }

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        ray_attacks(context, &DIRECTIONS)
    }
    fn attacks_square(
        &self,
        context: ChessPieceContext<'_>,
        target: Position,
    ) -> Result<bool, ChessError> {
        ray_attacks_square(context, &DIRECTIONS, target)
    }
}
