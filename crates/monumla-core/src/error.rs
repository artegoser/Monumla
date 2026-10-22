use crate::{EntityId, PlayerId, Position, TeamId};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("board dimensions must be non-zero")]
    InvalidBoardDimensions,
    #[error("board dimensions overflow addressable storage")]
    BoardTooLarge,
    #[error("position {0} is outside the board")]
    PositionOutOfBounds(Position),
    #[error("position {position} is occupied by entity {entity}")]
    PositionOccupied {
        position: Position,
        entity: EntityId,
    },
    #[error("entity {0} does not exist")]
    EntityNotFound(EntityId),
    #[error("player {0} does not exist")]
    PlayerNotFound(PlayerId),
    #[error("team {0} does not exist")]
    TeamNotFound(TeamId),
    #[error("entity {0} already exists")]
    DuplicateEntity(EntityId),
    #[error("player {0} already exists")]
    DuplicatePlayer(PlayerId),
    #[error("team {0} already exists")]
    DuplicateTeam(TeamId),
    #[error("active player {0} appears more than once")]
    DuplicateActivePlayer(PlayerId),
    #[error("board storage length does not match its dimensions")]
    InvalidBoardStorage,
    #[error("entity {entity} is not stored at its declared position {position}")]
    EntityPlacementMismatch {
        entity: EntityId,
        position: Position,
    },
    #[error("board contains unknown entity {entity} at {position}")]
    DanglingBoardEntity {
        entity: EntityId,
        position: Position,
    },
    #[error("entity {entity} appears on the board at {actual} but declares {declared}")]
    BoardEntityPositionMismatch {
        entity: EntityId,
        actual: Position,
        declared: Position,
    },
    #[error("turn session was created from a different committed state")]
    TurnStateMismatch,
    #[error("turn actor {0} does not exist")]
    TurnActorNotFound(PlayerId),
}
