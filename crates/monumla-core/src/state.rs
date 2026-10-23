use crate::{Board, CoreError, EntityId, EntityTypeId, PlayerId, Position, StateMap, TeamId};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

pub type EntityData = StateMap;
pub type PlayerData = StateMap;
pub type TeamData = StateMap;
pub type RulesetState = StateMap;
pub type TurnData = StateMap;
pub type EntityStore = BTreeMap<EntityId, EntityState>;
pub type PlayerStore = BTreeMap<PlayerId, PlayerState>;
pub type TeamStore = BTreeMap<TeamId, TeamState>;

#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub struct EntityState {
    pub id: EntityId,
    pub entity_type: EntityTypeId,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub position: Position,
    pub state: EntityData,
}

impl EntityState {
    pub fn new(
        id: EntityId,
        entity_type: EntityTypeId,
        owner: PlayerId,
        position: Position,
    ) -> Self {
        Self {
            id,
            entity_type,
            owner,
            controller: owner,
            position,
            state: EntityData::new(),
        }
    }

    pub fn with_controller(mut self, controller: PlayerId) -> Self {
        self.controller = controller;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PlayerId,
    pub team: Option<TeamId>,
    pub state: PlayerData,
}

impl PlayerState {
    pub fn new(id: PlayerId) -> Self {
        Self {
            id,
            team: None,
            state: PlayerData::new(),
        }
    }

    pub fn with_team(mut self, team: TeamId) -> Self {
        self.team = Some(team);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub struct TeamState {
    pub id: TeamId,
    pub state: TeamData,
}

impl TeamState {
    pub fn new(id: TeamId) -> Self {
        Self {
            id,
            state: TeamData::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Hash, Serialize, Deserialize)]
pub struct TurnState {
    pub active_players: Vec<PlayerId>,
    pub state: TurnData,
}

#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub struct GameStateData {
    pub board: Board,
    pub entities: EntityStore,
    pub players: PlayerStore,
    pub teams: TeamStore,
    pub turn: TurnState,
    pub ruleset_state: RulesetState,
}

/// Immutable snapshots share storage. The first write isolates the branch.
#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameState(Arc<StateSnapshot>);
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct StateSnapshot {
    data: GameStateData,
    #[serde(skip)]
    digest: OnceLock<u128>,
}
impl PartialEq for StateSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}
impl std::hash::Hash for StateSnapshot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}
impl Deref for GameState {
    type Target = GameStateData;
    fn deref(&self) -> &GameStateData {
        &self.0.data
    }
}
impl DerefMut for GameState {
    fn deref_mut(&mut self) -> &mut GameStateData {
        let snapshot = Arc::make_mut(&mut self.0);
        snapshot.digest.take();
        &mut snapshot.data
    }
}

impl GameState {
    /// Memoized content identity. Every mutable projection invalidates the cache.
    pub fn semantic_digest(&self) -> u128 {
        *self
            .0
            .digest
            .get_or_init(|| crate::fingerprint(&self.0.data))
    }
    pub fn new(width: u16, height: u16) -> Result<Self, CoreError> {
        Ok(Self::from_board(Board::new(width, height)?))
    }

    pub fn from_board(board: Board) -> Self {
        Self(Arc::new(StateSnapshot {
            data: GameStateData {
                board,
                entities: EntityStore::new(),
                players: PlayerStore::new(),
                teams: TeamStore::new(),
                turn: TurnState::default(),
                ruleset_state: RulesetState::new(),
            },
            digest: OnceLock::new(),
        }))
    }

    pub fn add_team(&mut self, team: TeamState) -> Result<(), CoreError> {
        if self.teams.contains_key(&team.id) {
            return Err(CoreError::DuplicateTeam(team.id));
        }
        self.teams.insert(team.id, team);
        Ok(())
    }

    pub fn add_player(&mut self, player: PlayerState) -> Result<(), CoreError> {
        if self.players.contains_key(&player.id) {
            return Err(CoreError::DuplicatePlayer(player.id));
        }
        if let Some(team) = player.team {
            if !self.teams.contains_key(&team) {
                return Err(CoreError::TeamNotFound(team));
            }
        }
        self.players.insert(player.id, player);
        Ok(())
    }

    pub fn add_entity(&mut self, entity: EntityState) -> Result<(), CoreError> {
        if self.entities.contains_key(&entity.id) {
            return Err(CoreError::DuplicateEntity(entity.id));
        }
        self.ensure_player(entity.owner)?;
        self.ensure_player(entity.controller)?;
        self.board.place(entity.position, entity.id)?;
        self.entities.insert(entity.id, entity);
        Ok(())
    }

    pub fn entity_at(&self, position: Position) -> Result<Option<&EntityState>, CoreError> {
        let Some(id) = self.board.entity_at(position)? else {
            return Ok(None);
        };
        Ok(self.entities.get(&id))
    }

    pub fn entity(&self, id: EntityId) -> Result<&EntityState, CoreError> {
        self.entities.get(&id).ok_or(CoreError::EntityNotFound(id))
    }

    pub fn entity_mut(&mut self, id: EntityId) -> Result<&mut EntityState, CoreError> {
        self.entities
            .get_mut(&id)
            .ok_or(CoreError::EntityNotFound(id))
    }

    pub fn move_entity(&mut self, id: EntityId, to: Position) -> Result<(), CoreError> {
        let from = self.entity(id)?.position;
        if from == to {
            return Ok(());
        }
        if let Some(occupant) = self.board.entity_at(to)? {
            return Err(CoreError::PositionOccupied {
                position: to,
                entity: occupant,
            });
        }

        self.board.clear(from, id)?;
        self.board.place(to, id)?;
        self.entity_mut(id)?.position = to;
        Ok(())
    }

    pub fn remove_entity(&mut self, id: EntityId) -> Result<EntityState, CoreError> {
        let position = self.entity(id)?.position;
        self.board.clear(position, id)?;
        self.entities
            .remove(&id)
            .ok_or(CoreError::EntityNotFound(id))
    }

    pub fn set_owner(&mut self, entity: EntityId, owner: PlayerId) -> Result<(), CoreError> {
        self.ensure_player(owner)?;
        self.entity_mut(entity)?.owner = owner;
        Ok(())
    }

    pub fn set_controller(
        &mut self,
        entity: EntityId,
        controller: PlayerId,
    ) -> Result<(), CoreError> {
        self.ensure_player(controller)?;
        self.entity_mut(entity)?.controller = controller;
        Ok(())
    }

    pub fn set_active_players(&mut self, players: Vec<PlayerId>) -> Result<(), CoreError> {
        let mut seen = BTreeSet::new();
        for player in &players {
            self.ensure_player(*player)?;
            if !seen.insert(*player) {
                return Err(CoreError::DuplicateActivePlayer(*player));
            }
        }
        self.turn.active_players = players;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        self.board.validate_storage()?;

        for player in self.players.values() {
            if let Some(team) = player.team {
                if !self.teams.contains_key(&team) {
                    return Err(CoreError::TeamNotFound(team));
                }
            }
        }

        let mut active = BTreeSet::new();
        for player in &self.turn.active_players {
            self.ensure_player(*player)?;
            if !active.insert(*player) {
                return Err(CoreError::DuplicateActivePlayer(*player));
            }
        }

        for entity in self.entities.values() {
            self.ensure_player(entity.owner)?;
            self.ensure_player(entity.controller)?;
            match self.board.entity_at(entity.position)? {
                Some(board_id) if board_id == entity.id => {}
                _ => {
                    return Err(CoreError::EntityPlacementMismatch {
                        entity: entity.id,
                        position: entity.position,
                    });
                }
            }
        }

        for (index, entity) in self.board.raw_cells().iter().copied().enumerate() {
            let Some(entity_id) = entity else {
                continue;
            };
            let position = self
                .board
                .position_for_index(index)
                .ok_or(CoreError::InvalidBoardStorage)?;
            let Some(entity_state) = self.entities.get(&entity_id) else {
                return Err(CoreError::DanglingBoardEntity {
                    entity: entity_id,
                    position,
                });
            };
            if entity_state.position != position {
                return Err(CoreError::BoardEntityPositionMismatch {
                    entity: entity_id,
                    actual: position,
                    declared: entity_state.position,
                });
            }
        }

        Ok(())
    }

    pub fn speculate<T>(
        &self,
        operation: impl FnOnce(&mut GameState) -> Result<T, CoreError>,
    ) -> Result<(GameState, T), CoreError> {
        let mut candidate = self.clone();
        let value = operation(&mut candidate)?;
        candidate.validate()?;
        Ok((candidate, value))
    }

    fn ensure_player(&self, player: PlayerId) -> Result<(), CoreError> {
        if self.players.contains_key(&player) {
            Ok(())
        } else {
            Err(CoreError::PlayerNotFound(player))
        }
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn branches_isolate_board_entities_and_direct_field_edits() {
        let mut original = GameState::new(4, 4).unwrap();
        original
            .add_player(PlayerState::new(PlayerId::new(1)))
            .unwrap();
        original
            .add_entity(EntityState::new(
                EntityId::new(1),
                EntityTypeId::new(1),
                PlayerId::new(1),
                Position::new(0, 0),
            ))
            .unwrap();
        let before = original.semantic_digest();
        let mut branch = original.clone();
        assert_eq!(branch.semantic_digest(), before);
        branch
            .move_entity(EntityId::new(1), Position::new(2, 2))
            .unwrap();
        branch
            .entities
            .get_mut(&EntityId::new(1))
            .unwrap()
            .state
            .insert("changed", true);
        assert_ne!(branch.semantic_digest(), before);
        assert_eq!(original.semantic_digest(), before);
        assert_eq!(
            original.entity(EntityId::new(1)).unwrap().position,
            Position::new(0, 0)
        );
        assert!(!original
            .entity(EntityId::new(1))
            .unwrap()
            .state
            .contains_key("changed"));
        original.validate().unwrap();
        branch.validate().unwrap();
        let digest = branch.semantic_digest();
        branch.ruleset_state.insert("phase", "next");
        assert_ne!(
            branch.semantic_digest(),
            digest,
            "direct public-field mutation must invalidate identity"
        );
    }
}
