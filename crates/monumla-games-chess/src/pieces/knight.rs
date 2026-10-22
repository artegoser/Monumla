use crate::{
    piece::{leaper_attacks, leaper_moves, standard_presentation},
    ChessError, ChessPieceContext, ChessPieceKind, ChessPieceRule, PseudoMove, KNIGHT,
};
use monumla_core::{
    EntityPresentation, EntityRule, EntityRuleContext, EntityTypeId, Position, RuleError,
};

const OFFSETS: [(i16, i16); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

pub struct Knight;

impl EntityRule for Knight {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        standard_presentation(context, ChessPieceKind::Knight)
    }
}

impl ChessPieceRule for Knight {
    fn entity_type(&self) -> EntityTypeId {
        KNIGHT
    }

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        leaper_moves(context, &OFFSETS)
    }

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        Ok(leaper_attacks(context, &OFFSETS))
    }
}
