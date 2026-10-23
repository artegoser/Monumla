//! Reusable action data; game rules decide which paths and inputs are legal.
use crate::{ChoiceInput, EntityId, Position};
use serde::{Deserialize, Serialize};

/// A complete entity path, including landings and removed entities in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityActionPath {
    pub actor: EntityId,
    pub landings: Vec<Position>,
    pub captures: Vec<EntityId>,
}

/// Stable inputs for one complete turn, independent of transient choice IDs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnInput {
    pub choices: Vec<ChoiceInput>,
}
