//! Provisional root updates are separate from completed iterations. They never
//! become a bot decision or an equal-depth review result.
use super::*;
impl<D: SearchDomain, F: FnMut() -> bool> Context<'_, D, F> {
    pub(super) fn total_statistics(&self) -> SearchStatistics {
        SearchStatistics {
            selective_depth: self
                .base_statistics
                .selective_depth
                .max(self.statistics.selective_depth),
            samples: self.base_statistics.samples + self.statistics.samples,
            quiescence_nodes: self.base_statistics.quiescence_nodes
                + self.statistics.quiescence_nodes,
            transposition_hits: self.base_statistics.transposition_hits
                + self.statistics.transposition_hits,
            cutoffs: self.base_statistics.cutoffs + self.statistics.cutoffs,
            researches: self.base_statistics.researches + self.statistics.researches,
        }
    }
    pub(super) fn notify(&mut self) {
        if !self.options.report_partial {
            return;
        }
        let mut update = self.completed.clone();
        update.iteration = self.live.clone();
        update.nodes = self.base_nodes + self.nodes;
        update.statistics = self.total_statistics();
        (self.progress)(&update);
    }
    pub(super) fn root_started(
        &mut self,
        depth: u16,
        total: usize,
        index: usize,
        action: &D::Action,
    ) {
        if !self.options.report_partial {
            return;
        }
        if self.excluded_root.is_empty() {
            if self.live.as_ref().map_or(true, |v| v.depth != depth) {
                self.live = Some(IterationProgress {
                    depth,
                    root_done: 0,
                    root_total: total,
                    line_index: 1,
                    current_action: None,
                    score: None,
                    bound: ScoreBound::Estimate,
                    principal_variation: Vec::new(),
                });
            }
            let live = self.live.as_mut().unwrap();
            live.root_done = index;
            live.root_total = total;
            live.current_action = Some(action.clone());
        } else if let Some(live) = &mut self.live {
            live.line_index = self.excluded_root.len() + 1;
            live.current_action = Some(action.clone());
        }
        if index == 0 {
            self.notify();
        }
    }
    pub(super) fn root_result(
        &mut self,
        depth: u16,
        done: usize,
        total: usize,
        line: &Line<D::Action>,
        bound: ScoreBound,
    ) {
        if !self.options.report_partial || !self.excluded_root.is_empty() {
            return;
        }
        let current_action = self.live.as_ref().and_then(|v| v.current_action.clone());
        self.live = Some(IterationProgress {
            depth,
            root_done: done,
            root_total: total,
            line_index: 1,
            current_action,
            score: Some(line.score),
            bound,
            principal_variation: line.actions.clone(),
        });
        self.notify();
    }
}
