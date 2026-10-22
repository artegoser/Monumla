//! Game-independent, deterministic alpha-beta search over complete actions.
//!
//! The adapter owns legality, transitions, history, evaluation, and terminal
//! utility. Search has no board, entity, piece, or alternating-player assumptions.
//! This first algorithm targets perfect-information games with two opposed
//! objectives. Other game models can use the same domain boundary with a future
//! algorithm; chance and simultaneous moves are not interpreted by alpha-beta.

#[cfg(test)]
mod tests;

/// Whether a node optimizes for or against the caller's fixed perspective.
/// Explicit roles support extra turns and multiple players on opposing teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRole {
    Maximize,
    Minimize,
}

pub trait SearchDomain {
    type Position;
    type Action: Clone + PartialEq;
    type Perspective: Copy;
    type Error;

    /// Return terminal utility from the fixed perspective, including draws.
    fn terminal_score(
        &self,
        position: &Self::Position,
        perspective: Self::Perspective,
    ) -> Result<Option<i32>, Self::Error>;
    /// Static utility from the same perspective, never from the side to move.
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
    /// Complete legal actions, optionally ordered by the game for better pruning.
    /// Selection clicks and forced substeps must not count as separate turns.
    fn actions(&self, position: &Self::Position) -> Result<Vec<Self::Action>, Self::Error>;
    /// An independent successor, preserving any history needed by the rules.
    fn play(
        &self,
        position: &Self::Position,
        action: &Self::Action,
    ) -> Result<Self::Position, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchLimits {
    /// Complete actions per path, not interaction substeps. Zero evaluates only.
    pub max_depth: u16,
    /// Total visited nodes across all iterations. None means no node cap.
    pub max_nodes: Option<u64>,
}
impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_nodes: Some(100_000),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    DepthLimit,
    NodeLimit,
    Cancelled,
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult<A> {
    pub best_action: Option<A>,
    /// Absent when interrupted before any iteration completed.
    pub score: Option<i32>,
    pub principal_variation: Vec<A>,
    pub completed_depth: u16,
    pub nodes: u64,
    pub stop_reason: StopReason,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchError<E> {
    #[error("search adapter failed: {0}")]
    Domain(E),
    #[error("maximum search depth must not exceed 256")]
    InvalidDepth,
    #[error("adapter returned no legal actions for a nonterminal position")]
    NoActions,
}

/// Iterative deepening with deterministic tie-breaking. An interrupted iteration
/// never replaces the result of the last fully completed iteration.
pub fn search<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    limits: SearchLimits,
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    search_with_control(domain, position, perspective, limits, || false)
}

/// `cancel` is checked before every visited node. Hosts can implement deadlines
/// or external cancellation here without imposing a native clock on WASM.
pub fn search_with_control<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    limits: SearchLimits,
    cancel: impl FnMut() -> bool,
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    if limits.max_depth > 256 {
        return Err(SearchError::InvalidDepth);
    }
    let mut context = SearchContext {
        domain,
        perspective,
        limits,
        cancel,
        nodes: 0,
    };
    let mut result = SearchResult {
        best_action: None,
        score: None,
        principal_variation: Vec::new(),
        completed_depth: 0,
        nodes: 0,
        stop_reason: StopReason::DepthLimit,
    };
    let start_depth = u16::from(limits.max_depth != 0);
    for depth in start_depth..=limits.max_depth {
        match context.visit(
            position,
            depth,
            i32::MIN,
            i32::MAX,
            &result.principal_variation,
        ) {
            Ok(line) => {
                result.completed_depth = depth;
                result.best_action = line.actions.first().cloned();
                result.score = Some(line.score);
                result.principal_variation = line.actions;
                if line.terminal {
                    result.stop_reason = StopReason::Terminal;
                    break;
                }
            }
            Err(Abort::Stopped(reason)) => {
                result.stop_reason = reason;
                break;
            }
            Err(Abort::Error(error)) => return Err(error),
        }
    }
    result.nodes = context.nodes;
    Ok(result)
}

struct Line<A> {
    score: i32,
    actions: Vec<A>,
    terminal: bool,
}
enum Abort<E> {
    Stopped(StopReason),
    Error(SearchError<E>),
}
impl<E> From<E> for Abort<E> {
    fn from(error: E) -> Self {
        Self::Error(SearchError::Domain(error))
    }
}
struct SearchContext<'a, D: SearchDomain, F> {
    domain: &'a D,
    perspective: D::Perspective,
    limits: SearchLimits,
    cancel: F,
    nodes: u64,
}
impl<D: SearchDomain, F: FnMut() -> bool> SearchContext<'_, D, F> {
    fn visit(
        &mut self,
        position: &D::Position,
        depth: u16,
        mut alpha: i32,
        mut beta: i32,
        preferred: &[D::Action],
    ) -> Result<Line<D::Action>, Abort<D::Error>> {
        if (self.cancel)() {
            return Err(Abort::Stopped(StopReason::Cancelled));
        }
        if self
            .limits
            .max_nodes
            .is_some_and(|limit| self.nodes >= limit)
        {
            return Err(Abort::Stopped(StopReason::NodeLimit));
        }
        self.nodes += 1;
        if let Some(score) = self.domain.terminal_score(position, self.perspective)? {
            return Ok(Line {
                score,
                actions: Vec::new(),
                terminal: true,
            });
        }
        if depth == 0 {
            return Ok(Line {
                score: self.domain.evaluate(position, self.perspective)?,
                actions: Vec::new(),
                terminal: false,
            });
        }
        let role = self.domain.role(position, self.perspective)?;
        let mut actions = self.domain.actions(position)?;
        if actions.is_empty() {
            return Err(Abort::Error(SearchError::NoActions));
        }
        if let Some(index) = preferred
            .first()
            .and_then(|first| actions.iter().position(|action| action == first))
        {
            let first = actions.remove(index);
            actions.insert(0, first);
        }
        let mut best: Option<Line<D::Action>> = None;
        for action in actions {
            let child = self.domain.play(position, &action)?;
            let follow = if preferred.first() == Some(&action) {
                &preferred[1..]
            } else {
                &[]
            };
            let line = self.visit(&child, depth - 1, alpha, beta, follow)?;
            let improves = best.as_ref().map_or(true, |best| match role {
                NodeRole::Maximize => line.score > best.score,
                NodeRole::Minimize => line.score < best.score,
            });
            if improves {
                let mut pv = vec![action];
                pv.extend(line.actions);
                best = Some(Line {
                    score: line.score,
                    actions: pv,
                    terminal: false,
                });
            }
            let score = best.as_ref().expect("at least one action evaluated").score;
            match role {
                NodeRole::Maximize => alpha = alpha.max(score),
                NodeRole::Minimize => beta = beta.min(score),
            }
            if alpha >= beta {
                break;
            }
        }
        Ok(best.expect("nonempty legal actions"))
    }
}
