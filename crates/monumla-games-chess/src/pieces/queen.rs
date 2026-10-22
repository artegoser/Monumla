use crate::{
    piece::{ray_attacks, ray_moves, standard_presentation},
    ChessError, ChessPieceContext, ChessPieceKind, ChessPieceRule, PseudoMove, QUEEN,
};
use monumla_core::{
    EntityPresentation, EntityRule, EntityRuleContext, EntityTypeId, Position, RuleError,
};

const DIRECTIONS: [(i16, i16); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

pub struct Queen;

impl EntityRule for Queen {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        standard_presentation(context, ChessPieceKind::Queen)
    }
}

impl ChessPieceRule for Queen {
    fn entity_type(&self) -> EntityTypeId {
        QUEEN
    }

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        ray_moves(context, &DIRECTIONS)
    }

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        ray_attacks(context, &DIRECTIONS)
    }
}
