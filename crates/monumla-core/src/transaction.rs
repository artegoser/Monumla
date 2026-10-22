use crate::{
    CoreError, EntityData, EntityId, EntityState, EntityTypeId, GameState, PlayerData, PlayerId,
    PlayerState, Position, RulesetState, StateMap, TeamData, TeamId, TeamState, TurnState,
};
use serde::{Deserialize, Serialize};

/// Non-authoritative semantic information intended for presentation layers.
/// Gameplay correctness must never depend on these cues.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresentationCue {
    pub kind: String,
    pub data: StateMap,
}

impl PresentationCue {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            data: StateMap::new(),
        }
    }

    pub fn with_data(mut self, data: StateMap) -> Self {
        self.data = data;
        self
    }
}

/// Structural description of what differs between two valid game states.
/// This is derived infrastructure, not a language rules are required to use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateChange {
    EntityAdded {
        entity: EntityState,
    },
    EntityRemoved {
        entity: EntityState,
    },
    EntityMoved {
        entity: EntityId,
        from: Position,
        to: Position,
    },
    EntityTypeChanged {
        entity: EntityId,
        from: EntityTypeId,
        to: EntityTypeId,
    },
    EntityOwnerChanged {
        entity: EntityId,
        from: PlayerId,
        to: PlayerId,
    },
    EntityControllerChanged {
        entity: EntityId,
        from: PlayerId,
        to: PlayerId,
    },
    EntityStateChanged {
        entity: EntityId,
        before: EntityData,
        after: EntityData,
    },
    PlayerAdded {
        player: PlayerState,
    },
    PlayerRemoved {
        player: PlayerState,
    },
    PlayerChanged {
        player: PlayerId,
        before: PlayerData,
        after: PlayerData,
        before_team: Option<TeamId>,
        after_team: Option<TeamId>,
    },
    TeamAdded {
        team: TeamState,
    },
    TeamRemoved {
        team: TeamState,
    },
    TeamChanged {
        team: TeamId,
        before: TeamData,
        after: TeamData,
    },
    TurnChanged {
        before: TurnState,
        after: TurnState,
    },
    RulesetStateChanged {
        before: RulesetState,
        after: RulesetState,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateDelta {
    pub changes: Vec<StateChange>,
}

impl StateDelta {
    pub fn between(before: &GameState, after: &GameState) -> Self {
        let mut changes = Vec::new();

        for (id, old) in &before.entities {
            let Some(new) = after.entities.get(id) else {
                changes.push(StateChange::EntityRemoved {
                    entity: old.clone(),
                });
                continue;
            };

            if old.position != new.position {
                changes.push(StateChange::EntityMoved {
                    entity: *id,
                    from: old.position,
                    to: new.position,
                });
            }
            if old.entity_type != new.entity_type {
                changes.push(StateChange::EntityTypeChanged {
                    entity: *id,
                    from: old.entity_type,
                    to: new.entity_type,
                });
            }
            if old.owner != new.owner {
                changes.push(StateChange::EntityOwnerChanged {
                    entity: *id,
                    from: old.owner,
                    to: new.owner,
                });
            }
            if old.controller != new.controller {
                changes.push(StateChange::EntityControllerChanged {
                    entity: *id,
                    from: old.controller,
                    to: new.controller,
                });
            }
            if old.state != new.state {
                changes.push(StateChange::EntityStateChanged {
                    entity: *id,
                    before: old.state.clone(),
                    after: new.state.clone(),
                });
            }
        }

        for (id, entity) in &after.entities {
            if !before.entities.contains_key(id) {
                changes.push(StateChange::EntityAdded {
                    entity: entity.clone(),
                });
            }
        }

        for (id, old) in &before.players {
            let Some(new) = after.players.get(id) else {
                changes.push(StateChange::PlayerRemoved {
                    player: old.clone(),
                });
                continue;
            };
            if old.team != new.team || old.state != new.state {
                changes.push(StateChange::PlayerChanged {
                    player: *id,
                    before: old.state.clone(),
                    after: new.state.clone(),
                    before_team: old.team,
                    after_team: new.team,
                });
            }
        }
        for (id, player) in &after.players {
            if !before.players.contains_key(id) {
                changes.push(StateChange::PlayerAdded {
                    player: player.clone(),
                });
            }
        }

        for (id, old) in &before.teams {
            let Some(new) = after.teams.get(id) else {
                changes.push(StateChange::TeamRemoved { team: old.clone() });
                continue;
            };
            if old.state != new.state {
                changes.push(StateChange::TeamChanged {
                    team: *id,
                    before: old.state.clone(),
                    after: new.state.clone(),
                });
            }
        }
        for (id, team) in &after.teams {
            if !before.teams.contains_key(id) {
                changes.push(StateChange::TeamAdded { team: team.clone() });
            }
        }

        if before.turn != after.turn {
            changes.push(StateChange::TurnChanged {
                before: before.turn.clone(),
                after: after.turn.clone(),
            });
        }
        if before.ruleset_state != after.ruleset_state {
            changes.push(StateChange::RulesetStateChanged {
                before: before.ruleset_state.clone(),
                after: after.ruleset_state.clone(),
            });
        }

        Self { changes }
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionOutcome {
    pub state: GameState,
    pub delta: StateDelta,
    pub presentation: Vec<PresentationCue>,
}

/// A disposable working copy used to apply one game step atomically.
/// Nothing here touches committed state until the caller accepts `finish()`.
pub struct Transaction {
    before: GameState,
    working: GameState,
    presentation: Vec<PresentationCue>,
}

impl Transaction {
    pub fn new(state: &GameState) -> Self {
        Self {
            before: state.clone(),
            working: state.clone(),
            presentation: Vec::new(),
        }
    }

    pub fn state(&self) -> &GameState {
        &self.working
    }

    /// Controlled escape hatch for mechanics that cannot be expressed through
    /// convenience helpers. The resulting state is still validated at finish.
    pub fn raw_state_mut(&mut self) -> &mut GameState {
        &mut self.working
    }

    pub fn entity(&self, entity: EntityId) -> Result<&EntityState, CoreError> {
        self.working.entity(entity)
    }

    pub fn entity_mut(&mut self, entity: EntityId) -> Result<&mut EntityState, CoreError> {
        self.working.entity_mut(entity)
    }

    pub fn move_entity(&mut self, entity: EntityId, to: Position) -> Result<(), CoreError> {
        self.working.move_entity(entity, to)
    }

    pub fn spawn_entity(&mut self, entity: EntityState) -> Result<(), CoreError> {
        self.working.add_entity(entity)
    }

    pub fn remove_entity(&mut self, entity: EntityId) -> Result<EntityState, CoreError> {
        self.working.remove_entity(entity)
    }

    pub fn player_mut(&mut self, player: PlayerId) -> Result<&mut PlayerState, CoreError> {
        self.working
            .players
            .get_mut(&player)
            .ok_or(CoreError::PlayerNotFound(player))
    }

    pub fn team_mut(&mut self, team: TeamId) -> Result<&mut TeamState, CoreError> {
        self.working
            .teams
            .get_mut(&team)
            .ok_or(CoreError::TeamNotFound(team))
    }

    pub fn ruleset_state_mut(&mut self) -> &mut RulesetState {
        &mut self.working.ruleset_state
    }

    pub fn turn_state_mut(&mut self) -> &mut TurnState {
        &mut self.working.turn
    }

    pub fn present(&mut self, cue: PresentationCue) {
        self.presentation.push(cue);
    }

    pub fn finish(self) -> Result<TransactionOutcome, CoreError> {
        self.working.validate()?;
        let delta = StateDelta::between(&self.before, &self.working);
        Ok(TransactionOutcome {
            state: self.working,
            delta,
            presentation: self.presentation,
        })
    }
}
