use crate::{
    piece::{has_moved, leaper_attacks, leaper_moves, standard_presentation},
    ChessError, ChessPieceContext, ChessPieceKind, ChessPieceRule, ChessRules, PseudoMove, KING, ROOK,
};
use monumla_core::{
    EntityPresentation, EntityRule, EntityRuleContext, EntityTypeId, Position, RuleError,
};

const OFFSETS: [(i16, i16); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

pub struct King;

impl EntityRule for King {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        standard_presentation(context, ChessPieceKind::King)
    }
}

impl ChessPieceRule for King {
    fn entity_type(&self) -> EntityTypeId {
        KING
    }

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        leaper_moves(context, &OFFSETS)
    }

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        Ok(leaper_attacks(context, &OFFSETS))
    }
}


pub(crate) fn castling_moves(
    rules: &ChessRules,
    context: ChessPieceContext<'_>,
) -> Result<Vec<PseudoMove>, ChessError> {
    let king = context.entity();
    let side = context.side()?;
    let home_rank = side.home_rank();
    if has_moved(king) || king.position != Position::new(4, home_rank) {
        return Ok(Vec::new());
    }
    if rules.in_check(context.state(), side)? {
        return Ok(Vec::new());
    }

    let mut moves = Vec::new();
    for (rook_x, destination_x, rook_to_x, between, transit) in [
        (7_u16, 6_u16, 5_u16, &[5_u16, 6_u16][..], 5_u16),
        (0_u16, 2_u16, 3_u16, &[1_u16, 2_u16, 3_u16][..], 3_u16),
    ] {
        let rook_position = Position::new(rook_x, home_rank);
        let Some(rook) = context.entity_at(rook_position)? else {
            continue;
        };
        if rook.entity_type != ROOK || rook.owner != king.owner || has_moved(rook) {
            continue;
        }
        let mut clear = true;
        for x in between {
            if !context.is_empty(Position::new(*x, home_rank))? {
                clear = false;
                break;
            }
        }
        if !clear {
            continue;
        }
        let transit_square = Position::new(transit, home_rank);
        let destination = Position::new(destination_x, home_rank);
        if rules.is_square_attacked(context.state(), side.opponent(), transit_square)?
            || rules.is_square_attacked(context.state(), side.opponent(), destination)?
        {
            continue;
        }
        moves.push(PseudoMove::castle(
            king.id,
            king.position,
            destination,
            rook.id,
            Position::new(rook_to_x, home_rank),
        ));
    }
    Ok(moves)
}
