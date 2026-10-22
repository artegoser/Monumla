use crate::{
    piece::{offset, set_has_moved, standard_presentation},
    ChessError, ChessPieceContext, ChessPieceKind, ChessPieceRule, ChessSide, PseudoMove, BISHOP,
    KNIGHT, PAWN, QUEEN, ROOK,
};
use monumla_core::{
    ChoiceInput, ChoiceKind, ChoiceSpec, EntityId, EntityPresentation, EntityRule,
    EntityRuleContext, EntityTypeId, GameState, Position, RuleError, StateChange, StateValue,
    TurnRecord,
};

pub struct Pawn;

const PROMOTION_TARGETS: [(EntityTypeId, &str, &str); 4] = [
    (QUEEN, "queen", "Queen"),
    (ROOK, "rook", "Rook"),
    (BISHOP, "bishop", "Bishop"),
    (KNIGHT, "knight", "Knight"),
];

impl Pawn {
    #[cfg(test)]
    pub(crate) fn promotion_input(entity_type: EntityTypeId) -> Result<ChoiceInput, ChessError> {
        let (_, key, _) = PROMOTION_TARGETS
            .iter()
            .find(|(candidate, _, _)| *candidate == entity_type)
            .copied()
            .ok_or(ChessError::InvalidPromotion(entity_type))?;
        let mut data = monumla_core::StateMap::new();
        data.insert("entity_type", u64::from(entity_type.get()));
        Ok(ChoiceInput {
            kind: ChoiceKind::SelectOption { key: key.into() },
            data,
        })
    }

    pub(crate) const fn is_promotion_type(entity_type: EntityTypeId) -> bool {
        entity_type.get() == QUEEN.get()
            || entity_type.get() == ROOK.get()
            || entity_type.get() == BISHOP.get()
            || entity_type.get() == KNIGHT.get()
    }

    fn promotion_type_for_key(key: &str) -> Option<EntityTypeId> {
        PROMOTION_TARGETS
            .iter()
            .find(|(_, candidate_key, _)| *candidate_key == key)
            .map(|(entity_type, _, _)| *entity_type)
    }

    fn promotion_type(input: Option<&ChoiceInput>) -> Option<EntityTypeId> {
        input?
            .data
            .get("entity_type")
            .and_then(StateValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .map(EntityTypeId::new)
    }

    fn is_promotion_move(
        context: ChessPieceContext<'_>,
        movement: &PseudoMove,
    ) -> Result<bool, ChessError> {
        Ok(movement.to.y == context.side()?.opponent().home_rank())
    }
}

impl EntityRule for Pawn {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        standard_presentation(context, ChessPieceKind::Pawn)
    }
}

impl ChessPieceRule for Pawn {
    fn entity_type(&self) -> EntityTypeId {
        PAWN
    }

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError> {
        ensure_type(context, PAWN)?;
        let side = context.side()?;
        let from = context.entity().position;
        let mut moves = Vec::new();

        if let Some(one) = offset(context.state(), from, 0, side.forward()) {
            if context.is_empty(one)? {
                moves.push(context.pseudo_move(one)?);

                let on_start = from.y == side.pawn_start_rank();
                if on_start {
                    if let Some(two) = offset(context.state(), from, 0, side.forward() * 2) {
                        if context.is_empty(two)? {
                            moves.push(context.pseudo_move(two)?);
                        }
                    }
                }
            }
        }

        for dx in [-1_i16, 1_i16] {
            let Some(target) = offset(context.state(), from, dx, side.forward()) else {
                continue;
            };
            if context.capture_at(target)?.is_some() {
                moves.push(context.pseudo_move(target)?);
            }
        }

        if let Some(previous) = context.history().and_then(|history| history.previous_turn()) {
            moves.extend(en_passant_moves_from_previous(context, previous)?);
        }

        Ok(moves)
    }

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError> {
        ensure_type(context, PAWN)?;
        let side = context.side()?;
        Ok([-1_i16, 1_i16]
            .into_iter()
            .filter_map(|dx| {
                offset(
                    context.state(),
                    context.entity().position,
                    dx,
                    side.forward(),
                )
            })
            .collect())
    }

    fn move_choices(
        &self,
        context: ChessPieceContext<'_>,
        movement: &PseudoMove,
    ) -> Result<Vec<ChoiceSpec>, ChessError> {
        ensure_type(context, PAWN)?;
        if !Self::is_promotion_move(context, movement)? {
            return Ok(Vec::new());
        }

        let side_name = match context.side()? {
            ChessSide::White => "white",
            ChessSide::Black => "black",
        };
        Ok(PROMOTION_TARGETS
            .into_iter()
            .map(|(entity_type, key, label)| {
                let mut choice = ChoiceSpec::option(key)
                    .with_label(label)
                    .with_asset_key(format!("chess/{side_name}/{key}"));
                choice
                    .data
                    .insert("entity_type", u64::from(entity_type.get()));
                choice
            })
            .collect())
    }

    fn validate_move_input(
        &self,
        context: ChessPieceContext<'_>,
        movement: &PseudoMove,
        input: Option<&ChoiceInput>,
    ) -> Result<(), ChessError> {
        ensure_type(context, PAWN)?;
        if Self::is_promotion_move(context, movement)? {
            let input = input.ok_or(ChessError::PromotionRequired(movement.actor))?;
            let ChoiceKind::SelectOption { key } = &input.kind else {
                return Err(ChessError::UnexpectedMoveInput(movement.actor));
            };
            let expected = Self::promotion_type_for_key(key)
                .ok_or(ChessError::UnexpectedMoveInput(movement.actor))?;
            let promotion = Self::promotion_type(Some(input))
                .ok_or(ChessError::UnexpectedMoveInput(movement.actor))?;
            if !Self::is_promotion_type(promotion) {
                return Err(ChessError::InvalidPromotion(promotion));
            }
            if promotion != expected {
                return Err(ChessError::UnexpectedMoveInput(movement.actor));
            }
            Ok(())
        } else if input.is_some() {
            Err(ChessError::UnexpectedPromotion(movement.actor))
        } else {
            Ok(())
        }
    }

    fn apply_move_input(
        &self,
        state: &mut GameState,
        movement: &PseudoMove,
        input: Option<&ChoiceInput>,
    ) -> Result<(), ChessError> {
        let Some(promote_to) = Self::promotion_type(input) else {
            return Ok(());
        };
        let promoted = state.entity_mut(movement.actor)?;
        promoted.entity_type = promote_to;
        set_has_moved(promoted, true);
        Ok(())
    }
}

fn ensure_type(context: ChessPieceContext<'_>, expected: EntityTypeId) -> Result<(), ChessError> {
    let actual = context.entity().entity_type;
    if actual == expected {
        Ok(())
    } else {
        Err(ChessError::WrongPieceType {
            entity: context.entity().id,
            expected,
            actual,
        })
    }
}


pub(crate) fn turn_records_entity_move(
    previous: &TurnRecord,
    entity: EntityId,
    from: Position,
    to: Position,
) -> bool {
    previous.steps.iter().any(|step| {
        let action_actor = step
            .action
            .data
            .get("actor")
            .and_then(StateValue::as_u64);
        step.action.kind == "chess_move"
            && action_actor == Some(u64::from(entity.get()))
            && step.delta.changes.iter().any(|change| {
                matches!(
                    change,
                    StateChange::EntityMoved { entity: moved, from: actual_from, to: actual_to }
                        if *moved == entity && *actual_from == from && *actual_to == to
                )
            })
    })
}

pub(crate) fn en_passant_moves_from_previous(
    context: ChessPieceContext<'_>,
    previous: &TurnRecord,
) -> Result<Vec<PseudoMove>, ChessError> {
    let side = context.side()?;
    let from = context.entity().position;
    let mut moves = Vec::new();

    for dx in [-1_i16, 1_i16] {
        let Some(adjacent_position) = offset(context.state(), from, dx, 0) else {
            continue;
        };
        let Some(adjacent) = context.entity_at(adjacent_position)? else {
            continue;
        };
        if adjacent.entity_type != PAWN || adjacent.owner == context.entity().owner {
            continue;
        }
        let Some(enemy_side) = crate::ChessSide::from_player(adjacent.owner) else {
            continue;
        };
        if previous.actor != adjacent.controller {
            continue;
        }

        let Some(before) = previous.before.entities.get(&adjacent.id) else {
            continue;
        };
        let Some(after) = previous.after.entities.get(&adjacent.id) else {
            continue;
        };
        if before.entity_type != PAWN
            || after.entity_type != PAWN
            || before.position.x != after.position.x
            || before.position.y != enemy_side.pawn_start_rank()
            || after.position != adjacent_position
        {
            continue;
        }
        let expected_y = i32::from(before.position.y) + i32::from(enemy_side.forward() * 2);
        if expected_y != i32::from(after.position.y) {
            continue;
        }

        if !turn_records_entity_move(
            previous,
            adjacent.id,
            before.position,
            after.position,
        ) {
            continue;
        }

        let Some(target) = offset(context.state(), from, dx, side.forward()) else {
            continue;
        };
        if !context.is_empty(target)? {
            continue;
        }
        moves.push(PseudoMove::en_passant(
            context.entity().id,
            from,
            target,
            adjacent.id,
        ));
    }

    Ok(moves)
}
