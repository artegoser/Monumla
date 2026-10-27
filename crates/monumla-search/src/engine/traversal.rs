//! Alpha-beta/PVS traversal and move ordering.
use super::*;

// Only a resumable root shares its ordering. Interior nodes keep their Vec and
// incur no extra reference-counted allocation.
enum OrderedActions<A> {
    Local(Vec<A>),
    Root(std::sync::Arc<Vec<A>>),
}
impl<A> std::ops::Deref for OrderedActions<A> {
    type Target = [A];
    fn deref(&self) -> &[A] {
        match self {
            Self::Local(v) => v,
            Self::Root(v) => v,
        }
    }
}

impl<D: SearchDomain, F: FnMut() -> bool> Context<'_, D, F> {
    pub(super) fn tick(&mut self) -> Result<(), Abort<D::Error>> {
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
        if self.options.report_partial && self.nodes & 255 == 0 && self.live.is_some() {
            self.notify();
        }
        Ok(())
    }
    pub(super) fn order(
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
    pub(super) fn visit(
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
                    if ply == 0 {
                        let bound = match entry.bound {
                            Bound::Exact => ScoreBound::Exact,
                            Bound::Lower => ScoreBound::Lower,
                            Bound::Upper => ScoreBound::Upper,
                        };
                        self.root_result(
                            depth,
                            0,
                            0,
                            &Line {
                                score,
                                actions: entry.line.clone(),
                                terminal: false,
                            },
                            bound,
                        );
                    }
                    return Ok(Line {
                        score,
                        actions: entry.line.clone(),
                        terminal: false,
                    });
                }
            }
        }
        let role = self.domain.role(position, self.perspective)?;
        let original_alpha = alpha;
        let original_beta = beta;
        let root_key = if ply == 0 {
            self.domain.position_key(position)
        } else {
            None
        };
        let resumed = self
            .memory
            .frontier
            .as_ref()
            .filter(|f| {
                Some(f.key) == root_key
                    && f.depth == depth
                    && f.window == (alpha, beta)
                    && f.excluded == self.excluded_root
                    && f.allowed == self.allowed_root
            })
            .cloned();
        let (actions, next, mut best) = if let Some(frontier) = resumed {
            alpha = frontier.alpha;
            beta = frontier.beta;
            let best = frontier.best.map(|v| Line {
                score: v.score,
                actions: v.actions,
                terminal: false,
            });
            if let Some(line) = &best {
                self.root_result(
                    depth,
                    frontier.next,
                    frontier.actions.len(),
                    line,
                    ScoreBound::Estimate,
                );
            }
            (OrderedActions::Root(frontier.actions), frontier.next, best)
        } else {
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
            (
                if root_key.is_some() {
                    OrderedActions::Root(std::sync::Arc::new(actions))
                } else {
                    OrderedActions::Local(actions)
                },
                0,
                None,
            )
        };
        let forced = actions.len() == 1;
        if ply == 0
            && self.excluded_root.is_empty()
            && self.options.decision_policy == DecisionPolicy::BestAvailable
        {
            self.emergency = best
                .as_ref()
                .and_then(|line| line.actions.first())
                .or_else(|| actions.first())
                .cloned();
        }
        let root_total = actions.len();
        let mut root_done = next;
        for (index, action) in actions.iter().enumerate().skip(next) {
            if ply == 0 {
                self.root_started(depth, root_total, index, action);
            }
            let child = self.domain.play(position, action)?;
            let follow = if preferred.first() == Some(action) {
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
                && self.domain.can_reduce(position, action);
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
            // A completed full-depth probe can prove that this root choice is
            // better even if its subsequent exact-window re-search is stopped.
            // Retain the action for play, without publishing its bound as an
            // exact evaluation or replacing a completed analysis iteration.
            if ply == 0
                && self.excluded_root.is_empty()
                && improves_bound
                && self.options.decision_policy == DecisionPolicy::BestAvailable
            {
                self.emergency = Some(action.clone());
            }
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
                if ply == 0
                    && self.excluded_root.is_empty()
                    && self.options.decision_policy == DecisionPolicy::BestAvailable
                {
                    self.emergency = Some(action.clone());
                }
                let mut pv = vec![action.clone()];
                pv.extend(line.actions);
                best = Some(Line {
                    score: line.score,
                    actions: pv,
                    terminal: false,
                });
            }
            if ply == 0 {
                root_done = index + 1;
                if improves && self.options.report_partial {
                    let line = best.as_ref().unwrap();
                    let bound = match role {
                        NodeRole::Maximize if line.score > original_alpha => ScoreBound::Lower,
                        NodeRole::Minimize if line.score < original_beta => ScoreBound::Upper,
                        _ => ScoreBound::Estimate,
                    };
                    let line = Line {
                        score: line.score,
                        actions: line.actions.clone(),
                        terminal: false,
                    };
                    self.root_result(depth, root_done, root_total, &line, bound);
                }
            }
            let score = best.as_ref().expect("one action evaluated").score;
            match role {
                NodeRole::Maximize => alpha = alpha.max(score),
                NodeRole::Minimize => beta = beta.min(score),
            }
            if let Some(key) = root_key {
                self.memory.frontier = Some(crate::memory::RootFrontier {
                    key,
                    depth,
                    window: (original_alpha, original_beta),
                    excluded: self.excluded_root.clone(),
                    allowed: self.allowed_root.clone(),
                    actions: match &actions {
                        OrderedActions::Root(v) => v.clone(),
                        OrderedActions::Local(_) => unreachable!(),
                    },
                    next: index + 1,
                    best: best.as_ref().map(|v| SearchVariation {
                        score: v.score,
                        actions: v.actions.clone(),
                    }),
                    alpha,
                    beta,
                });
            }
            if alpha >= beta {
                self.record_cutoff(action, ply, depth);
                break;
            }
        }
        let best = best.expect("nonempty actions");
        if root_key.is_some() {
            self.memory.frontier = None;
        }
        if ply == 0 {
            let bound = if best.score <= original_alpha {
                ScoreBound::Upper
            } else if best.score >= original_beta {
                ScoreBound::Lower
            } else {
                ScoreBound::Exact
            };
            self.root_result(depth, root_done, root_total, &best, bound);
        }

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
}
