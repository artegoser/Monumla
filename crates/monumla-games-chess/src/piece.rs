use crate::{ChessError, ChessSide};
use monumla_core::{
    ChoiceInput, ChoiceSpec, EntityId, EntityPresentation, EntityRule, EntityRuleContext,
    EntityState, EntityTypeId, GameState, History, Position, RuleError,
};
use serde::{Deserialize, Serialize};

pub const PAWN: EntityTypeId = EntityTypeId::new(1);
pub const KNIGHT: EntityTypeId = EntityTypeId::new(2);
pub const BISHOP: EntityTypeId = EntityTypeId::new(3);
pub const ROOK: EntityTypeId = EntityTypeId::new(4);
pub const QUEEN: EntityTypeId = EntityTypeId::new(5);
pub const KING: EntityTypeId = EntityTypeId::new(6);

pub(crate) const HAS_MOVED_STATE: &str = "has_moved";

pub(crate) const fn tracks_has_moved(entity_type: EntityTypeId) -> bool {
    entity_type.get() == KING.get() || entity_type.get() == ROOK.get()
}

pub(crate) fn has_moved(entity: &EntityState) -> bool {
    entity
        .state
        .get(HAS_MOVED_STATE)
        .and_then(monumla_core::StateValue::as_bool)
        .unwrap_or(false)
}

pub(crate) fn set_has_moved(entity: &mut EntityState, moved: bool) {
    if tracks_has_moved(entity.entity_type) {
        entity.state.insert(HAS_MOVED_STATE, moved);
    } else {
        entity.state.remove(HAS_MOVED_STATE);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChessPieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl ChessPieceKind {
    pub const fn entity_type(self) -> EntityTypeId {
        match self {
            Self::Pawn => PAWN,
            Self::Knight => KNIGHT,
            Self::Bishop => BISHOP,
            Self::Rook => ROOK,
            Self::Queen => QUEEN,
            Self::King => KING,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Pawn => "pawn",
            Self::Knight => "knight",
            Self::Bishop => "bishop",
            Self::Rook => "rook",
            Self::Queen => "queen",
            Self::King => "king",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChessMoveKind {
    Normal,
    EnPassant { victim: EntityId },
    Castle { rook: EntityId, rook_to: Position },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PseudoMove {
    pub actor: EntityId,
    pub from: Position,
    pub to: Position,
    pub capture: Option<EntityId>,
    pub kind: ChessMoveKind,
}

impl PseudoMove {
    pub const fn new(
        actor: EntityId,
        from: Position,
        to: Position,
        capture: Option<EntityId>,
    ) -> Self {
        Self {
            actor,
            from,
            to,
            capture,
            kind: ChessMoveKind::Normal,
        }
    }

    pub const fn en_passant(
        actor: EntityId,
        from: Position,
        to: Position,
        victim: EntityId,
    ) -> Self {
        Self {
            actor,
            from,
            to,
            capture: Some(victim),
            kind: ChessMoveKind::EnPassant { victim },
        }
    }

    pub const fn castle(
        actor: EntityId,
        from: Position,
        to: Position,
        rook: EntityId,
        rook_to: Position,
    ) -> Self {
        Self {
            actor,
            from,
            to,
            capture: None,
            kind: ChessMoveKind::Castle { rook, rook_to },
        }
    }
}

#[derive(Clone, Copy)]
pub struct ChessPieceContext<'a> {
    state: &'a GameState,
    history: Option<&'a History>,
    entity: &'a EntityState,
}

impl<'a> ChessPieceContext<'a> {
    pub fn new(state: &'a GameState, entity: EntityId) -> Result<Self, ChessError> {
        Self::with_history(state, None, entity)
    }

    pub fn with_history(
        state: &'a GameState,
        history: Option<&'a History>,
        entity: EntityId,
    ) -> Result<Self, ChessError> {
        Ok(Self {
            state,
            history,
            entity: state.entity(entity)?,
        })
    }

    pub const fn state(self) -> &'a GameState {
        self.state
    }

    pub const fn entity(self) -> &'a EntityState {
        self.entity
    }

    pub const fn history(self) -> Option<&'a History> {
        self.history
    }

    pub fn side(self) -> Result<ChessSide, ChessError> {
        ChessSide::from_player(self.entity.owner).ok_or(ChessError::UnknownSide(self.entity.owner))
    }

    pub fn entity_at(self, position: Position) -> Result<Option<&'a EntityState>, ChessError> {
        Ok(self.state.entity_at(position)?)
    }

    pub fn capture_at(self, position: Position) -> Result<Option<EntityId>, ChessError> {
        Ok(match self.entity_at(position)? {
            Some(other) if other.owner != self.entity.owner => Some(other.id),
            _ => None,
        })
    }

    pub fn is_empty(self, position: Position) -> Result<bool, ChessError> {
        Ok(self.entity_at(position)?.is_none())
    }

    pub fn can_move_to(self, position: Position) -> Result<bool, ChessError> {
        Ok(match self.entity_at(position)? {
            None => true,
            Some(other) => other.owner != self.entity.owner,
        })
    }

    pub fn pseudo_move(self, to: Position) -> Result<PseudoMove, ChessError> {
        Ok(PseudoMove::new(
            self.entity.id,
            self.entity.position,
            to,
            self.capture_at(to)?,
        ))
    }
}

pub trait ChessPieceRule: EntityRule {
    fn entity_type(&self) -> EntityTypeId;

    fn pseudo_moves(&self, context: ChessPieceContext<'_>) -> Result<Vec<PseudoMove>, ChessError>;

    fn attacks(&self, context: ChessPieceContext<'_>) -> Result<Vec<Position>, ChessError>;

    /// Piece-local continuation choices required to complete a selected move.
    /// Core and the chess interaction coordinator treat these as opaque generic
    /// choices; only the piece rule owns their meaning.
    fn move_choices(
        &self,
        _context: ChessPieceContext<'_>,
        _movement: &PseudoMove,
    ) -> Result<Vec<ChoiceSpec>, ChessError> {
        Ok(Vec::new())
    }

    /// Validate semantic move input selected from `move_choices`. Most pieces
    /// do not need any continuation input and therefore reject any input.
    fn validate_move_input(
        &self,
        context: ChessPieceContext<'_>,
        _movement: &PseudoMove,
        input: Option<&ChoiceInput>,
    ) -> Result<(), ChessError> {
        if input.is_none() {
            Ok(())
        } else {
            Err(ChessError::UnexpectedMoveInput(context.entity().id))
        }
    }

    /// Apply piece-local consequences after the geometric move has been
    /// performed but before the turn is committed.
    fn apply_move_input(
        &self,
        _state: &mut GameState,
        _movement: &PseudoMove,
        _input: Option<&ChoiceInput>,
    ) -> Result<(), ChessError> {
        Ok(())
    }
}

pub(crate) fn offset(state: &GameState, from: Position, dx: i16, dy: i16) -> Option<Position> {
    let x = i32::from(from.x) + i32::from(dx);
    let y = i32::from(from.y) + i32::from(dy);
    if x < 0 || y < 0 {
        return None;
    }
    let x = u16::try_from(x).ok()?;
    let y = u16::try_from(y).ok()?;
    let position = Position::new(x, y);
    state.board.contains(position).then_some(position)
}

pub(crate) fn leaper_moves(
    context: ChessPieceContext<'_>,
    offsets: &[(i16, i16)],
) -> Result<Vec<PseudoMove>, ChessError> {
    let mut moves = Vec::new();
    for &(dx, dy) in offsets {
        let Some(to) = offset(context.state(), context.entity().position, dx, dy) else {
            continue;
        };
        if context.can_move_to(to)? {
            moves.push(context.pseudo_move(to)?);
        }
    }
    Ok(moves)
}

pub(crate) fn leaper_attacks(
    context: ChessPieceContext<'_>,
    offsets: &[(i16, i16)],
) -> Vec<Position> {
    offsets
        .iter()
        .filter_map(|&(dx, dy)| offset(context.state(), context.entity().position, dx, dy))
        .collect()
}

pub(crate) fn ray_moves(
    context: ChessPieceContext<'_>,
    directions: &[(i16, i16)],
) -> Result<Vec<PseudoMove>, ChessError> {
    let mut moves = Vec::new();
    for &(dx, dy) in directions {
        let mut current = context.entity().position;
        while let Some(to) = offset(context.state(), current, dx, dy) {
            match context.entity_at(to)? {
                None => moves.push(context.pseudo_move(to)?),
                Some(other) => {
                    if other.owner != context.entity().owner {
                        moves.push(context.pseudo_move(to)?);
                    }
                    break;
                }
            }
            current = to;
        }
    }
    Ok(moves)
}

pub(crate) fn ray_attacks(
    context: ChessPieceContext<'_>,
    directions: &[(i16, i16)],
) -> Result<Vec<Position>, ChessError> {
    let mut attacks = Vec::new();
    for &(dx, dy) in directions {
        let mut current = context.entity().position;
        while let Some(to) = offset(context.state(), current, dx, dy) {
            attacks.push(to);
            if context.entity_at(to)?.is_some() {
                break;
            }
            current = to;
        }
    }
    Ok(attacks)
}

pub(crate) fn standard_presentation(
    context: EntityRuleContext<'_>,
    kind: ChessPieceKind,
) -> Result<EntityPresentation, RuleError> {
    let side = ChessSide::from_player(context.entity().owner).ok_or_else(|| {
        RuleError::Rejected(format!(
            "entity {} is owned by non-chess player {}",
            context.entity().id,
            context.entity().owner
        ))
    })?;
    let side_name = match side {
        ChessSide::White => "white",
        ChessSide::Black => "black",
    };
    Ok(
        EntityPresentation::new(format!("chess/{side_name}/{}", kind.name()))
            .with_label(kind.name()),
    )
}
