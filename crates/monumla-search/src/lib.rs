//! Game-independent search over complete actions, with bounded memory and work.
//! The runtime module searches the ordinary core rules and interaction protocol.
//! The low-level SearchDomain trait also supports abstract trees and other hosts.
#![forbid(unsafe_code)]

pub mod runtime;
mod table;
#[cfg(test)]
mod tests;
use std::collections::HashMap;
use table::{Bound, Entry, Table};

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

/// Bounded reusable work. Keep one memory with one rules/evaluation model.
/// Perspective or quiescence-policy changes invalidate cached scores.
pub struct SearchMemory<A, P> {
    table: Table<A>,
    quiet_table: Table<A>,
    killers: Vec<Vec<A>>,
    history: HashMap<u64, u32>,
    results: HashMap<u128, SearchResult<A>>,
    perspective: Option<P>,
    policy: Option<(u8, usize, bool)>,
    capacity: usize,
}
impl<A: Clone, P> SearchMemory<A, P> {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.min(1_048_576);
        Self {
            table: Table::new(capacity),
            quiet_table: Table::new(capacity / 4),
            killers: vec![Vec::new(); 322],
            history: HashMap::new(),
            results: HashMap::new(),
            perspective: None,
            policy: None,
            capacity,
        }
    }
}
/// Continue completed iterations and reuse partial subtree work after a deadline.
pub fn analyze_cached<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    options: SearchOptions,
    memory: &mut SearchMemory<D::Action, D::Perspective>,
    cancel: impl FnMut() -> bool,
    progress: impl FnMut(&SearchResult<D::Action>),
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    analyze_roots(
        domain,
        position,
        perspective,
        options,
        memory,
        cancel,
        None,
        progress,
    )
}

pub fn search<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    limits: SearchLimits,
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    search_with_control(domain, position, perspective, limits, || false)
}
pub fn search_with_control<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    limits: SearchLimits,
    cancel: impl FnMut() -> bool,
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    analyze(
        domain,
        position,
        perspective,
        SearchOptions {
            limits,
            ..SearchOptions::default()
        },
        cancel,
        |_| {},
    )
}

/// Reports each completed depth. Cancellation and budgets cover all iterations,
/// aspiration re-searches, PVS probes, and quiescence. Interrupted iterations
/// never replace a completed result. Hosts supply deadlines through `cancel`.
pub fn analyze<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    options: SearchOptions,
    cancel: impl FnMut() -> bool,
    progress: impl FnMut(&SearchResult<D::Action>),
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    analyze_roots(
        domain,
        position,
        perspective,
        options,
        &mut SearchMemory::new(options.transposition_entries),
        cancel,
        None,
        progress,
    )
}

/// Evaluate a played candidate with the same search and horizon as the best move.
pub fn analyze_action<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    action: D::Action,
    mut options: SearchOptions,
    cancel: impl FnMut() -> bool,
    progress: impl FnMut(&SearchResult<D::Action>),
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    options.multi_pv = 1;
    analyze_roots(
        domain,
        position,
        perspective,
        options,
        &mut SearchMemory::new(options.transposition_entries),
        cancel,
        Some(vec![action]),
        progress,
    )
}

#[allow(clippy::too_many_arguments)] // One internal entry point for both ordinary and restricted-root search.
fn analyze_roots<D: SearchDomain>(
    domain: &D,
    position: &D::Position,
    perspective: D::Perspective,
    options: SearchOptions,
    memory: &mut SearchMemory<D::Action, D::Perspective>,
    cancel: impl FnMut() -> bool,
    allowed_root: Option<Vec<D::Action>>,
    mut progress: impl FnMut(&SearchResult<D::Action>),
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    if options.limits.max_depth > 256 {
        return Err(SearchError::InvalidDepth);
    }
    if options.multi_pv == 0
        || options.multi_pv > 8
        || options.quiescence_depth > 64
        || options.transposition_entries > 1_048_576
        || options.aspiration_window < 0
    {
        return Err(SearchError::InvalidOptions);
    }
    if memory.perspective != Some(perspective)
        || memory.policy
            != Some((
                options.quiescence_depth,
                options.multi_pv,
                options.late_move_reductions,
            ))
        || memory.capacity != options.transposition_entries
    {
        *memory = SearchMemory::new(options.transposition_entries);
        memory.perspective = Some(perspective);
        memory.policy = Some((
            options.quiescence_depth,
            options.multi_pv,
            options.late_move_reductions,
        ));
    }
    let root_key = if allowed_root.is_none() {
        domain.position_key(position)
    } else {
        None
    };
    let retained = root_key
        .and_then(|key| memory.results.get(&key))
        .filter(|r| r.completed_depth <= options.limits.max_depth)
        .cloned();
    // No evaluation is required when a bot has no decision to make. Terminal
    // positions still follow the normal outcome path, never an arbitrary fallback.
    if options.stop_on_forced_move
        && domain
            .terminal_score(position, perspective)
            .map_err(SearchError::Domain)?
            .is_none()
    {
        let actions = domain.actions(position).map_err(SearchError::Domain)?;
        if actions.len() == 1 {
            return Ok(SearchResult {
                best_action: actions.first().cloned(),
                score: None,
                principal_variation: actions,
                completed_depth: 0,
                nodes: 0,
                stop_reason: StopReason::ForcedMove,
                statistics: SearchStatistics::default(),
                variations: Vec::new(),
            });
        }
    }
    domain.prepare(position).map_err(SearchError::Domain)?;
    let mut context = Context {
        domain,
        perspective,
        options,
        cancel,
        nodes: 0,
        statistics: SearchStatistics::default(),
        memory,
        generation: 0,
        excluded_root: Vec::new(),
        allowed_root,
    };
    let mut result = SearchResult {
        best_action: None,
        score: None,
        principal_variation: Vec::new(),
        completed_depth: 0,
        nodes: 0,
        stop_reason: StopReason::DepthLimit,
        statistics: SearchStatistics::default(),
        variations: Vec::new(),
    };
    if let Some(previous) = retained {
        result = previous;
    }
    let carried_nodes = result.nodes;
    let carried_statistics = result.statistics;
    let start = if result.score.is_some() {
        result.completed_depth.saturating_add(1)
    } else {
        u16::from(options.limits.max_depth != 0)
    };
    'iterations: for depth in start..=options.limits.max_depth {
        context.generation = depth;
        let mut width = i64::from(options.aspiration_window);
        let center = result.score;
        let line = loop {
            let (alpha, beta) = match center {
                Some(center) if width > 0 && width < i64::from(u32::MAX) => (
                    (i64::from(center) - width).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        as i32,
                    (i64::from(center) + width).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        as i32,
                ),
                _ => (i32::MIN, i32::MAX),
            };
            match context.visit(position, depth, 0, alpha, beta, &result.principal_variation) {
                Ok(line) => {
                    let fail = (line.score <= alpha && alpha != i32::MIN)
                        || (line.score >= beta && beta != i32::MAX);
                    if fail {
                        width = (width * 4).max(1);
                        context.statistics.researches += 1;
                        continue;
                    }
                    break line;
                }
                Err(Abort::Stopped(reason)) => {
                    result.stop_reason = reason;
                    break 'iterations;
                }
                Err(Abort::Error(error)) => return Err(error),
            }
        };
        let mut variations = Vec::new();
        if !line.actions.is_empty() {
            variations.push(SearchVariation {
                score: line.score,
                actions: line.actions.clone(),
            });
        }
        let mut interrupted = None;
        if depth > 0 && !line.terminal {
            for _ in 1..options.multi_pv {
                if let Some(first) = variations.last().and_then(|v| v.actions.first()) {
                    context.excluded_root.push(first.clone());
                }
                match context.visit(position, depth, 0, i32::MIN, i32::MAX, &[]) {
                    Ok(next) => variations.push(SearchVariation {
                        score: next.score,
                        actions: next.actions,
                    }),
                    Err(Abort::Stopped(reason)) => {
                        interrupted = Some(reason);
                        break;
                    }
                    Err(Abort::Error(SearchError::NoActions)) => break,
                    Err(Abort::Error(error)) => return Err(error),
                }
            }
        }
        context.excluded_root.clear();
        if let Some(reason) = interrupted {
            result.stop_reason = reason;
            break 'iterations;
        }
        result.variations = variations;
        result.completed_depth = depth;
        result.best_action = line.actions.first().cloned();
        result.score = Some(line.score);
        result.principal_variation = line.actions;
        result.nodes = context.nodes;
        result.statistics = context.statistics;
        if line.terminal {
            result.stop_reason = StopReason::Terminal;
        }
        progress(&result);
        if line.terminal {
            break;
        }
    }
    result.nodes = carried_nodes.saturating_add(context.nodes);
    result.statistics = SearchStatistics {
        quiescence_nodes: carried_statistics.quiescence_nodes + context.statistics.quiescence_nodes,
        transposition_hits: carried_statistics.transposition_hits
            + context.statistics.transposition_hits,
        cutoffs: carried_statistics.cutoffs + context.statistics.cutoffs,
        researches: carried_statistics.researches + context.statistics.researches,
    };
    if let Some(key) = root_key {
        if context.memory.results.len() >= 32 && !context.memory.results.contains_key(&key) {
            context.memory.results.clear();
        }
        context.memory.results.insert(key, result.clone());
    }
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
struct Context<'a, D: SearchDomain, F> {
    domain: &'a D,
    perspective: D::Perspective,
    options: SearchOptions,
    cancel: F,
    nodes: u64,
    statistics: SearchStatistics,
    memory: &'a mut SearchMemory<D::Action, D::Perspective>,
    generation: u16,
    excluded_root: Vec<D::Action>,
    allowed_root: Option<Vec<D::Action>>,
}
impl<D: SearchDomain, F: FnMut() -> bool> Context<'_, D, F> {
    fn tick(&mut self) -> Result<(), Abort<D::Error>> {
        if (self.cancel)() {
            return Err(Abort::Stopped(StopReason::Cancelled));
        }
        if self
            .options
            .limits
            .max_nodes
            .is_some_and(|n| self.nodes >= n)
        {
            return Err(Abort::Stopped(StopReason::NodeLimit));
        }
        self.nodes += 1;
        Ok(())
    }
    fn order(
        &self,
        position: &D::Position,
        actions: &mut [D::Action],
        preferred: Option<&D::Action>,
        cached: Option<&D::Action>,
        ply: u16,
    ) {
        actions.sort_by_cached_key(|action| {
            let priority = self.domain.action_priority(position, action);
            let killer = self.memory.killers[usize::from(ply)]
                .iter()
                .position(|a| a == action)
                .map_or(0, |i| 2 - i);
            let history = self
                .domain
                .action_key(action)
                .and_then(|key| self.memory.history.get(&key))
                .copied()
                .unwrap_or(0);
            std::cmp::Reverse((
                preferred == Some(action),
                cached == Some(action),
                priority,
                killer,
                history,
            ))
        });
    }
    fn record_cutoff(&mut self, action: &D::Action, ply: u16, depth: u16) {
        self.statistics.cutoffs += 1;
        let killers = &mut self.memory.killers[usize::from(ply)];
        if !killers.contains(action) {
            killers.insert(0, action.clone());
            killers.truncate(2);
        }
        if let Some(key) = self.domain.action_key(action) {
            if self.memory.history.len() >= 8192 && !self.memory.history.contains_key(&key) {
                self.memory.history.clear();
            }
            let count = self.memory.history.entry(key).or_default();
            *count = count.saturating_add(u32::from(depth).pow(2));
        }
    }
    fn visit(
        &mut self,
        position: &D::Position,
        depth: u16,
        ply: u16,
        mut alpha: i32,
        mut beta: i32,
        preferred: &[D::Action],
    ) -> Result<Line<D::Action>, Abort<D::Error>> {
        if depth == 0 {
            return self.quiet(position, self.options.quiescence_depth, ply, alpha, beta);
        }
        self.tick()?;
        if let Some(score) = self
            .domain
            .terminal_score_at_ply(position, self.perspective, ply)?
        {
            return Ok(Line {
                score,
                actions: Vec::new(),
                terminal: true,
            });
        }
        let key = if ply == 0 && (!self.excluded_root.is_empty() || self.allowed_root.is_some()) {
            None
        } else {
            self.domain.position_key(position)
        };
        let cached = key.and_then(|key| self.memory.table.get(key)).cloned();
        if let Some(entry) = &cached {
            self.statistics.transposition_hits += 1;
            if entry.depth >= depth {
                let score = self.domain.score_from_cache(entry.score, ply);
                let usable = match entry.bound {
                    Bound::Exact => true,
                    Bound::Lower => score >= beta,
                    Bound::Upper => score <= alpha,
                };
                if usable {
                    return Ok(Line {
                        score,
                        actions: entry.line.clone(),
                        terminal: false,
                    });
                }
            }
        }
        let role = self.domain.role(position, self.perspective)?;
        let mut actions = self.domain.actions(position)?;
        if ply == 0 {
            actions.retain(|a| {
                !self.excluded_root.contains(a)
                    && self
                        .allowed_root
                        .as_ref()
                        .map_or(true, |allowed| allowed.contains(a))
            });
        }
        if actions.is_empty() {
            return Err(Abort::Error(SearchError::NoActions));
        }
        self.order(
            position,
            &mut actions,
            preferred.first(),
            cached.as_ref().and_then(|e| e.line.first()),
            ply,
        );
        let original_alpha = alpha;
        let original_beta = beta;
        let forced = actions.len() == 1;
        let mut best: Option<Line<D::Action>> = None;
        for (index, action) in actions.into_iter().enumerate() {
            let child = self.domain.play(position, &action)?;
            let follow = if preferred.first() == Some(&action) {
                &preferred[1..]
            } else {
                &[]
            };
            let probe = self.options.principal_variation_search
                && best.is_some()
                && i64::from(beta) - i64::from(alpha) > 1;
            let (a, b) = if probe {
                match role {
                    NodeRole::Maximize => (alpha, alpha.saturating_add(1)),
                    NodeRole::Minimize => (beta.saturating_sub(1), beta),
                }
            } else {
                (alpha, beta)
            };
            let reduced = self.options.late_move_reductions
                && ply > 0
                && depth >= 3
                && index >= 4
                && !forced
                && follow.is_empty()
                && i64::from(b) - i64::from(a) == 1
                && self.domain.can_reduce(position, &action);
            let mut line = self.visit(
                &child,
                depth - 1 - u16::from(reduced),
                ply + 1,
                a,
                b,
                follow,
            )?;
            let improves_bound = match role {
                NodeRole::Maximize => line.score > alpha,
                NodeRole::Minimize => line.score < beta,
            };
            if reduced && improves_bound {
                self.statistics.researches += 1;
                line = self.visit(&child, depth - 1, ply + 1, a, b, follow)?;
            }
            if probe && line.score > alpha && line.score < beta {
                self.statistics.researches += 1;
                line = self.visit(&child, depth - 1, ply + 1, alpha, beta, follow)?;
            }
            let improves = best.as_ref().map_or(true, |best| match role {
                NodeRole::Maximize => line.score > best.score,
                NodeRole::Minimize => line.score < best.score,
            });
            if improves {
                let mut pv = vec![action.clone()];
                pv.extend(line.actions);
                best = Some(Line {
                    score: line.score,
                    actions: pv,
                    terminal: false,
                });
            }
            let score = best.as_ref().expect("one action evaluated").score;
            match role {
                NodeRole::Maximize => alpha = alpha.max(score),
                NodeRole::Minimize => beta = beta.min(score),
            }
            if alpha >= beta {
                self.record_cutoff(&action, ply, depth);
                break;
            }
        }
        let best = best.expect("nonempty actions");
        if let Some(key) = key {
            let bound = if best.score <= original_alpha {
                Bound::Upper
            } else if best.score >= original_beta {
                Bound::Lower
            } else {
                Bound::Exact
            };
            self.memory.table.store(Entry {
                key,
                depth,
                score: self.domain.score_to_cache(best.score, ply),
                bound,
                line: best.actions.clone(),
                generation: self.generation,
            });
        }
        Ok(best)
    }
    fn quiet(
        &mut self,
        position: &D::Position,
        depth: u8,
        ply: u16,
        mut alpha: i32,
        mut beta: i32,
    ) -> Result<Line<D::Action>, Abort<D::Error>> {
        self.tick()?;
        self.statistics.quiescence_nodes += 1;
        if let Some(score) = self
            .domain
            .terminal_score_at_ply(position, self.perspective, ply)?
        {
            return Ok(Line {
                score,
                actions: Vec::new(),
                terminal: true,
            });
        }
        let quiet_key = self
            .domain
            .position_key(position)
            .map(|key| fingerprint(&(key, depth)));
        if let Some(entry) = quiet_key.and_then(|key| self.memory.quiet_table.get(key)) {
            let score = self.domain.score_from_cache(entry.score, ply);
            if entry.depth == u16::from(depth)
                && match entry.bound {
                    Bound::Exact => true,
                    Bound::Lower => score >= beta,
                    Bound::Upper => score <= alpha,
                }
            {
                self.statistics.transposition_hits += 1;
                return Ok(Line {
                    score,
                    actions: entry.line.clone(),
                    terminal: false,
                });
            }
        }
        let original_alpha = alpha;
        let original_beta = beta;
        let evaluation = self.domain.evaluate(position, self.perspective)?;
        if depth > 0 {
            let cutoff = match self.domain.role(position, self.perspective)? {
                NodeRole::Maximize => evaluation >= beta,
                NodeRole::Minimize => evaluation <= alpha,
            };
            if cutoff && self.domain.allows_stand_pat(position)? == Some(true) {
                self.statistics.cutoffs += 1;
                return Ok(Line {
                    score: evaluation,
                    actions: Vec::new(),
                    terminal: false,
                });
            }
        }
        let Some(mut tactical) = (if depth > 0 {
            self.domain.quiescence(position)?
        } else {
            None
        }) else {
            return Ok(Line {
                score: evaluation,
                actions: Vec::new(),
                terminal: false,
            });
        };
        let role = self.domain.role(position, self.perspective)?;
        let mut best = if tactical.stand_pat {
            Some(Line {
                score: evaluation,
                actions: Vec::new(),
                terminal: false,
            })
        } else {
            None
        };
        if tactical.stand_pat {
            match role {
                NodeRole::Maximize => alpha = alpha.max(evaluation),
                NodeRole::Minimize => beta = beta.min(evaluation),
            }
            if alpha >= beta {
                self.statistics.cutoffs += 1;
                return Ok(best.unwrap());
            }
        } else if tactical.actions.is_empty() {
            return Err(Abort::Error(SearchError::NoActions));
        }
        self.order(position, &mut tactical.actions, None, None, ply);
        for action in tactical.actions {
            let child = self.domain.play(position, &action)?;
            let line = self.quiet(&child, depth - 1, ply + 1, alpha, beta)?;
            let improves = best.as_ref().map_or(true, |best| match role {
                NodeRole::Maximize => line.score > best.score,
                NodeRole::Minimize => line.score < best.score,
            });
            if improves {
                let mut actions = vec![action];
                actions.extend(line.actions);
                best = Some(Line {
                    score: line.score,
                    actions,
                    terminal: false,
                });
            }
            let score = best.as_ref().unwrap().score;
            match role {
                NodeRole::Maximize => alpha = alpha.max(score),
                NodeRole::Minimize => beta = beta.min(score),
            }
            if alpha >= beta {
                self.statistics.cutoffs += 1;
                break;
            }
        }
        let best = best.expect("standing pat or tactical response");
        if let Some(key) = quiet_key {
            self.memory.quiet_table.store(Entry {
                key,
                depth: u16::from(depth),
                score: self.domain.score_to_cache(best.score, ply),
                bound: if best.score <= original_alpha {
                    Bound::Upper
                } else if best.score >= original_beta {
                    Bound::Lower
                } else {
                    Bound::Exact
                },
                line: best.actions.clone(),
                generation: self.generation,
            });
        }
        Ok(best)
    }
}

pub use monumla_core::fingerprint;
