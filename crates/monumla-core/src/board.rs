use crate::{CoreError, EntityId};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
    width: u16,
    height: u16,
    cells: Vec<Option<EntityId>>,
}

impl Board {
    pub fn new(width: u16, height: u16) -> Result<Self, CoreError> {
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidBoardDimensions);
        }

        let len = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or(CoreError::BoardTooLarge)?;

        Ok(Self {
            width,
            height,
            cells: vec![None; len],
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn contains(&self, position: Position) -> bool {
        position.x < self.width && position.y < self.height
    }

    pub fn entity_at(&self, position: Position) -> Result<Option<EntityId>, CoreError> {
        Ok(self.cells[self.index(position)?])
    }

    pub fn positions(&self) -> impl Iterator<Item = Position> + '_ {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| Position::new(x, y)))
    }

    pub(crate) fn place(&mut self, position: Position, entity: EntityId) -> Result<(), CoreError> {
        let index = self.index(position)?;
        if let Some(existing) = self.cells[index] {
            return Err(CoreError::PositionOccupied {
                position,
                entity: existing,
            });
        }
        self.cells[index] = Some(entity);
        Ok(())
    }

    pub(crate) fn clear(
        &mut self,
        position: Position,
        expected: EntityId,
    ) -> Result<(), CoreError> {
        let index = self.index(position)?;
        if self.cells[index] == Some(expected) {
            self.cells[index] = None;
        }
        Ok(())
    }

    pub(crate) fn raw_cells(&self) -> &[Option<EntityId>] {
        &self.cells
    }

    pub(crate) fn validate_storage(&self) -> Result<(), CoreError> {
        if self.width == 0 || self.height == 0 {
            return Err(CoreError::InvalidBoardDimensions);
        }

        let expected = usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .ok_or(CoreError::BoardTooLarge)?;
        if self.cells.len() != expected {
            return Err(CoreError::InvalidBoardStorage);
        }
        Ok(())
    }

    pub(crate) fn position_for_index(&self, index: usize) -> Option<Position> {
        if self.width == 0 || index >= self.cells.len() {
            return None;
        }
        let width = usize::from(self.width);
        let x = u16::try_from(index % width).ok()?;
        let y = u16::try_from(index / width).ok()?;
        Some(Position::new(x, y))
    }

    fn index(&self, position: Position) -> Result<usize, CoreError> {
        if !self.contains(position) {
            return Err(CoreError::PositionOutOfBounds(position));
        }
        Ok(usize::from(position.y) * usize::from(self.width) + usize::from(position.x))
    }
}
