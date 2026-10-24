//! Visibility and legal hidden-state completion belong to the ruleset.
use crate::{GameState, PlayerId, RuleError};

/// Projects a position to information available to one player, and completes
/// that observation with a possible world consistent with its public facts.
/// Sampling must depend only on the observation and supplied seed.
pub trait InformationRule {
    fn observe(&self, state: &GameState, viewer: PlayerId) -> Result<GameState, RuleError>;
    fn sample(
        &self,
        observation: &GameState,
        viewer: PlayerId,
        seed: u64,
    ) -> Result<GameState, RuleError>;
}
