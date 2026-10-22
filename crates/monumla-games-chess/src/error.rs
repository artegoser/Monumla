use monumla_core::{CoreError, EntityId, EntityTypeId, PlayerId, Position, RuleError};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ChessError {
    #[error("invalid FEN: {0}")]
    InvalidFen(String),
    #[error("invalid SAN: {0}")]
    InvalidSan(String),
    #[error("ambiguous SAN: {0}")]
    AmbiguousSan(String),
    #[error("invalid PGN: {0}")]
    InvalidPgn(String),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Rule(#[from] RuleError),
    #[error("player {0} is not a standard chess side")]
    UnknownSide(PlayerId),
    #[error("entity type {0} has no registered chess piece rule")]
    PieceRuleNotFound(EntityTypeId),
    #[error("entity {entity} uses type {actual}, expected {expected}")]
    WrongPieceType {
        entity: EntityId,
        expected: EntityTypeId,
        actual: EntityTypeId,
    },
    #[error("chess piece rule {0} is already registered")]
    DuplicatePieceRule(EntityTypeId),
    #[error("chess side/player {0} has no king")]
    MissingKing(PlayerId),
    #[error("chess side/player {0} has multiple kings")]
    MultipleKings(PlayerId),
    #[error("move for entity {0} was generated for a stale position")]
    StaleMove(EntityId),
    #[error("entity {0} has no legal move to {1}")]
    IllegalMove(EntityId, Position),
    #[error("pawn {0} requires a promotion choice")]
    PromotionRequired(EntityId),
    #[error("entity type {0} is not a legal promotion type")]
    InvalidPromotion(EntityTypeId),
    #[error("entity {0} cannot promote on this move")]
    UnexpectedPromotion(EntityId),
    #[error("entity {0} does not accept move input for this move")]
    UnexpectedMoveInput(EntityId),
    #[error("move input for entity {0} is rejected by the active game rules")]
    MoveInputRejected(EntityId),
    #[error("standard chess requires exactly one active player")]
    InvalidTurnState,
    #[error("draw by agreement is unavailable until both players have made at least one move")]
    DrawAgreementUnavailable,
    #[error("the requested draw claim is not currently valid")]
    InvalidDrawClaim,
    #[error("stored chess outcome state is invalid")]
    InvalidOutcomeState,
    #[error("the chess game is already finished")]
    GameFinished,
}
