//! Shared root sampling for games with hidden state. Only observations enter
//! memory; candidate worlds come from the rules' visibility contract.
use crate::*;

pub trait InformationDomain: SearchDomain {
    fn observe(
        &self,
        position: &Self::Position,
        viewer: Self::Perspective,
    ) -> Result<Self::Position, Self::Error>;
    fn sample(
        &self,
        observation: &Self::Position,
        viewer: Self::Perspective,
        seed: u64,
    ) -> Result<Self::Position, Self::Error>;
}
#[derive(Default)]
pub struct InformationMemory<A> {
    root: Option<u128>,
    samples: u64,
    nodes: u64,
    actions: Vec<A>,
    sums: Vec<i64>,
}
impl<A> InformationMemory<A> {
    pub fn new() -> Self {
        Self {
            root: None,
            samples: 0,
            nodes: 0,
            actions: Vec::new(),
            sums: Vec::new(),
        }
    }
}
/// Compare all root actions against the same possible worlds. Completed batches
/// are atomic: cancellation never mixes estimates with different sample counts.
/// This reports a sampled one-step estimate, not a perfect-information depth.
pub fn analyze_information<D: InformationDomain>(
    domain: &D,
    position: &D::Position,
    viewer: D::Perspective,
    options: SearchOptions,
    memory: &mut InformationMemory<D::Action>,
    mut cancel: impl FnMut() -> bool,
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    let observation = domain
        .observe(position, viewer)
        .map_err(SearchError::Domain)?;
    let root = domain.position_key(&observation);
    if root.is_none() || memory.root != root {
        *memory = InformationMemory::new();
        memory.root = root;
    }
    if let Some(score) = domain
        .terminal_score(&observation, viewer)
        .map_err(SearchError::Domain)?
    {
        return Ok(SearchResult {
            best_action: None,
            score: Some(score),
            principal_variation: vec![],
            completed_depth: 0,
            nodes: 0,
            stop_reason: StopReason::Terminal,
            statistics: SearchStatistics::default(),
            variations: vec![],
        });
    }
    let mut stop = StopReason::Cancelled;
    // A per-call work budget allows a persistent worker to resume the same root.
    let initial_nodes = memory.nodes;
    loop {
        if cancel() && memory.samples > 0 {
            break;
        }
        if memory.nodes - initial_nodes >= options.limits.max_nodes.unwrap_or(10_000) {
            stop = StopReason::NodeLimit;
            break;
        }
        let world = domain
            .sample(&observation, viewer, memory.samples.wrapping_add(1))
            .map_err(SearchError::Domain)?;
        let actions = domain.actions(&world).map_err(SearchError::Domain)?;
        if actions.is_empty() {
            return Err(SearchError::NoActions);
        }
        if memory.actions.is_empty() {
            memory.actions = actions.clone();
            memory.sums = vec![0; actions.len()];
        }
        // Visibility must not alter the set or identity of available root inputs.
        if actions.len() != memory.actions.len()
            || actions.iter().any(|a| !memory.actions.contains(a))
        {
            return Err(SearchError::InvalidOptions);
        }
        let mut values = Vec::with_capacity(actions.len());
        let mut interrupted = false;
        for stored in &memory.actions {
            if cancel() && memory.samples > 0 {
                interrupted = true;
                break;
            }
            let action = actions
                .iter()
                .find(|a| *a == stored)
                .expect("checked root input set");
            let child = domain.play(&world, action).map_err(SearchError::Domain)?;
            let visible = domain
                .observe(&child, viewer)
                .map_err(SearchError::Domain)?;
            let value = if let Some(value) = domain
                .terminal_score(&visible, viewer)
                .map_err(SearchError::Domain)?
            {
                value
            } else {
                domain
                    .evaluate(&visible, viewer)
                    .map_err(SearchError::Domain)?
            };
            memory.nodes += 1;
            values.push(value);
        }
        if interrupted {
            break;
        }
        for (sum, value) in memory.sums.iter_mut().zip(values) {
            *sum += i64::from(value);
        }
        memory.samples += 1;
        // Setup/forced decisions also use this path; do not invent hidden foresight.
        if options.stop_on_forced_move && memory.actions.len() == 1 {
            stop = StopReason::ForcedMove;
            break;
        }
    }
    let mut ranked = memory
        .actions
        .iter()
        .cloned()
        .zip(&memory.sums)
        .map(|(action, sum)| SearchVariation {
            score: (sum / memory.samples.max(1) as i64) as i32,
            actions: vec![action],
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|v| std::cmp::Reverse(v.score));
    ranked.truncate(options.multi_pv.clamp(1, 8));
    let first = ranked.first();
    Ok(SearchResult {
        best_action: first.and_then(|v| v.actions.first()).cloned(),
        score: first.map(|v| v.score),
        principal_variation: first.map(|v| v.actions.clone()).unwrap_or_default(),
        completed_depth: u16::from(memory.samples > 0),
        nodes: memory.nodes,
        stop_reason: stop,
        statistics: SearchStatistics {
            samples: memory.samples,
            ..SearchStatistics::default()
        },
        variations: ranked,
    })
}
