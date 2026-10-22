use crate::{
    piece::{ray_attacks, ray_moves, standard_presentation},
    ChessError, ChessPieceContext, ChessPieceKind, ChessPieceRule, PseudoMove, ROOK,
};
use monumla_core::{
    EntityPresentation, EntityRule, EntityRuleContext, EntityTypeId, Position, RuleError,
};

const DIRECTIONS: [(i16, i16); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

pub struct Rook;

impl EntityRule for Rook {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        standard_presentation(context, ChessPieceKind::Rook)
    }
}

impl ChessPieceRule for Rook {
    fn entity_type(&self) -> EntityTypeId {
        ROOK
    }

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        ray_moves(context, &DIRECTIONS)
    }

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        ray_attacks(context, &DIRECTIONS)
    }
}
