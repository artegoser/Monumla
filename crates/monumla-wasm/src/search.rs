use super::*;
use monumla_core::PlayerId;
use monumla_search::{
    analyze, fingerprint,
    runtime::{RuntimeAction, RuntimeDomain},
    SearchDomain, SearchLimits, SearchMemory, SearchOptions, SearchResult, StopReason,
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
    pub samples: u64,
}
#[derive(Deserialize)]
struct CachedLine {
    score: i32,
    moves: Vec<String>,
}
#[derive(Deserialize)]
struct CachedAnalysis {
    lines: Vec<CachedLine>,
    perspective: u32,
    depth: u16,
    best_action: Option<BestAction>,
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
    #[wasm_bindgen(js_name=analysisPositionKey)]
    pub fn analysis_position_key(&self) -> String {
        self.position_key()
    }
    #[wasm_bindgen(js_name=reviewCachedTurn)]
    pub fn review_cached_turn(&self, index: u32, analysis: JsValue) -> Result<JsValue, JsValue> {
        let analysis = serde_wasm_bindgen::from_value(analysis).map_err(js_error)?;
        to_js(
            &self
                .cached_assessment(index as usize, analysis)
                .map_err(js_error)?,
        )
    }
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
        // A completed root variation already measures this exact action at the
        // same horizon. Reusing it avoids a contradictory second estimate.
        for (depth, best) in &best_depths {
            if let Some(line) = best
                .variations
                .iter()
                .find(|line| line.actions.first() == Some(&played))
            {
                let mut candidate = best.clone();
                candidate.score = Some(line.score);
                candidate.best_action = Some(played.clone());
                candidate.principal_variation = line.actions.clone();
                played_depths.insert(*depth, candidate);
            }
        }
        let already_measured = best_depths
            .keys()
            .next_back()
            .is_some_and(|d| played_depths.contains_key(d));
        let deadline = now_ms() + time_ms;
        if !already_measured {
            monumla_search::analyze_action(
                &domain,
                &before,
                actual.actor,
                played.clone(),
                options,
                || now_ms() >= deadline,
                |r| {
                    played_depths
                        .entry(r.completed_depth)
                        .or_insert_with(|| r.clone());
                },
            )
            .map_err(string_error)?;
        }
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
        let is_best = best.best_action.as_ref() == Some(&played);
        let margin = best
            .variations
            .first()
            .zip(best.variations.get(1))
            .map_or(0, |(a, b)| a.score.saturating_sub(b.score));
        Ok(assess_scores(
            depth,
            best.score,
            candidate.score,
            is_best,
            margin,
            best.best_action
                .as_ref()
                .map(|a| self.action_label(&domain, &before, a))
                .transpose()?,
        ))
    }
    fn cached_assessment(
        &self,
        index: usize,
        analysis: CachedAnalysis,
    ) -> Result<Option<MoveAssessment>, String> {
        let actual = self
            .timeline()
            .history()
            .turns()
            .get(index)
            .ok_or("Move outside history")?;
        let mut before = self.timeline().clone();
        while before.history().len() > index {
            before.undo().ok_or("Cannot rewind history")?;
        }
        let key = format!("{}:{:032x}", self.ruleset(), fingerprint(&before));
        if analysis
            .best_action
            .as_ref()
            .map(|a| a.position_key.as_str())
            != Some(key.as_str())
        {
            return Ok(None);
        }
        let notation = match &self.inner {
            Runtime::Chess(r) => r
                .rules
                .san_for_turn(actual, before.history())
                .map_err(string_error)?,
            Runtime::Go(_) => go_turn_notation(actual, before.current().board.width()),
            Runtime::Checkers(_) => checkers_turn_notation(actual),
            Runtime::Rules(r) => match r.kind {
                Kind::Battleship => monumla_games_battleship::turn_notation(actual),
                Kind::TicTacToe => monumla_games_tictactoe::turn_notation(actual),
            },
        };
        let Some((index, line)) = analysis
            .lines
            .iter()
            .enumerate()
            .find(|(_, l)| l.moves.first() == Some(&notation))
        else {
            return Ok(None);
        };
        let Some(best) = analysis.lines.first() else {
            return Ok(None);
        };
        let sign = if analysis.perspective == actual.actor.get() {
            1
        } else {
            -1
        };
        let best_score = best.score * sign;
        let played_score = line.score * sign;
        let margin = analysis
            .lines
            .get(1)
            .map_or(0, |second| best_score.saturating_sub(second.score * sign));
        Ok(Some(assess_scores(
            analysis.depth,
            Some(best_score),
            Some(played_score),
            index == 0,
            margin,
            best.moves.first().cloned(),
        )))
    }
    pub(super) fn domain(&self) -> RuntimeDomain {
        match &self.inner {
            Runtime::Rules(r) => {
                let kind = r.kind;
                RuntimeDomain::new(r.registry(), move |_| {
                    let rules: Rc<dyn InteractionRules> = match kind {
                        Kind::Battleship => Rc::new(monumla_games_battleship::BattleshipRules),
                        Kind::TicTacToe => Rc::new(monumla_games_tictactoe::TicTacToeRules),
                    };
                    Ok(rules)
                })
            }
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
    fn analysis_timeline(&self) -> Result<GameTimeline, String> {
        if let Runtime::Rules(r) = &self.inner {
            let registry = r.registry();
            if let Some(information) = registry.information() {
                let viewer = self
                    .timeline()
                    .current()
                    .turn
                    .active_players
                    .first()
                    .copied()
                    .unwrap_or(PlayerId::new(1));
                return GameTimeline::new(
                    information
                        .observe(self.timeline().current(), viewer)
                        .map_err(string_error)?,
                )
                .map_err(string_error);
            }
        }
        Ok(self.timeline().clone())
    }
    fn position_key(&self) -> String {
        let position = self
            .analysis_timeline()
            .expect("valid information projection");
        format!("{}:{:032x}", self.ruleset(), fingerprint(&position))
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
            timeline: self.analysis_timeline()?,
            initial_fen,
            undo_floor,
            pgn_tags,
        })
    }
    fn restore_snapshot(snapshot: Snapshot) -> Result<Self, String> {
        validate_timeline(&snapshot.timeline)?;
        let mut game = Self::new_ruleset(&snapshot.ruleset)?;
        match &mut game.inner {
            Runtime::Rules(r) => {
                r.timeline = snapshot.timeline;
                r.viewer = r.timeline.current().turn.active_players[0];
                r.rebuild_interaction()?;
            }
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
    pub(super) fn analyze_position(
        &self,
        options: SearchOptions,
        cancel: impl FnMut() -> bool,
        progress: impl FnMut(&AnalysisView),
    ) -> Result<AnalysisView, String> {
        let domain = self.domain();
        let mut memory = SearchMemory::new(options.transposition_entries);
        self.analyze_position_cached(&domain, &mut memory, options, cancel, progress, false)
    }
    pub(super) fn analyze_position_cached(
        &self,
        domain: &RuntimeDomain,
        memory: &mut SearchMemory<RuntimeAction, PlayerId>,
        options: SearchOptions,
        cancel: impl FnMut() -> bool,
        mut progress: impl FnMut(&AnalysisView),
        stable_perspective: bool,
    ) -> Result<AnalysisView, String> {
        if self.turn_in_progress() {
            return Err("Finish the current move first.".into());
        }
        let hidden = domain.has_hidden_information();
        let root = self.analysis_timeline()?;
        let player = if stable_perspective && !hidden {
            self.timeline()
                .current()
                .players
                .keys()
                .next()
                .copied()
                .unwrap_or(PlayerId::new(1))
        } else {
            self.timeline()
                .current()
                .turn
                .active_players
                .first()
                .copied()
                .unwrap_or(PlayerId::new(1))
        };
        let start = now_ms();
        let key = self.position_key();
        let convert = |result: &SearchResult<RuntimeAction>| -> Result<AnalysisView, String> {
            let mut lines = Vec::new();
            for variation in &result.variations {
                let mut position = root.clone();
                let mut moves = Vec::new();
                let path = variation
                    .actions
                    .first()
                    .map(|a| {
                        if hidden {
                            Ok(a.input
                                .choices
                                .iter()
                                .filter_map(|c| {
                                    if let ChoiceKind::SelectPosition { position } = c.kind {
                                        Some(position)
                                    } else {
                                        None
                                    }
                                })
                                .collect())
                        } else {
                            domain.action_path(&position, a)
                        }
                    })
                    .transpose()
                    .map_err(string_error)?
                    .unwrap_or_default()
                    .into_iter()
                    .map(PositionView::from)
                    .collect();
                for action in &variation.actions {
                    if hidden {
                        moves.push(
                            action
                                .input
                                .choices
                                .iter()
                                .map(|c| match &c.kind {
                                    ChoiceKind::SelectPosition { position } => {
                                        monumla_games_battleship::coordinate(
                                            usize::from(position.y) * 10
                                                + usize::from(position.x % 10),
                                        )
                                    }
                                    ChoiceKind::SelectOption { key } => key.clone(),
                                    _ => "Action".into(),
                                })
                                .collect::<Vec<_>>()
                                .join(" → "),
                        );
                    } else {
                        moves.push(self.action_label(domain, &position, action)?);
                        position = domain.play(&position, action).map_err(string_error)?;
                    }
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
                    StopReason::ForcedMove => "forced_move",
                }
                .into(),
                transposition_hits: result.statistics.transposition_hits,
                quiescence_nodes: result.statistics.quiescence_nodes,
                cutoffs: result.statistics.cutoffs,
                samples: result.statistics.samples,
            })
        };
        if hidden {
            let result = monumla_search::analyze_information(
                domain,
                &root,
                player,
                options,
                &mut domain.information_memory.borrow_mut(),
                cancel,
            )
            .map_err(string_error)?;
            let view = convert(&result)?;
            progress(&view);
            return Ok(view);
        }
        let result = monumla_search::analyze_cached(
            domain,
            self.timeline(),
            player,
            options,
            memory,
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
            Runtime::Checkers(_) => Ok(checkers_turn_notation(turn)),
            Runtime::Rules(r) => Ok(match r.kind {
                Kind::Battleship => monumla_games_battleship::turn_notation(turn),
                Kind::TicTacToe => monumla_games_tictactoe::turn_notation(turn),
            }),
        }
    }
    pub(super) fn apply_analyzed_action(
        &mut self,
        best: BestAction,
    ) -> Result<Vec<StepRecord>, String> {
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
            Runtime::Rules(r) => {
                r.timeline = next;
                r.rebuild_interaction()?;
            }
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

fn assess_scores(
    depth: u16,
    best_score: Option<i32>,
    played_score: Option<i32>,
    is_best: bool,
    margin: i32,
    best_move: Option<String>,
) -> MoveAssessment {
    let loss = best_score
        .zip(played_score)
        .map(|(b, p)| b.saturating_sub(p).max(0));
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
    MoveAssessment {
        depth,
        best_score,
        played_score,
        loss,
        symbol: symbol.map(str::to_owned),
        label: label.into(),
        best_move,
    }
}

/// A worker-owned search session. No JS pointers or game-specific search state.
#[wasm_bindgen]
pub struct SearchSession {
    game: GameHandle,
    domain: RuntimeDomain,
    memory: SearchMemory<RuntimeAction, PlayerId>,
}
#[wasm_bindgen]
impl SearchSession {
    #[wasm_bindgen(constructor)]
    pub fn new(snapshot: JsValue) -> Result<SearchSession, JsValue> {
        let game = GameHandle::restore_snapshot(
            serde_wasm_bindgen::from_value(snapshot).map_err(js_error)?,
        )
        .map_err(js_error)?;
        let domain = game.domain();
        Ok(Self {
            game,
            domain,
            memory: SearchMemory::new(16384),
        })
    }
    #[wasm_bindgen(js_name=setPosition)]
    pub fn set_position(&mut self, snapshot: JsValue) -> Result<(), JsValue> {
        let game = GameHandle::restore_snapshot(
            serde_wasm_bindgen::from_value(snapshot).map_err(js_error)?,
        )
        .map_err(js_error)?;
        if game.ruleset() != self.game.ruleset() {
            self.domain = game.domain();
            self.memory = SearchMemory::new(16384);
        }
        self.game = game;
        Ok(())
    }
    pub fn chunk(
        &mut self,
        depth: u16,
        nodes: u32,
        time_ms: u32,
        lines: u8,
        forced: bool,
    ) -> Result<JsValue, JsValue> {
        let deadline = now_ms() + f64::from(time_ms.clamp(20, 1000));
        let result = self
            .game
            .analyze_position_cached(
                &self.domain,
                &mut self.memory,
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: depth.clamp(1, 32),
                        max_nodes: Some(u64::from(nodes.clamp(100, 50_000_000))),
                    },
                    transposition_entries: 16384,
                    multi_pv: usize::from(lines.clamp(1, 5)),
                    stop_on_forced_move: forced,
                    late_move_reductions: true,
                    ..Default::default()
                },
                || now_ms() >= deadline,
                |_| {},
                true,
            )
            .map_err(js_error)?;
        to_js(&result)
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
    #[test]
    fn capability_probes_do_not_undervalue_knights_in_terminal_isolations() {
        for pgn in ["1. Nc3 d5 *", "1. e4 Nc6 2. d4 *"] {
            let game = GameHandle {
                inner: Runtime::Chess(ChessRuntime::from_pgn(pgn).unwrap()),
            };
            let report = game
                .analyze_position(
                    SearchOptions {
                        limits: SearchLimits {
                            max_depth: 1,
                            max_nodes: Some(5000),
                        },
                        ..Default::default()
                    },
                    || false,
                    |_| {},
                )
                .unwrap();
            assert!(
                !matches!(
                    report.principal_variation.first().map(String::as_str),
                    Some("Nxd5" | "Nxd4")
                ),
                "{pgn}: {:?}",
                report.principal_variation
            );
        }
    }
    #[test]
    fn shared_capability_probes_value_draughts_promotion_without_piece_knowledge() {
        for ruleset in ["checkers", "checkers-international"] {
            let game = GameHandle::new_ruleset(ruleset).unwrap();
            let domain = game.domain();
            domain.prepare(game.timeline()).unwrap();
            let player = PlayerId::new(1);
            let ordinary = domain.evaluate(game.timeline(), player).unwrap();
            let mut upgraded = game.timeline().current().clone();
            let id = upgraded
                .entities
                .values()
                .find(|e| e.controller == player)
                .unwrap()
                .id;
            upgraded
                .entity_mut(id)
                .unwrap()
                .state
                .insert(monumla_games_checkers::KING, true);
            let upgraded = GameTimeline::new(upgraded).unwrap();
            assert!(
                domain.evaluate(&upgraded, player).unwrap() > ordinary + 100,
                "{ruleset}"
            );
        }
    }
    #[test]
    fn published_analysis_grades_played_candidates_without_searching_again() {
        for ruleset in ["chess", "checkers", "checkers-international", "go"] {
            let mut game = if ruleset == "go" {
                GameHandle {
                    inner: Runtime::Go(
                        GoRuntime::new(GoConfig::aga(9, GoScoring::Area, 0).unwrap()).unwrap(),
                    ),
                }
            } else {
                GameHandle::new_ruleset(ruleset).unwrap()
            };
            let report = game
                .analyze_position(
                    SearchOptions {
                        limits: SearchLimits {
                            max_depth: 1,
                            max_nodes: Some(10000),
                        },
                        multi_pv: 3,
                        ..Default::default()
                    },
                    || false,
                    |_| {},
                )
                .unwrap();
            game.apply_analyzed_action(report.best_action.clone().unwrap())
                .unwrap();
            let cached = CachedAnalysis {
                depth: report.depth,
                perspective: report.perspective,
                best_action: report.best_action,
                lines: report
                    .lines
                    .into_iter()
                    .map(|line| CachedLine {
                        score: line.score,
                        moves: line.moves,
                    })
                    .collect(),
            };
            let grade = game.cached_assessment(0, cached).unwrap().unwrap();
            assert_eq!(grade.loss, Some(0), "{ruleset}");
            assert_ne!(grade.label, "Mistake");
            assert_ne!(grade.label, "Blunder");
        }
    }
    #[test]
    fn recommended_move_review_agrees_with_root_search() {
        let game = GameHandle {
            inner: Runtime::Chess(
                ChessRuntime::from_pgn("1. Nc3 Nc6 2. e4 e5 3. Bc4 Bc5 4. Qg4 Qf6 *").unwrap(),
            ),
        };
        let assessment = game
            .assess_turn(
                7,
                SearchOptions {
                    limits: SearchLimits {
                        max_depth: 3,
                        max_nodes: Some(10000),
                    },
                    multi_pv: 2,
                    ..Default::default()
                },
                6000.,
            )
            .unwrap();
        if assessment.best_move.as_deref() == Some("Qf6") {
            assert_eq!(assessment.loss, Some(0));
        }
    }
    #[test]
    fn rejects_reported_bishop_sacrifice_in_imported_and_played_positions() {
        let pgn = "1. Nc3 Nc6 2. e4 e5 3. Bc4 Bc5 4. Qg4 g5 5. Nf3 *";
        let imported = ChessRuntime::from_pgn(pgn).unwrap();
        let mut played = ChessRuntime::standard().unwrap();
        for san in ["Nc3", "Nc6", "e4", "e5", "Bc4", "Bc5", "Qg4", "g5", "Nf3"] {
            let (movement, _) = played
                .rules
                .resolve_san(played.timeline.current(), played.timeline.history(), san)
                .unwrap();
            let actor = played
                .rules
                .side_to_move(played.timeline.current())
                .unwrap()
                .player();
            let mut turn = played.timeline.begin_turn(actor).unwrap();
            played
                .rules
                .execute_move(&mut turn, Some(played.timeline.history()), movement, None)
                .unwrap();
            played.timeline.commit_turn(turn).unwrap();
        }
        played.rebuild_interaction().unwrap();
        let mut models = Vec::new();
        for runtime in [imported, played] {
            let game = GameHandle {
                inner: Runtime::Chess(runtime),
            };
            let domain = game.domain();
            domain.prepare(game.timeline()).unwrap();
            let model: BTreeMap<_, _> = game
                .timeline()
                .current()
                .entities
                .values()
                .map(|e| {
                    (
                        (e.position, e.entity_type, e.owner),
                        domain.inferred_value(e),
                    )
                })
                .collect();
            models.push(model);
            let report = game
                .analyze_position(
                    SearchOptions {
                        limits: SearchLimits {
                            max_depth: 2,
                            max_nodes: Some(100000),
                        },
                        multi_pv: 3,
                        ..Default::default()
                    },
                    || false,
                    |_| {},
                )
                .unwrap();
            assert_ne!(
                report.principal_variation.first().map(String::as_str),
                Some("Bxf2+")
            );
        }
        assert_eq!(
            models[0], models[1],
            "inferred values must not depend on entity IDs"
        );
    }
}

#[cfg(test)]
#[path = "games_tests.rs"]
mod games_tests;
