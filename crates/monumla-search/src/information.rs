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
struct SampleBatch<A, P> {
    world: P,
    actions: Vec<A>,
    values: Vec<i32>,
}
pub struct InformationMemory<A, P> {
    root: Option<u128>,
    batch: Option<SampleBatch<A, P>>,
    samples: u64,
    nodes: u64,
    actions: Vec<A>,
    sums: Vec<i64>,
    batches: Vec<Vec<i64>>,
    partial: Vec<i64>,
}
impl<A, P> Default for InformationMemory<A, P> {
    fn default() -> Self {
        Self::new()
    }
}
impl<A, P> InformationMemory<A, P> {
    pub fn new() -> Self {
        Self {
            root: None,
            batch: None,
            samples: 0,
            nodes: 0,
            actions: Vec::new(),
            sums: Vec::new(),
            batches: Vec::new(),
            partial: Vec::new(),
        }
    }
}
/// Approximate uncertainty from blocks, rather than treating consecutive
/// correlated world samples as independent observations. Not a rating or proof.
pub struct SampleComparison {
    pub best_score: i32,
    pub played_score: i32,
    pub margin: i32,
    pub samples: u64,
    pub standard_error: f64,
    pub is_best: bool,
}
impl<A: PartialEq, P> InformationMemory<A, P> {
    pub fn compare(&self, action: &A) -> Option<SampleComparison> {
        if self.samples == 0 {
            return None;
        }
        let played = self.actions.iter().position(|a| a == action)?;
        let best = self
            .sums
            .iter()
            .enumerate()
            .min_by_key(|(i, s)| (std::cmp::Reverse(**s), *i))?
            .0;
        let best_score = (self.sums[best] / self.samples.max(1) as i64) as i32;
        let played_score = (self.sums[played] / self.samples.max(1) as i64) as i32;
        let other = self
            .sums
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != best)
            .map(|(_, s)| *s / self.samples.max(1) as i64)
            .max()
            .unwrap_or(i64::from(best_score));
        let compare = if played == best {
            self.sums
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != best)
                .max_by_key(|(_, s)| *s)
                .map(|(i, _)| i)
                .unwrap_or(best)
        } else {
            played
        };
        let diffs = self
            .batches
            .iter()
            .map(|b| (b[best] - b[compare]) as f64 / 16.0)
            .collect::<Vec<_>>();
        let standard_error = if diffs.len() < 8 {
            f64::INFINITY
        } else {
            let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
            (diffs.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                / (diffs.len() * (diffs.len() - 1)) as f64)
                .sqrt()
        };
        Some(SampleComparison {
            best_score,
            played_score,
            margin: best_score.saturating_sub(other as i32),
            samples: self.samples,
            standard_error,
            is_best: played == best,
        })
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
    memory: &mut InformationMemory<D::Action, D::Position>,
    cancel: impl FnMut() -> bool,
) -> Result<SearchResult<D::Action>, SearchError<D::Error>> {
    analyze_information_with_progress(domain, position, viewer, options, memory, cancel, |_| {})
}

pub fn analyze_information_with_progress<D: InformationDomain>(
    domain: &D,
    position: &D::Position,
    viewer: D::Perspective,
    options: SearchOptions,
    memory: &mut InformationMemory<D::Action, D::Position>,
    mut cancel: impl FnMut() -> bool,
    mut progress: impl FnMut(&SearchResult<D::Action>),
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
            iteration: None,
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
    'batches: loop {
        if cancel() {
            break;
        }
        if memory.nodes - initial_nodes >= options.limits.max_nodes.unwrap_or(10_000) {
            stop = StopReason::NodeLimit;
            break;
        }
        let mut batch = if let Some(batch) = memory.batch.take() {
            batch
        } else {
            let world = domain
                .sample(&observation, viewer, memory.samples.wrapping_add(1))
                .map_err(SearchError::Domain)?;
            let mut actions = domain.actions(&world).map_err(SearchError::Domain)?;
            if actions.is_empty() {
                return Err(SearchError::NoActions);
            }
            if memory.actions.is_empty() {
                memory.actions = actions.clone();
                memory.sums = vec![0; actions.len()];
                memory.partial = vec![0; actions.len()];
            }
            if actions.len() != memory.actions.len() {
                return Err(SearchError::InvalidOptions);
            }
            if actions != memory.actions {
                // Match a bijection, not just membership; reject duplicate/missing inputs.
                let mut ordered = Vec::with_capacity(actions.len());
                for stored in &memory.actions {
                    let index = actions
                        .iter()
                        .position(|a| a == stored)
                        .ok_or(SearchError::InvalidOptions)?;
                    ordered.push(actions.swap_remove(index));
                }
                actions = ordered;
            }
            SampleBatch {
                world,
                actions,
                values: Vec::with_capacity(memory.actions.len()),
            }
        };
        for action in batch.actions.iter().skip(batch.values.len()) {
            let limited =
                memory.nodes - initial_nodes >= options.limits.max_nodes.unwrap_or(10_000);
            if cancel() || limited {
                if limited {
                    stop = StopReason::NodeLimit;
                }
                memory.batch = Some(batch);
                break 'batches;
            }
            let child = domain
                .play(&batch.world, action)
                .map_err(SearchError::Domain)?;
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
            batch.values.push(value);
            if options.report_partial && memory.samples > 0 && memory.nodes & 255 == 0 {
                progress(&sample_report(memory, options, StopReason::Cancelled));
            }
        }
        let values = batch.values;
        for ((sum, partial), value) in memory.sums.iter_mut().zip(&mut memory.partial).zip(values) {
            *sum += i64::from(value);
            *partial += i64::from(value);
        }
        memory.samples += 1;
        if memory.samples % 16 == 0 {
            if memory.batches.len() == 128 {
                memory.batches.remove(0);
            }
            memory.batches.push(memory.partial.clone());
            memory.partial.fill(0);
        }
        if options.report_partial {
            progress(&sample_report(memory, options, StopReason::Cancelled));
        }
        // Setup/forced decisions also use this path; do not invent hidden foresight.
        if options.stop_on_forced_move && memory.actions.len() == 1 {
            stop = StopReason::ForcedMove;
            break;
        }
    }
    Ok(sample_report(memory, options, stop))
}

fn sample_report<A: Clone, P>(
    memory: &InformationMemory<A, P>,
    options: SearchOptions,
    stop: StopReason,
) -> SearchResult<A> {
    let mut ranked = memory
        .actions
        .iter()
        .cloned()
        .zip(&memory.sums)
        .filter(|_| memory.samples > 0)
        .map(|(action, sum)| SearchVariation {
            score: (sum / memory.samples.max(1) as i64) as i32,
            actions: vec![action],
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|v| std::cmp::Reverse(v.score));
    ranked.truncate(options.multi_pv.clamp(1, 8));
    let first = ranked.first();
    SearchResult {
        iteration: None,
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
    }
}

#[cfg(test)]
#[path = "information_tests.rs"]
mod tests;
