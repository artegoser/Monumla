//! Tactical horizon search with separate bound-aware caching.
use super::*;

impl<D: SearchDomain, F: FnMut() -> bool> Context<'_, D, F> {
    pub(super) fn quiet(
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
