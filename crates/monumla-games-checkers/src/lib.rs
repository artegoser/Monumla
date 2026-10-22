//! Russian and international draughts on the shared Monumla runtime.
#![forbid(unsafe_code)]

mod interaction;
mod moves;
mod outcome;
pub mod search;

pub use interaction::CheckersInteractionRules;
pub use moves::{legal_moves, play_move, DraughtsMove};
pub use outcome::CheckersOutcomeRule;

use monumla_core::{
    EntityId, EntityPresentation, EntityRule, EntityRuleContext, EntityState, EntityTypeId,
    GameState, PlayerId, PlayerState, Position, RuleError, RuleRegistry, StateValue,
};

pub const CHECKER: EntityTypeId = EntityTypeId::new(1);
pub const WHITE: PlayerId = PlayerId::new(1);
pub const BLACK: PlayerId = PlayerId::new(2);
pub const KING: &str = "checkers.king";
const VARIANT: &str = "checkers.variant";
const PENDING: &str = "checkers.pending_captures";
const FORCED: &str = "checkers.forced";

/// Variants share entities and interaction, while owning their rule differences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DraughtsVariant {
    #[default]
    Russian,
    International,
}

impl DraughtsVariant {
    pub const fn size(self) -> u16 {
        match self {
            Self::Russian => 8,
            Self::International => 10,
        }
    }
    pub const fn id(self) -> &'static str {
        match self {
            Self::Russian => "checkers",
            Self::International => "checkers-international",
        }
    }
    pub const fn title(self) -> &'static str {
        match self {
            Self::Russian => "Russian draughts",
            Self::International => "International draughts",
        }
    }
    pub fn from_state(state: &GameState) -> Self {
        match state
            .ruleset_state
            .get(VARIANT)
            .and_then(StateValue::as_str)
        {
            Some("international") => Self::International,
            _ => Self::Russian,
        }
    }
}

pub fn standard_state() -> GameState {
    state_for_variant(DraughtsVariant::Russian)
}

pub fn state_for_variant(variant: DraughtsVariant) -> GameState {
    let size = variant.size();
    let mut state = GameState::new(size, size).expect("valid draughts board");
    state.ruleset_state.insert(
        VARIANT,
        match variant {
            DraughtsVariant::Russian => "russian",
            DraughtsVariant::International => "international",
        },
    );
    state.add_player(PlayerState::new(WHITE)).unwrap();
    state.add_player(PlayerState::new(BLACK)).unwrap();
    state.set_active_players(vec![WHITE]).unwrap();
    let rows = size / 2 - 1;
    let mut next = 1;
    for y in 0..size {
        for x in 0..size {
            // a1 is dark; the long diagonal starts at each player's left.
            if (x + y) % 2 != 0 || (rows..size - rows).contains(&y) {
                continue;
            }
            let owner = if y < rows { WHITE } else { BLACK };
            state
                .add_entity(EntityState::new(
                    EntityId::new(next),
                    CHECKER,
                    owner,
                    Position::new(x, y),
                ))
                .unwrap();
            next += 1;
        }
    }
    state
}

pub fn is_king(entity: &EntityState) -> bool {
    entity
        .state
        .get(KING)
        .and_then(StateValue::as_bool)
        .unwrap_or(false)
}

fn promotion_row(state: &GameState, entity: &EntityState) -> bool {
    entity.position.y
        == if entity.owner == WHITE {
            state.board.height() - 1
        } else {
            0
        }
}

pub struct CheckerRule;
impl EntityRule for CheckerRule {
    fn presentation(
        &self,
        context: EntityRuleContext<'_>,
    ) -> Result<EntityPresentation, RuleError> {
        let entity = context.entity();
        let side = if entity.owner == WHITE {
            "white"
        } else {
            "black"
        };
        let rank = if is_king(entity) { "king" } else { "man" };
        Ok(EntityPresentation::new(format!("checkers/{side}/{rank}"))
            .with_label(format!("{side} {rank}")))
    }
}
pub fn registry() -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    registry
        .register_entity(CHECKER, CheckerRule)
        .expect("one checker entity rule");
    registry.register_outcome(CheckersOutcomeRule);
    registry
}

#[cfg(test)]
mod tests;
