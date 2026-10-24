//! Public game-independent contracts, limits and reports.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRole {
    Maximize,
    Minimize,
}

/// Tactical continuations at the nominal horizon. If standing pat is forbidden
/// (e.g. a forced response), every legal response must be included.
pub struct Quiescence<A> {
    pub actions: Vec<A>,
    pub stand_pat: bool,
}

pub trait SearchDomain {
    type Position;
    type Action: Clone + PartialEq;
    type Perspective: Copy + PartialEq;
    type Error;
    fn prepare(&self, _position: &Self::Position) -> Result<(), Self::Error> {
        Ok(())
    }
    fn terminal_score(
        &self,
        position: &Self::Position,
        perspective: Self::Perspective,
    ) -> Result<Option<i32>, Self::Error>;
    fn evaluate(
        &self,
        position: &Self::Position,
        perspective: Self::Perspective,
    ) -> Result<i32, Self::Error>;
    fn role(
        &self,
        position: &Self::Position,
        perspective: Self::Perspective,
    ) -> Result<NodeRole, Self::Error>;
    fn actions(&self, position: &Self::Position) -> Result<Vec<Self::Action>, Self::Error>;
    fn play(
        &self,
        position: &Self::Position,
        action: &Self::Action,
    ) -> Result<Self::Position, Self::Error>;

    /// Must include every rule-relevant fact: turn, draw counters, repetition
    /// context, and other history dependencies. None safely disables caching.
    fn position_key(&self, _position: &Self::Position) -> Option<u128> {
        None
    }
    /// Stable action identity for history ordering. None disables that heuristic.
    fn action_key(&self, _action: &Self::Action) -> Option<u64> {
        None
    }
    fn action_priority(&self, _position: &Self::Position, _action: &Self::Action) -> i32 {
        0
    }
    /// Conservative opt-in for quiet ordinary actions eligible for selective search.
    fn can_reduce(&self, _position: &Self::Position, _action: &Self::Action) -> bool {
        false
    }

    /// Optional fast witness for the domain's quiescence stand-pat policy.
    /// Some(true) must agree with `quiescence().stand_pat` (or with no tactical
    /// continuation). A quiet move alone is insufficient if the domain forbids
    /// standing pat for another reason. None keeps the ordinary path.
    fn allows_stand_pat(&self, _position: &Self::Position) -> Result<Option<bool>, Self::Error> {
        Ok(None)
    }
    fn quiescence(
        &self,
        _position: &Self::Position,
    ) -> Result<Option<Quiescence<Self::Action>>, Self::Error> {
        Ok(None)
    }
    /// Domains may prefer quicker wins/slower losses without teaching the engine
    /// what a win score means. Cache conversion must invert the ply adjustment.
    fn terminal_score_at_ply(
        &self,
        position: &Self::Position,
        perspective: Self::Perspective,
        _ply: u16,
    ) -> Result<Option<i32>, Self::Error> {
        self.terminal_score(position, perspective)
    }
    fn score_to_cache(&self, score: i32, _ply: u16) -> i32 {
        score
    }
    fn score_from_cache(&self, score: i32, _ply: u16) -> i32 {
        score
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchLimits {
    pub max_depth: u16,
    pub max_nodes: Option<u64>,
}
impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_nodes: Some(100_000),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    pub limits: SearchLimits,
    pub quiescence_depth: u8,
    pub transposition_entries: usize,
    pub aspiration_window: i32,
    pub principal_variation_search: bool,
    pub multi_pv: usize,
    /// Bot play can return the only legal complete action without evaluating a choice.
    pub stop_on_forced_move: bool,
    /// Verify promising reduced probes at full depth; never reduce forced or principal actions.
    pub late_move_reductions: bool,
}
impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limits: SearchLimits::default(),
            quiescence_depth: 4,
            transposition_entries: 65_536,
            aspiration_window: 50,
            principal_variation_search: true,
            multi_pv: 1,
            stop_on_forced_move: false,
            late_move_reductions: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    DepthLimit,
    NodeLimit,
    Cancelled,
    Terminal,
    ForcedMove,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchStatistics {
    pub samples: u64,
    pub quiescence_nodes: u64,
    pub transposition_hits: u64,
    pub cutoffs: u64,
    pub researches: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchVariation<A> {
    pub score: i32,
    pub actions: Vec<A>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult<A> {
    pub best_action: Option<A>,
    pub score: Option<i32>,
    pub principal_variation: Vec<A>,
    pub completed_depth: u16,
    pub nodes: u64,
    pub stop_reason: StopReason,
    pub statistics: SearchStatistics,
    pub variations: Vec<SearchVariation<A>>,
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchError<E> {
    #[error("search domain failed: {0}")]
    Domain(E),
    #[error("maximum search depth must not exceed 256")]
    InvalidDepth,
    #[error("invalid search options: quiescence depth <= 64, table <= 1048576 entries, nonnegative aspiration window required")]
    InvalidOptions,
    #[error("rules returned no legal actions for a nonterminal position")]
    NoActions,
}
