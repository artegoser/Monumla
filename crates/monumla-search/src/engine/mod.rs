//! Iterative deepening, alpha-beta/PVS, quiescence and controlled reductions.
use crate::{
    table::{Bound, Entry},
    *,
};

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
                iteration: None,
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
    let mut result = SearchResult {
        iteration: None,
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
    let mut context = Context {
        completed: result.clone(),
        live: None,
        progress: &mut progress,
        base_nodes: carried_nodes,
        base_statistics: carried_statistics,
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
    'iterations: for depth in start..=options.limits.max_depth {
        context.generation = depth;
        context.completed = result.clone();
        let mut width = i64::from(options.aspiration_window);
        let center = result.score;
        let saved = root_key
            .and_then(|key| context.memory.partial_iterations.get(&key))
            .filter(|(d, _)| *d == depth)
            .map(|(_, v)| v.clone());
        let line = if let Some(saved) = &saved {
            let primary = &saved[0];
            let line = Line {
                score: primary.score,
                actions: primary.actions.clone(),
                terminal: false,
            };
            context.root_result(depth, 0, 0, &line, ScoreBound::Exact);
            line
        } else {
            loop {
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
            }
        };
        let mut variations = saved.unwrap_or_default();
        if variations.is_empty() && !line.actions.is_empty() {
            variations.push(SearchVariation {
                score: line.score,
                actions: line.actions.clone(),
            });
        }
        let mut interrupted = None;
        if depth > 0 && !line.terminal {
            context.excluded_root = variations
                .iter()
                .filter_map(|v| v.actions.first().cloned())
                .collect();
            for _ in variations.len()..options.multi_pv {
                match context.visit(position, depth, 0, i32::MIN, i32::MAX, &[]) {
                    Ok(next) => {
                        if let Some(first) = next.actions.first() {
                            context.excluded_root.push(first.clone());
                        }
                        variations.push(SearchVariation {
                            score: next.score,
                            actions: next.actions,
                        });
                    }
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
            if let Some(key) = root_key {
                if context.memory.partial_iterations.len() >= 32 {
                    context.memory.partial_iterations.clear();
                }
                context
                    .memory
                    .partial_iterations
                    .insert(key, (depth, variations));
            }
            result.stop_reason = reason;
            break 'iterations;
        }
        if let Some(key) = root_key {
            context.memory.partial_iterations.remove(&key);
        }
        result.variations = variations;
        result.completed_depth = depth;
        result.best_action = line.actions.first().cloned();
        result.score = Some(line.score);
        result.principal_variation = line.actions;
        result.nodes = carried_nodes.saturating_add(context.nodes);
        result.statistics = context.total_statistics();
        if line.terminal {
            result.stop_reason = StopReason::Terminal;
        }
        result.iteration = None;
        (context.progress)(&result);
        if line.terminal {
            break;
        }
    }
    result.nodes = carried_nodes.saturating_add(context.nodes);
    result.statistics = context.total_statistics();
    result.iteration = context
        .live
        .clone()
        .filter(|live| live.depth > result.completed_depth);
    if let Some(key) = root_key {
        if context.memory.results.len() >= 32 && !context.memory.results.contains_key(&key) {
            context.memory.results.clear();
        }
        let mut completed = result.clone();
        completed.iteration = None;
        context.memory.results.insert(key, completed);
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
    completed: SearchResult<D::Action>,
    live: Option<IterationProgress<D::Action>>,
    progress: &'a mut dyn FnMut(&SearchResult<D::Action>),
    base_nodes: u64,
    base_statistics: SearchStatistics,
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

mod progress;
mod quiescence;
mod traversal;
