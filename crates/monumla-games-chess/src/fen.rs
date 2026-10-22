use crate::{
    piece::set_has_moved, pieces::pawn::turn_records_entity_move, ChessError, ChessRules, ChessSide,
    BISHOP, KING, KNIGHT, PAWN, QUEEN, ROOK,
};
pub const STANDARD_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

use monumla_core::{EntityId, EntityState, EntityTypeId, GameState, GameTimeline, History, Position};

pub struct FenGame {
    pub timeline: GameTimeline,
    pub synthetic_history_len: usize,
}

impl ChessRules {
    pub fn from_fen(&self, fen: &str) -> Result<FenGame, ChessError> {
        let fields = fen.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 6 {
            return Err(ChessError::InvalidFen(
                "expected exactly six space-separated fields".into(),
            ));
        }

        let mut state = crate::empty_chess_state()?;
        parse_placement(&mut state, fields[0])?;

        let side_to_move = match fields[1] {
            "w" => ChessSide::White,
            "b" => ChessSide::Black,
            other => {
                return Err(ChessError::InvalidFen(format!(
                    "invalid active-color field {other:?}"
                )))
            }
        };
        state.set_active_players(vec![side_to_move.player()])?;

        apply_castling_metadata(&mut state, fields[2])?;

        let halfmove_clock = fields[4]
            .parse::<u16>()
            .map_err(|_| ChessError::InvalidFen("invalid halfmove clock".into()))?;
        let fullmove_number = fields[5]
            .parse::<u32>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| ChessError::InvalidFen("fullmove number must be positive".into()))?;
        self.set_halfmove_clock(&mut state, halfmove_clock);
        self.set_fullmove_number(&mut state, fullmove_number);

        self.king(&state, ChessSide::White)?;
        self.king(&state, ChessSide::Black)?;
        state.validate()?;
        self.validate_imported_position(&state, side_to_move)?;

        if fields[3] == "-" {
            return Ok(FenGame {
                timeline: GameTimeline::new(state)?,
                synthetic_history_len: 0,
            });
        }

        if halfmove_clock != 0 {
            return Err(ChessError::InvalidFen(
                "en-passant target requires a zero halfmove clock".into(),
            ));
        }
        let target = parse_square(fields[3])?;
        let expected_target_rank = match side_to_move {
            ChessSide::White => 5,
            ChessSide::Black => 2,
        };
        if target.y != expected_target_rank {
            return Err(ChessError::InvalidFen(
                "en-passant target is on the wrong rank for the active side".into(),
            ));
        }
        if state.entity_at(target)?.is_some() {
            return Err(ChessError::InvalidFen(
                "en-passant target square must be empty".into(),
            ));
        }

        let mover = side_to_move.opponent();
        let current_pawn_square = offset_rank(target, mover.forward())?;
        let previous_pawn_square = offset_rank(target, -mover.forward())?;
        let pawn = state
            .entity_at(current_pawn_square)?
            .ok_or_else(|| ChessError::InvalidFen("en-passant pawn is missing".into()))?;
        if pawn.entity_type != PAWN || pawn.owner != mover.player() {
            return Err(ChessError::InvalidFen(
                "en-passant target does not correspond to the pawn that just advanced".into(),
            ));
        }
        if previous_pawn_square.y != mover.pawn_start_rank() {
            return Err(ChessError::InvalidFen(
                "en-passant source square is not the pawn start rank".into(),
            ));
        }
        if state.entity_at(previous_pawn_square)?.is_some() {
            return Err(ChessError::InvalidFen(
                "en-passant source square must be empty in the imported position".into(),
            ));
        }

        let pawn_id = pawn.id;
        let current = state.clone();
        let mut previous = state;
        let mut pawn_before = previous.remove_entity(pawn_id)?;
        pawn_before.position = previous_pawn_square;
        previous.add_entity(pawn_before)?;
        previous.set_active_players(vec![mover.player()])?;
        self.set_halfmove_clock(&mut previous, 0);
        if mover == ChessSide::Black {
            if fullmove_number <= 1 {
                return Err(ChessError::InvalidFen(
                    "a black en-passant move cannot precede fullmove 1".into(),
                ));
            }
            self.set_fullmove_number(&mut previous, fullmove_number - 1);
        } else {
            self.set_fullmove_number(&mut previous, fullmove_number);
        }

        let mut timeline = GameTimeline::new(previous)?;
        let mut turn = timeline.begin_turn(mover.player())?;
        turn.mark_synthetic();
        let movement = self
            .legal_moves(&turn.working, pawn_id)?
            .into_iter()
            .find(|movement| movement.to == current_pawn_square)
            .ok_or_else(|| {
                ChessError::InvalidFen("synthetic en-passant predecessor is not a legal double pawn move".into())
            })?;
        self.execute_move(&mut turn, None, movement, None)?;
        timeline.commit_turn(turn)?;
        if timeline.current() != &current {
            return Err(ChessError::InvalidFen(
                "synthetic en-passant predecessor did not reconstruct the imported state".into(),
            ));
        }

        Ok(FenGame {
            timeline,
            synthetic_history_len: 1,
        })
    }

    fn validate_imported_position(
        &self,
        state: &GameState,
        side_to_move: ChessSide,
    ) -> Result<(), ChessError> {
        if state.entities.len() > 32 {
            return Err(ChessError::InvalidFen(
                "unsupported position: more than 32 pieces".into(),
            ));
        }

        for side in [ChessSide::White, ChessSide::Black] {
            let pieces = state
                .entities
                .values()
                .filter(|entity| entity.owner == side.player())
                .collect::<Vec<_>>();
            if pieces.len() > 16 {
                return Err(ChessError::InvalidFen(format!(
                    "unsupported position: {} has more than sixteen pieces",
                    match side { ChessSide::White => "white", ChessSide::Black => "black" }
                )));
            }
            let pawns = pieces
                .iter()
                .filter(|entity| entity.entity_type == PAWN)
                .count();
            if pawns > 8 {
                return Err(ChessError::InvalidFen(format!(
                    "unsupported position: {} has more than eight pawns",
                    match side { ChessSide::White => "white", ChessSide::Black => "black" }
                )));
            }
            if pieces
                .iter()
                .any(|entity| entity.entity_type == PAWN && (entity.position.y == 0 || entity.position.y == 7))
            {
                return Err(ChessError::InvalidFen(
                    "unsupported position: pawn on the first or eighth rank".into(),
                ));
            }

            let count = |piece_type: EntityTypeId| {
                pieces
                    .iter()
                    .filter(|entity| entity.entity_type == piece_type)
                    .count()
            };
            let promoted_material = count(KNIGHT).saturating_sub(2)
                + count(BISHOP).saturating_sub(2)
                + count(ROOK).saturating_sub(2)
                + count(QUEEN).saturating_sub(1);
            if promoted_material > 8_usize.saturating_sub(pawns) {
                return Err(ChessError::InvalidFen(format!(
                    "unsupported position: {} has more promoted material than missing pawns can explain",
                    match side { ChessSide::White => "white", ChessSide::Black => "black" }
                )));
            }
        }

        // A legal game can leave the side to move in check, but the player who just
        // moved may not have left their own king capturable. This also rejects adjacent kings.
        let previous_side = side_to_move.opponent();
        let previous_king = self.king(state, previous_side)?;
        let previous_king_square = state.entity(previous_king)?.position;
        if self.is_square_attacked(state, side_to_move, previous_king_square)? {
            return Err(ChessError::InvalidFen(
                "unsupported position: the side that just moved left its king in check".into(),
            ));
        }
        Ok(())
    }

    pub fn to_fen(&self, state: &GameState, history: &History) -> Result<String, ChessError> {
        let placement = serialize_placement(state)?;
        let active = match self.side_to_move(state)? {
            ChessSide::White => "w",
            ChessSide::Black => "b",
        };
        let castling = serialize_castling(self.effective_castling_rights(state));
        let en_passant = fen_en_passant_target(state, history)
            .map(square_name)
            .unwrap_or_else(|| "-".into());
        Ok(format!(
            "{placement} {active} {castling} {en_passant} {} {}",
            self.halfmove_clock(state),
            self.fullmove_number(state)
        ))
    }
}

fn parse_placement(state: &mut GameState, field: &str) -> Result<(), ChessError> {
    let ranks = field.split('/').collect::<Vec<_>>();
    if ranks.len() != 8 {
        return Err(ChessError::InvalidFen(
            "piece placement must contain eight ranks".into(),
        ));
    }

    let mut next_id = 1_u32;
    for (fen_rank, rank) in ranks.iter().enumerate() {
        let y = 7_u16 - u16::try_from(fen_rank).expect("fen rank is within 0..8");
        let mut x = 0_u16;
        for symbol in rank.chars() {
            if let Some(empty) = symbol.to_digit(10) {
                if !(1..=8).contains(&empty) {
                    return Err(ChessError::InvalidFen("invalid empty-square digit".into()));
                }
                x = x
                    .checked_add(u16::try_from(empty).expect("empty count is <= 8"))
                    .ok_or_else(|| ChessError::InvalidFen("rank is too wide".into()))?;
                continue;
            }

            if x >= 8 {
                return Err(ChessError::InvalidFen("rank is too wide".into()));
            }
            let (entity_type, side) = piece_from_symbol(symbol).ok_or_else(|| {
                ChessError::InvalidFen(format!("unknown piece symbol {symbol:?}"))
            })?;
            let id = EntityId::new(next_id);
            next_id = next_id.saturating_add(1);
            let mut entity = EntityState::new(id, entity_type, side.player(), Position::new(x, y));
            set_has_moved(&mut entity, true);
            state.add_entity(entity)?;
            x += 1;
        }
        if x != 8 {
            return Err(ChessError::InvalidFen(format!(
                "rank {} expands to {x} squares instead of 8",
                8 - fen_rank
            )));
        }
    }
    Ok(())
}

fn apply_castling_metadata(state: &mut GameState, field: &str) -> Result<(), ChessError> {
    let mut rights = 0_u8;
    if field != "-" {
        if field.is_empty() {
            return Err(ChessError::InvalidFen("empty castling field".into()));
        }
        for symbol in field.chars() {
            let bit = match symbol {
                'K' => 0b0001,
                'Q' => 0b0010,
                'k' => 0b0100,
                'q' => 0b1000,
                _ => return Err(ChessError::InvalidFen("invalid castling field".into())),
            };
            if rights & bit != 0 {
                return Err(ChessError::InvalidFen("duplicate castling right".into()));
            }
            rights |= bit;
        }
    }

    for (side, king_side_bit, queen_side_bit) in [
        (ChessSide::White, 0b0001, 0b0010),
        (ChessSide::Black, 0b0100, 0b1000),
    ] {
        let rank = side.home_rank();
        let any_right = rights & (king_side_bit | queen_side_bit) != 0;
        if let Some(king_id) = entity_id_at(state, Position::new(4, rank))? {
            let king = state.entity_mut(king_id)?;
            if king.entity_type == KING && king.owner == side.player() {
                set_has_moved(king, !any_right);
            } else if any_right {
                return Err(ChessError::InvalidFen("castling right has no matching king".into()));
            }
        } else if any_right {
            return Err(ChessError::InvalidFen("castling right has no matching king".into()));
        }

        for (x, bit) in [(7_u16, king_side_bit), (0_u16, queen_side_bit)] {
            let has_right = rights & bit != 0;
            if let Some(rook_id) = entity_id_at(state, Position::new(x, rank))? {
                let rook = state.entity_mut(rook_id)?;
                if rook.entity_type == ROOK && rook.owner == side.player() {
                    set_has_moved(rook, !has_right);
                } else if has_right {
                    return Err(ChessError::InvalidFen("castling right has no matching rook".into()));
                }
            } else if has_right {
                return Err(ChessError::InvalidFen("castling right has no matching rook".into()));
            }
        }
    }
    Ok(())
}

fn entity_id_at(state: &GameState, position: Position) -> Result<Option<EntityId>, ChessError> {
    Ok(state.entity_at(position)?.map(|entity| entity.id))
}

fn piece_from_symbol(symbol: char) -> Option<(EntityTypeId, ChessSide)> {
    let side = if symbol.is_ascii_uppercase() {
        ChessSide::White
    } else {
        ChessSide::Black
    };
    let kind = match symbol.to_ascii_lowercase() {
        'p' => PAWN,
        'n' => KNIGHT,
        'b' => BISHOP,
        'r' => ROOK,
        'q' => QUEEN,
        'k' => KING,
        _ => return None,
    };
    Some((kind, side))
}

fn symbol_for_piece(entity_type: EntityTypeId, side: ChessSide) -> Result<char, ChessError> {
    let base = if entity_type == PAWN {
        'p'
    } else if entity_type == KNIGHT {
        'n'
    } else if entity_type == BISHOP {
        'b'
    } else if entity_type == ROOK {
        'r'
    } else if entity_type == QUEEN {
        'q'
    } else if entity_type == KING {
        'k'
    } else {
        return Err(ChessError::InvalidFen(format!(
            "entity type {} cannot be serialized as standard FEN",
            entity_type.get()
        )));
    };
    Ok(match side {
        ChessSide::White => base.to_ascii_uppercase(),
        ChessSide::Black => base,
    })
}

fn serialize_placement(state: &GameState) -> Result<String, ChessError> {
    let mut ranks = Vec::with_capacity(8);
    for y in (0..8_u16).rev() {
        let mut rank = String::new();
        let mut empty = 0_u8;
        for x in 0..8_u16 {
            if let Some(entity) = state.entity_at(Position::new(x, y))? {
                if empty > 0 {
                    rank.push(char::from(b'0' + empty));
                    empty = 0;
                }
                let side = ChessSide::from_player(entity.owner)
                    .ok_or(ChessError::UnknownSide(entity.owner))?;
                rank.push(symbol_for_piece(entity.entity_type, side)?);
            } else {
                empty += 1;
            }
        }
        if empty > 0 {
            rank.push(char::from(b'0' + empty));
        }
        ranks.push(rank);
    }
    Ok(ranks.join("/"))
}

fn serialize_castling(rights: u8) -> String {
    let mut result = String::new();
    for (bit, symbol) in [(0b0001, 'K'), (0b0010, 'Q'), (0b0100, 'k'), (0b1000, 'q')] {
        if rights & bit != 0 {
            result.push(symbol);
        }
    }
    if result.is_empty() {
        "-".into()
    } else {
        result
    }
}

fn fen_en_passant_target(state: &GameState, history: &History) -> Option<Position> {
    let previous = history
        .turns()
        .iter()
        .rev()
        .find(|turn| turn.steps.iter().any(|step| step.action.kind == "chess_move"))?;
    // Terminal chess actions (resignation, draw claim/agreement, imported PGN
    // result) intentionally do not change the board or turn. They may add
    // ruleset outcome metadata after the last real chess move, so compare only
    // the position-bearing parts here instead of the complete GameState.
    if previous.after.entities != state.entities || previous.after.turn != state.turn {
        return None;
    }
    for after in previous.after.entities.values().filter(|entity| entity.entity_type == PAWN) {
        let before = previous.before.entities.get(&after.id)?;
        let side = ChessSide::from_player(after.owner)?;
        let was_double_pawn_move =
            turn_records_entity_move(previous, after.id, before.position, after.position);
        if before.entity_type == PAWN
            && before.position.x == after.position.x
            && before.position.y == side.pawn_start_rank()
            && i32::from(after.position.y) - i32::from(before.position.y)
                == i32::from(side.forward() * 2)
            && was_double_pawn_move
        {
            return Some(Position::new(
                after.position.x,
                u16::try_from((u32::from(before.position.y) + u32::from(after.position.y)) / 2)
                    .ok()?,
            ));
        }
    }
    None
}

fn parse_square(value: &str) -> Result<Position, ChessError> {
    let bytes = value.as_bytes();
    if bytes.len() != 2 || !(b'a'..=b'h').contains(&bytes[0]) || !(b'1'..=b'8').contains(&bytes[1]) {
        return Err(ChessError::InvalidFen("invalid algebraic square".into()));
    }
    Ok(Position::new(
        u16::from(bytes[0] - b'a'),
        u16::from(bytes[1] - b'1'),
    ))
}

fn square_name(position: Position) -> String {
    let file = char::from(b'a' + u8::try_from(position.x).unwrap_or(0));
    let rank = char::from(b'1' + u8::try_from(position.y).unwrap_or(0));
    format!("{file}{rank}")
}

fn offset_rank(position: Position, dy: i16) -> Result<Position, ChessError> {
    let y = i32::from(position.y) + i32::from(dy);
    if !(0..8).contains(&y) {
        return Err(ChessError::InvalidFen("en-passant geometry is outside the board".into()));
    }
    Ok(Position::new(position.x, u16::try_from(y).expect("0..8 fits u16")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChessMoveKind, WHITE_PLAYER};

    #[test]
    fn standard_fen_roundtrips() {
        let rules = ChessRules::standard();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let imported = rules.from_fen(fen).unwrap();
        assert_eq!(rules.to_fen(imported.timeline.current(), imported.timeline.history()).unwrap(), fen);
        assert_eq!(imported.synthetic_history_len, 0);
    }

    #[test]
    fn imported_castling_rights_control_piece_state() {
        let rules = ChessRules::standard();
        let imported = rules
            .from_fen("r3k2r/8/8/8/8/8/8/R3K2R w Kq - 7 12")
            .unwrap();
        let state = imported.timeline.current();
        assert!(!crate::piece::has_moved(state.entity_at(Position::new(4, 0)).unwrap().unwrap()));
        assert!(!crate::piece::has_moved(state.entity_at(Position::new(7, 0)).unwrap().unwrap()));
        assert!(crate::piece::has_moved(state.entity_at(Position::new(0, 0)).unwrap().unwrap()));
        assert!(!crate::piece::has_moved(state.entity_at(Position::new(4, 7)).unwrap().unwrap()));
        assert!(!crate::piece::has_moved(state.entity_at(Position::new(0, 7)).unwrap().unwrap()));
        assert!(crate::piece::has_moved(state.entity_at(Position::new(7, 7)).unwrap().unwrap()));
    }

    #[test]
    fn en_passant_import_synthesizes_only_the_required_previous_turn() {
        let rules = ChessRules::standard();
        let fen = "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 17";
        let imported = rules.from_fen(fen).unwrap();
        assert_eq!(imported.synthetic_history_len, 1);
        let previous = imported.timeline.history().previous_turn().unwrap();
        assert!(previous.synthetic);
        assert_eq!(previous.actor, crate::BLACK_PLAYER);

        let white_pawn = imported
            .timeline
            .current()
            .entity_at(Position::new(4, 4))
            .unwrap()
            .unwrap();
        let ep = rules
            .legal_moves_with_history(
                imported.timeline.current(),
                Some(imported.timeline.history()),
                white_pawn.id,
            )
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(3, 5))
            .unwrap();
        assert!(matches!(ep.kind, ChessMoveKind::EnPassant { .. }));
        assert_eq!(rules.to_fen(imported.timeline.current(), imported.timeline.history()).unwrap(), fen);
    }

    #[test]
    fn invalid_fen_is_rejected() {
        let rules = ChessRules::standard();
        assert!(rules.from_fen("8/8/8/8/8/8/8/8 w - - 0 1").is_err());
        assert!(rules
            .from_fen("4k3/8/8/8/8/8/8/4K3 w K - 0 1")
            .is_err());
        assert!(rules
            .from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d3 0 17")
            .is_err());
    }

    #[test]
    fn strict_fen_validation_rejects_impossible_standard_positions() {
        let rules = ChessRules::standard();
        assert!(rules
            .from_fen("4k3/8/8/8/8/8/8/P3K3 w - - 0 1")
            .is_err());
        assert!(rules
            .from_fen("4k3/pppppppp/8/8/8/8/PPPPPPPP/Q3KQQQ w - - 0 1")
            .is_err());
        assert!(rules
            .from_fen("4k3/8/8/8/8/8/4R3/4K3 w - - 0 1")
            .is_err());
    }

    #[test]
    fn terminal_metadata_does_not_erase_fen_en_passant_target() {
        use monumla_core::RecordedAction;

        let rules = ChessRules::standard();
        let imported = rules
            .from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 17")
            .unwrap();
        let mut timeline = imported.timeline;
        let actor = timeline.current().turn.active_players[0];
        let history = timeline.history().clone();
        let mut turn = timeline.begin_turn(actor).unwrap();
        turn.apply_transaction(
            RecordedAction::new("chess_draw_agreement"),
            |transaction| -> Result<(), ChessError> {
                rules.agree_draw(transaction.raw_state_mut(), &history)?;
                Ok(())
            },
        )
        .unwrap();
        timeline.commit_turn(turn).unwrap();

        assert_eq!(
            rules.to_fen(timeline.current(), timeline.history()).unwrap(),
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 17"
        );
    }

    #[test]
    fn fullmove_number_advances_after_black_move() {
        let rules = ChessRules::standard();
        let imported = rules
            .from_fen("4k3/7p/8/8/8/8/P7/4K3 b - - 0 23")
            .unwrap();
        let mut timeline = imported.timeline;
        let pawn = timeline.current().entity_at(Position::new(7, 6)).unwrap().unwrap().id;
        let mut turn = timeline.begin_turn(crate::BLACK_PLAYER).unwrap();
        let movement = rules
            .legal_moves(timeline.current(), pawn)
            .unwrap()
            .into_iter()
            .find(|movement| movement.to == Position::new(7, 5))
            .unwrap();
        rules.execute_move(&mut turn, Some(timeline.history()), movement, None).unwrap();
        timeline.commit_turn(turn).unwrap();
        assert_eq!(rules.halfmove_clock(timeline.current()), 0);
        assert_eq!(rules.fullmove_number(timeline.current()), 24);
        assert!(rules.to_fen(timeline.current(), timeline.history()).unwrap().ends_with(" 0 24"));
        assert_eq!(timeline.current().turn.active_players, vec![WHITE_PLAYER]);
    }
}
