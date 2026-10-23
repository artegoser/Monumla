use super::*;
use monumla_core::PlayerId;
use monumla_search::{
    analyze, fingerprint,
    runtime::{RuntimeAction, RuntimeDomain},
    SearchDomain, SearchLimits, SearchOptions, SearchResult, StopReason,
};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Clone, Serialize, Deserialize)]
struct Snapshot {
    ruleset: String,
    timeline: GameTimeline,
    initial_fen: String,
    undo_floor: usize,
    pgn_tags: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BestAction {
    pub position_key: String,
    pub action: RuntimeAction,
}
#[derive(Clone, Debug, Serialize)]
pub struct AnalysisLine {
    pub score: i32,
    pub moves: Vec<String>,
    pub path: Vec<PositionView>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AnalysisView {
    pub lines: Vec<AnalysisLine>,
    pub score: Option<i32>,
    pub perspective: u32,
    pub depth: u16,
    pub nodes: u64,
    pub elapsed_ms: f64,
    pub nodes_per_second: f64,
    pub principal_variation: Vec<String>,
    pub best_action: Option<BestAction>,
    pub stop_reason: String,
    pub transposition_hits: u64,
    pub quiescence_nodes: u64,
    pub cutoffs: u64,
}
/// A comparison of the actual move and best move at the same completed depth.
#[derive(Clone, Debug, Serialize)]
pub struct MoveAssessment {
    pub depth: u16,
    pub best_score: Option<i32>,
    pub played_score: Option<i32>,
    pub loss: Option<i32>,
    pub symbol: Option<String>,
    pub label: String,
    pub best_move: Option<String>,
}
#[wasm_bindgen]
pub fn from_search_snapshot(snapshot: JsValue) -> Result<GameHandle, JsValue> {
    GameHandle::restore_snapshot(serde_wasm_bindgen::from_value(snapshot).map_err(js_error)?)
        .map_err(js_error)
}

#[wasm_bindgen]
impl GameHandle {
    #[wasm_bindgen(js_name=searchSnapshot)]
    pub fn search_snapshot(&self) -> Result<JsValue, JsValue> {
        to_js(&self.snapshot().map_err(js_error)?)
    }
    #[wasm_bindgen(js_name=turnInProgress)]
    pub fn turn_in_progress(&self) -> bool {
        self.visible_state() != self.timeline().current()
            || self.interaction_view().pending_target.is_some()
    }
    pub fn analyze(
        &self,
        max_depth: u16,
        max_nodes: u32,
        time_ms: u32,
        line_count: u8,
        progress: js_sys::Function,
    ) -> Result<JsValue, JsValue> {
        let deadline = now_ms() + f64::from(time_ms.clamp(20, 600_000));
        let options = SearchOptions {
            limits: SearchLimits {
                max_depth: max_depth.clamp(1, 32),
                max_nodes: Some(u64::from(max_nodes.clamp(100, 50_000_000))),
            },
            multi_pv: usize::from(line_count.clamp(1, 5)),
            transposition_entries: 16_384,
            ..SearchOptions::default()
        };
        let report = self
            .analyze_position(
                options,
                || now_ms() >= deadline,
                |report| {
                    if let Ok(value) = to_js(report) {
                        let _ = progress.call1(&JsValue::NULL, &value);
                    }
                },
            )
            .map_err(js_error)?;
        to_js(&report)
    }
    #[wasm_bindgen(js_name=reviewTurn)]
    pub fn review_turn(
        &self,
        index: u32,
        max_depth: u16,
        nodes: u32,
        time_ms: u32,
    ) -> Result<JsValue, JsValue> {
        let report = self
            .assess_turn(
                index as usize,
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: max_depth.clamp(1, 8),
                        max_nodes: Some(u64::from(nodes.clamp(100, 500_000))),
                    },
                    multi_pv: 2,
                    transposition_entries: 4096,
                    ..Default::default()
                },
                f64::from(time_ms.clamp(50, 30_000)),
            )
            .map_err(js_error)?;
        to_js(&report)
    }
    #[wasm_bindgen(js_name=applyBestAction)]
    pub fn apply_best_action(&mut self, best: JsValue) -> Result<JsValue, JsValue> {
        let best = serde_wasm_bindgen::from_value(best).map_err(js_error)?;
        let steps = self.apply_analyzed_action(best).map_err(js_error)?;
        to_js(&self.transition_view(true, &steps).map_err(js_error)?)
    }
}
impl GameHandle {
    fn assess_turn(
        &self,
        index: usize,
        options: SearchOptions,
        time_ms: f64,
    ) -> Result<MoveAssessment, String> {
        let actual = self
            .timeline()
            .history()
            .turns()
            .get(index)
            .cloned()
            .ok_or("Move is outside the history")?;
        if actual.synthetic {
            return Err("Synthetic history is not a played move".into());
        }
        let mut before = self.timeline().clone();
        while before.history().len() > index {
            before.undo().ok_or("Cannot rewind review history")?;
        }
        let domain = self.domain();
        let played = domain
            .actions(&before)
            .map_err(string_error)?
            .into_iter()
            .find(|a| {
                domain
                    .action_turn(&before, a)
                    .is_ok_and(|t| t.working == actual.after)
            });
        let Some(played) = played else {
            return Ok(MoveAssessment {
                depth: 0,
                best_score: None,
                played_score: None,
                loss: None,
                symbol: None,
                label: "Game action".into(),
                best_move: None,
            });
        };
        let mut best_depths = BTreeMap::new();
        let mut played_depths = BTreeMap::new();
        let deadline = now_ms() + time_ms;
        analyze(
            &domain,
            &before,
            actual.actor,
            options,
            || now_ms() >= deadline,
            |r| {
                best_depths.insert(r.completed_depth, r.clone());
            },
        )
        .map_err(string_error)?;
        let deadline = now_ms() + time_ms;
        monumla_search::analyze_action(
            &domain,
            &before,
            actual.actor,
            played.clone(),
            options,
            || now_ms() >= deadline,
            |r| {
                played_depths.insert(r.completed_depth, r.clone());
            },
        )
        .map_err(string_error)?;
        let common = best_depths
            .iter()
            .rev()
            .find_map(|(d, b)| played_depths.get(d).map(|p| (*d, b, p)));
        let Some((depth, best, candidate)) = common else {
            return Ok(MoveAssessment {
                depth: 0,
                best_score: None,
                played_score: None,
                loss: None,
                symbol: None,
                label: "Unassessed".into(),
                best_move: None,
            });
        };
        let loss = best
            .score
            .zip(candidate.score)
            .map(|(b, p)| b.saturating_sub(p).max(0));
        let is_best = best.best_action.as_ref() == Some(&played);
        let margin = best
            .variations
            .first()
            .zip(best.variations.get(1))
            .map_or(0, |(a, b)| a.score.saturating_sub(b.score));
        let (symbol, label) = if loss.unwrap_or(0) >= 250 {
            (Some("??"), "Blunder")
        } else if depth < 2 {
            (None, "Shallow estimate")
        } else if loss.unwrap_or(0) >= 100 {
            (Some("?"), "Mistake")
        } else if loss.unwrap_or(0) >= 40 {
            (Some("?!"), "Inaccuracy")
        } else if is_best && depth >= 3 && margin >= 200 {
            (Some("!!"), "Only clearly preferred move")
        } else if is_best && depth >= 3 && margin >= 80 {
            (Some("!"), "Strong move")
        } else if is_best || loss == Some(0) {
            (Some("✓"), "Best move")
        } else {
            (Some("·"), "Good move")
        };
        Ok(MoveAssessment {
            depth,
            best_score: best.score,
            played_score: candidate.score,
            loss,
            symbol: symbol.map(str::to_owned),
            label: label.into(),
            best_move: best
                .best_action
                .as_ref()
                .map(|a| self.action_label(&domain, &before, a))
                .transpose()?,
        })
    }
    fn domain(&self) -> RuntimeDomain {
        match &self.inner {
            Runtime::Chess(runtime) => {
                let rules = runtime.rules.clone();
                let mut registry = monumla_core::RuleRegistry::new();
                registry.register_outcome(rules.outcome_rule());
                RuntimeDomain::new(registry, move |position| {
                    Ok(Rc::new(ChessInteractionRules::with_history(
                        &rules,
                        position.history(),
                    )))
                })
            }
            Runtime::Checkers(_) => RuntimeDomain::new(checkers_registry(), |_| {
                Ok(Rc::new(CheckersInteractionRules))
            }),
            Runtime::Go(_) => RuntimeDomain::new(go_registry(), |position| {
                Ok(Rc::new(GoInteractionRules::with_history(
                    position.history(),
                )?))
            }),
        }
    }
    fn position_key(&self) -> String {
        format!("{}:{:032x}", self.ruleset(), fingerprint(self.timeline()))
    }
    fn snapshot(&self) -> Result<Snapshot, String> {
        if self.turn_in_progress() {
            return Err("Finish the current move first.".into());
        }
        let (initial_fen, undo_floor, pgn_tags) = match &self.inner {
            Runtime::Chess(r) => (r.initial_fen.clone(), r.undo_floor, r.pgn_tags.clone()),
            _ => (String::new(), 0, BTreeMap::new()),
        };
        Ok(Snapshot {
            ruleset: self.ruleset(),
            timeline: self.timeline().clone(),
            initial_fen,
            undo_floor,
            pgn_tags,
        })
    }
    fn restore_snapshot(snapshot: Snapshot) -> Result<Self, String> {
        validate_timeline(&snapshot.timeline)?;
        let mut game = Self::new_ruleset(&snapshot.ruleset)?;
        match &mut game.inner {
            Runtime::Chess(r) => {
                r.timeline = snapshot.timeline;
                r.initial_fen = snapshot.initial_fen;
                r.undo_floor = snapshot.undo_floor;
                r.pgn_tags = snapshot.pgn_tags;
                r.rebuild_interaction()?;
            }
            Runtime::Checkers(r) => {
                r.timeline = snapshot.timeline;
                r.rebuild_interaction()?;
            }
            Runtime::Go(r) => {
                r.timeline = snapshot.timeline;
                r.rebuild_interaction()?;
            }
        }
        Ok(game)
    }
    fn analyze_position(
        &self,
        options: SearchOptions,
        cancel: impl FnMut() -> bool,
        mut progress: impl FnMut(&AnalysisView),
    ) -> Result<AnalysisView, String> {
        if self.turn_in_progress() {
            return Err("Finish the current move first.".into());
        }
        let domain = self.domain();
        let player = self
            .timeline()
            .current()
            .turn
            .active_players
            .first()
            .copied()
            .unwrap_or(PlayerId::new(1));
        let start = now_ms();
        let key = self.position_key();
        let convert = |result: &SearchResult<RuntimeAction>| -> Result<AnalysisView, String> {
            let mut lines = Vec::new();
            for variation in &result.variations {
                let mut position = self.timeline().clone();
                let mut moves = Vec::new();
                let path = variation
                    .actions
                    .first()
                    .map(|a| domain.action_path(&position, a))
                    .transpose()
                    .map_err(string_error)?
                    .unwrap_or_default()
                    .into_iter()
                    .map(PositionView::from)
                    .collect();
                for action in &variation.actions {
                    moves.push(self.action_label(&domain, &position, action)?);
                    position = domain.play(&position, action).map_err(string_error)?;
                }
                lines.push(AnalysisLine {
                    score: variation.score,
                    moves,
                    path,
                });
            }
            let elapsed_ms = (now_ms() - start).max(0.0);
            Ok(AnalysisView {
                principal_variation: lines.first().map(|v| v.moves.clone()).unwrap_or_default(),
                lines,
                score: result.score,
                perspective: player.get(),
                depth: result.completed_depth,
                nodes: result.nodes,
                elapsed_ms,
                nodes_per_second: result.nodes as f64 * 1000.0 / elapsed_ms.max(1.0),
                best_action: result.best_action.clone().map(|action| BestAction {
                    position_key: key.clone(),
                    action,
                }),
                stop_reason: match result.stop_reason {
                    StopReason::DepthLimit => "depth_limit",
                    StopReason::NodeLimit => "node_limit",
                    StopReason::Cancelled => "time_limit",
                    StopReason::Terminal => "terminal",
                }
                .into(),
                transposition_hits: result.statistics.transposition_hits,
                quiescence_nodes: result.statistics.quiescence_nodes,
                cutoffs: result.statistics.cutoffs,
            })
        };
        let result = analyze(
            &domain,
            self.timeline(),
            player,
            options,
            cancel,
            |result| {
                if let Ok(view) = convert(result) {
                    progress(&view);
                }
            },
        )
        .map_err(string_error)?;
        convert(&result)
    }
    fn action_label(
        &self,
        domain: &RuntimeDomain,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<String, String> {
        let next = domain.play(position, action).map_err(string_error)?;
        let turn = next
            .history()
            .previous_turn()
            .ok_or("Action did not finish a turn")?;
        match &self.inner {
            Runtime::Chess(r) => r
                .rules
                .san_for_turn(turn, position.history())
                .map_err(string_error),
            Runtime::Go(_) => Ok(go_turn_notation(turn, position.current().board.width())),
            Runtime::Checkers(_) => {
                let path = domain.action_path(position, action).map_err(string_error)?;
                let capture = turn.before.entities.len() > turn.after.entities.len();
                let format = |p: Position| {
                    if position.current().board.width() == 10 {
                        ((9 - p.y) * 5 + p.x / 2 + 1).to_string()
                    } else {
                        format!("{}{}", char::from(b'a' + p.x as u8), p.y + 1)
                    }
                };
                Ok(path
                    .into_iter()
                    .map(format)
                    .collect::<Vec<_>>()
                    .join(if capture { "x" } else { "-" }))
            }
        }
    }
    fn apply_analyzed_action(&mut self, best: BestAction) -> Result<Vec<StepRecord>, String> {
        if self.turn_in_progress() {
            return Err("Finish the current move first.".into());
        }
        if best.position_key != self.position_key() {
            return Err("Analysis belongs to an older position.".into());
        }
        let domain = self.domain();
        if domain
            .outcome(self.timeline())
            .map_err(string_error)?
            .is_some()
        {
            return Err("The game has finished".into());
        }
        let next = domain
            .play(self.timeline(), &best.action)
            .map_err(string_error)?;
        let steps = next
            .history()
            .previous_turn()
            .ok_or("Action did not finish a turn")?
            .steps
            .clone();
        match &mut self.inner {
            Runtime::Chess(r) => {
                r.timeline = next;
                r.rebuild_interaction()?;
            }
            Runtime::Checkers(r) => {
                r.timeline = next;
                r.rebuild_interaction()?;
            }
            Runtime::Go(r) => {
                r.timeline = next;
                r.rebuild_interaction()?;
            }
        }
        Ok(steps)
    }
}

/// Native host uses the exact same protocol as the browser worker.
pub fn native_analysis(ruleset: &str, options: SearchOptions) -> Result<AnalysisView, String> {
    GameHandle::new_ruleset(ruleset)?.analyze_position(options, || false, |_| {})
}
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}
#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
        * 1000.0
}
fn validate_timeline(timeline: &GameTimeline) -> Result<(), String> {
    timeline.current().validate().map_err(string_error)?;
    let mut previous = None;
    for turn in timeline.history().turns() {
        turn.before.validate().map_err(string_error)?;
        turn.after.validate().map_err(string_error)?;
        if previous.is_some_and(|state| state != &turn.before) {
            return Err("Invalid search history".into());
        }
        let mut at = &turn.before;
        for step in &turn.steps {
            if at != &step.before {
                return Err("Invalid search steps".into());
            }
            step.after.validate().map_err(string_error)?;
            at = &step.after;
        }
        if at != &turn.after {
            return Err("Invalid search turn".into());
        }
        previous = Some(&turn.after);
    }
    if previous.is_some_and(|state| state != timeline.current()) {
        return Err("Search history does not match current position".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options(depth: u16) -> SearchOptions {
        SearchOptions {
            limits: SearchLimits {
                max_depth: depth,
                max_nodes: Some(20_000),
            },
            quiescence_depth: 0,
            multi_pv: 3,
            ..Default::default()
        }
    }
    fn analyze_game(game: &GameHandle, depth: u16) -> AnalysisView {
        game.analyze_position(options(depth), || false, |_| {})
            .unwrap()
    }
    #[test]
    fn every_game_uses_shared_search_and_replays_worker_inputs() {
        for ruleset in ["chess", "checkers", "checkers-international", "go"] {
            let mut game = if ruleset == "go" {
                GameHandle {
                    inner: Runtime::Go(
                        GoRuntime::new(GoConfig {
                            size: 9,
                            ..GoConfig::default()
                        })
                        .unwrap(),
                    ),
                }
            } else {
                GameHandle::new_ruleset(ruleset).unwrap()
            };
            let initial = game.timeline().clone();
            let restored = GameHandle::restore_snapshot(game.snapshot().unwrap()).unwrap();
            assert_eq!(restored.timeline(), &initial);
            let report = analyze_game(&restored, 1);
            assert_eq!(report.depth, 1, "{ruleset}");
            assert_eq!(report.lines.len(), 3, "{ruleset}");
            assert!(report.lines.windows(2).all(|v| v[0].score >= v[1].score));
            let mut best = report.best_action.unwrap();
            // Worker serialization carries only core TurnInput, no cached turn.
            best.action = RuntimeAction::from(best.action.input);
            game.apply_analyzed_action(best.clone()).unwrap();
            assert_eq!(game.timeline().history().len(), initial.history().len() + 1);
            assert!(game
                .apply_analyzed_action(best)
                .unwrap_err()
                .contains("older position"));
            assert_eq!(restored.timeline(), &initial);
        }
    }
    #[test]
    fn interrupted_first_iteration_never_invents_a_move_or_score() {
        let game = GameHandle::new_ruleset("chess").unwrap();
        let report = game.analyze_position(options(3), || true, |_| {}).unwrap();
        assert_eq!(report.depth, 0);
        assert!(report.best_action.is_none());
        assert!(report.score.is_none());
        assert!(report.lines.is_empty());
    }
    #[test]
    fn mate_is_found_and_applied_through_normal_rules() {
        let mut game = GameHandle {
            inner: Runtime::Chess(ChessRuntime::from_fen("7k/8/5KQ1/8/8/8/8/8 w - - 0 1").unwrap()),
        };
        let report = analyze_game(&game, 2);
        assert!(report.score.unwrap() > 90_000);
        assert!(report.lines[0].moves[0].ends_with('#'));
        game.apply_analyzed_action(report.best_action.unwrap())
            .unwrap();
        assert!(game.game_view().unwrap().status.outcome.is_some());
    }
    #[test]
    fn midgame_from_report_completes_iterations_and_keeps_searching() {
        let game = GameHandle {
            inner: Runtime::Chess(
                ChessRuntime::from_pgn(
                    "1. d4 Nc6 2. Nc3 Nf6 3. Nf3 a5 4. Bf4 h5 5. a4 b6 6. h4 d6 7. b3 Bf5 *",
                )
                .unwrap(),
            ),
        };
        let mut depths = Vec::new();
        let report = game
            .analyze_position(
                SearchOptions {
                    multi_pv: 1,
                    ..options(2)
                },
                || false,
                |v| depths.push(v.depth),
            )
            .unwrap();
        assert_eq!(depths, [1, 2]);
        assert_eq!(report.depth, 2);
        assert!(report.best_action.is_some());
        assert!(report.nodes > 50);
    }
    #[test]
    fn all_promotion_choices_are_complete_generic_turns() {
        let game = GameHandle {
            inner: Runtime::Chess(ChessRuntime::from_fen("7k/P7/8/8/8/8/8/7K w - - 0 1").unwrap()),
        };
        let domain = game.domain();
        let promotions = domain
            .actions(game.timeline())
            .unwrap()
            .into_iter()
            .filter(|action| {
                domain
                    .action_turn(game.timeline(), action)
                    .unwrap()
                    .steps
                    .iter()
                    .flat_map(|s| &s.delta.changes)
                    .any(|c| matches!(c, monumla_core::StateChange::EntityTypeChanged { .. }))
            })
            .collect::<Vec<_>>();
        assert_eq!(promotions.len(), 4);
        for action in promotions {
            assert_eq!(
                domain
                    .play(game.timeline(), &action)
                    .unwrap()
                    .history()
                    .len(),
                game.timeline().history().len() + 1
            );
        }
    }
    #[test]
    fn does_not_hang_queen_in_reported_opening() {
        let game = GameHandle {
            inner: Runtime::Chess(
                ChessRuntime::from_pgn("1. e4 e5 2. Nf3 d5 3. Nc3 dxe4 4. Nxe4 *").unwrap(),
            ),
        };
        let report = game
            .analyze_position(
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 1,
                        max_nodes: Some(5000),
                    },
                    ..SearchOptions::default()
                },
                || false,
                |_| {},
            )
            .unwrap();
        assert_eq!(report.depth, 1);
        assert_ne!(
            report.principal_variation.first().map(String::as_str),
            Some("Qxd2+")
        );
    }
    #[test]
    fn review_marks_reported_queen_sacrifice_from_equal_depth_searches() {
        let game = GameHandle {
            inner: Runtime::Chess(
                ChessRuntime::from_pgn("1. e4 e5 2. Nf3 d5 3. Nc3 dxe4 4. Nxe4 Qxd2+ *").unwrap(),
            ),
        };
        let report = game
            .assess_turn(
                7,
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 1,
                        max_nodes: Some(5000),
                    },
                    multi_pv: 2,
                    ..Default::default()
                },
                1_000_000.0,
            )
            .unwrap();
        assert_eq!(report.depth, 1);
        assert_eq!(report.symbol.as_deref(), Some("??"));
        assert!(report.loss.unwrap() >= 250);
        assert_ne!(report.best_move.as_deref(), Some("Qxd2+"));
    }
    #[test]
    fn shared_turn_enumeration_preserves_forced_draughts_capture_chains() {
        use monumla_core::{EntityId, EntityState};
        for variant in [DraughtsVariant::Russian, DraughtsVariant::International] {
            let mut state = checkers_state_for_variant(variant);
            for id in state.entities.keys().copied().collect::<Vec<_>>() {
                state.remove_entity(id).unwrap();
            }
            for (id, owner, x, y) in [(1, 1, 0, 0), (2, 2, 1, 1), (3, 2, 3, 3)] {
                state
                    .add_entity(EntityState::new(
                        EntityId::new(id),
                        monumla_games_checkers::CHECKER,
                        PlayerId::new(owner),
                        Position::new(x, y),
                    ))
                    .unwrap();
            }
            let mut runtime = CheckersRuntime::new(variant).unwrap();
            runtime.timeline = GameTimeline::new(state).unwrap();
            runtime.rebuild_interaction().unwrap();
            let mut game = GameHandle {
                inner: Runtime::Checkers(runtime),
            };
            let report = analyze_game(&game, 1);
            let best = report.best_action.unwrap();
            assert_eq!(best.action.input.choices.len(), 2);
            game.apply_analyzed_action(best).unwrap();
            assert_eq!(game.timeline().current().entities.len(), 1);
            assert_eq!(game.timeline().history().len(), 1);
        }
    }
}
