//! Optional position knowledge. Legal rules and terminal outcomes remain authoritative.
use crate::{PlayerId, RuleContext, RuleError};

/// Supplies a position estimate without implementing a search algorithm.
/// Scores use a consistent scale and player perspective throughout a game.
/// Terminal outcomes are handled separately by the shared search.
pub trait PositionScoreRule {
    fn score(&self, context: RuleContext<'_>, perspective: PlayerId) -> Result<i32, RuleError>;
}
