//! Native experiment host. Uses the same game construction, rules, and search as the web worker.
use super::*;
use crate::search::{AnalysisView, BestAction};
use monumla_search::{SearchDomain, SearchOptions};
use std::time::{Duration, Instant};

pub struct Session(GameHandle);
#[derive(Serialize)]
pub struct Status {
    pub key: String,
    pub active: Vec<u32>,
    pub outcome: Option<GameOutcome>,
    pub plies: usize,
}
impl Session {
    pub fn new(ruleset: &str, pgn: Option<&str>) -> Result<Self, String> {
        let game = if let Some(pgn) = pgn {
            if ruleset != "chess" {
                return Err("PGN is a chess interchange format".into());
            }
            GameHandle {
                inner: Runtime::Chess(ChessRuntime::from_pgn(pgn)?),
            }
        } else if ruleset == "go" {
            GameHandle {
                inner: Runtime::Go(GoRuntime::new(
                    GoConfig::aga(9, GoScoring::Area, 0).map_err(js_error_string)?,
                )?),
            }
        } else {
            GameHandle::new_ruleset(ruleset)?
        };
        Ok(Self(game))
    }
    pub fn status(&self) -> Result<Status, String> {
        Ok(Status {
            key: self.0.analysis_position_key(),
            active: self
                .0
                .timeline()
                .current()
                .turn
                .active_players
                .iter()
                .map(|p| p.get())
                .collect(),
            outcome: self
                .0
                .domain()
                .outcome(self.0.timeline())
                .map_err(js_error_string)?,
            plies: self.0.timeline().history().len(),
        })
    }
    pub fn analyze(&self, options: SearchOptions, time_ms: u64) -> Result<AnalysisView, String> {
        let start = Instant::now();
        self.0.analyze_position(
            options,
            || time_ms > 0 && start.elapsed() >= Duration::from_millis(time_ms),
            |_| {},
        )
    }
    pub fn apply(&mut self, action: BestAction) -> Result<Status, String> {
        self.0.apply_analyzed_action(action)?;
        self.status()
    }
    /// Deterministic rule-generated opening, paired identically across versions.
    /// The runner checks keys to reject any rule/order incompatibility.
    pub fn opening(&mut self, seed: usize, plies: usize) -> Result<Status, String> {
        for ply in 0..plies {
            let domain = self.0.domain();
            if domain
                .outcome(self.0.timeline())
                .map_err(js_error_string)?
                .is_some()
            {
                break;
            }
            let actions = domain.actions(self.0.timeline()).map_err(js_error_string)?;
            if actions.is_empty() {
                break;
            }
            let index = seed
                .wrapping_mul(7919)
                .wrapping_add(ply.wrapping_mul(104729))
                % actions.len();
            let mut chosen = None;
            for offset in 0..actions.len() {
                let action = &actions[(index + offset) % actions.len()];
                let next = domain
                    .play(self.0.timeline(), action)
                    .map_err(js_error_string)?;
                if domain.outcome(&next).map_err(js_error_string)?.is_none() {
                    chosen = Some(action.clone());
                    break;
                }
            }
            let Some(action) = chosen else {
                break;
            };
            self.0.apply_analyzed_action(BestAction {
                position_key: self.0.analysis_position_key(),
                action,
            })?;
        }
        self.status()
    }
}
fn js_error_string(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use monumla_search::SearchLimits;
    #[test]
    fn native_peers_replay_shared_actions_in_every_ruleset() {
        for ruleset in ["chess", "checkers", "checkers-international", "go"] {
            let mut a = Session::new(ruleset, None).unwrap();
            let mut b = Session::new(ruleset, None).unwrap();
            assert_eq!(a.opening(1, 2).unwrap().key, b.opening(1, 2).unwrap().key);
            let result = a
                .analyze(
                    SearchOptions {
                        limits: SearchLimits {
                            max_depth: 1,
                            max_nodes: Some(10000),
                        },
                        ..Default::default()
                    },
                    0,
                )
                .unwrap();
            let action = result.best_action.unwrap();
            assert_eq!(
                a.apply(action.clone()).unwrap().key,
                b.apply(action).unwrap().key
            );
        }
    }
}
