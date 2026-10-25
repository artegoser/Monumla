//! Match runner only: chess rules validate both engines; Monumla uses shared search.
use monumla_core::{
    ChoiceInput, EntityTypeId, GameTimeline, PlayerId, Position, RuleRegistry, StateMap, StateValue,
};
use monumla_games_chess::{
    ChessInteractionRules, ChessRules, BISHOP, KNIGHT, QUEEN, ROOK, STANDARD_FEN,
};
use monumla_search::{analyze, runtime::RuntimeDomain, SearchDomain, SearchLimits, SearchOptions};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    rc::Rc,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Config {
    stockfish: String,
    games: usize,
    time_ms: u64,
    max_plies: usize,
    out: PathBuf,
    elo: Option<u32>,
    clock_ms: Option<u64>,
    increment_ms: u64,
}
impl Config {
    fn parse() -> Result<Self> {
        let mut c = Self {
            stockfish: "stockfish".into(),
            games: 6,
            elo: None,
            clock_ms: None,
            increment_ms: 1000,
            time_ms: 1000,
            max_plies: 160,
            out: "target/monumla-bench/stockfish/latest".into(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(key) = args.next() {
            let value = args.next().ok_or("Each option requires a value")?;
            match key.as_str() {
                "--stockfish" => c.stockfish = value,
                "--games" => c.games = value.parse()?,
                "--elo" => c.elo = Some(value.parse()?),
                "--clock-ms" => c.clock_ms = Some(value.parse()?),
                "--increment-ms" => c.increment_ms = value.parse()?,
                "--movetime-ms" => c.time_ms = value.parse()?,
                "--max-plies" => c.max_plies = value.parse()?,
                "--out" => c.out = value.into(),
                _ => return Err(format!("Unknown option {key}").into()),
            }
        }
        if c.games == 0
            || c.games % 2 != 0
            || c.games > 1000
            || !(20..=60000).contains(&c.time_ms)
            || !(2..=500).contains(&c.max_plies)
        {
            return Err("Use an even game count (2..1000), 20..60000 ms, and 2..500 plies".into());
        }
        if c.clock_ms.is_some_and(|v| !(1000..=3_600_000).contains(&v)) || c.increment_ms > 60000 {
            return Err("Clock must be 1000..3600000 ms; increment 0..60000 ms".into());
        }
        Ok(c)
    }
}
struct Opponent {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
    name: String,
    options: Vec<String>,
}
impl Opponent {
    fn new(path: &str) -> Result<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().ok_or("No UCI stdin")?;
        let stdout = child.stdout.take().ok_or("No UCI stdout")?;
        let (send, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut engine = Self {
            child,
            input,
            output,
            name: "Stockfish".into(),
            options: Vec::new(),
        };
        engine.send("uci")?;
        for line in engine.until("uciok", Duration::from_secs(30))? {
            if line.starts_with("option name ") {
                engine.options.push(line.clone());
            }
            if let Some(name) = line.strip_prefix("id name ") {
                engine.name = name.to_owned();
            }
        }
        for setting in [
            "Threads value 1",
            "Hash value 16",
            "Skill Level value 20",
            "UCI_LimitStrength value false",
            "Ponder value false",
            "MultiPV value 1",
        ] {
            engine.send(&format!("setoption name {setting}"))?;
        }
        engine.ready()?;
        Ok(engine)
    }
    fn limit_strength(&mut self, elo: u32) -> Result<()> {
        let option = self
            .options
            .iter()
            .find(|s| s.starts_with("option name UCI_Elo "))
            .ok_or("Opponent does not advertise UCI_Elo")?;
        let words: Vec<_> = option.split_whitespace().collect();
        let bound = |key| -> Result<u32> {
            Ok(words
                .windows(2)
                .find(|p| p[0] == key)
                .ok_or("UCI_Elo missing bounds")?[1]
                .parse()?)
        };
        if !(bound("min")?..=bound("max")?).contains(&elo) {
            return Err("Elo outside advertised opponent bounds".into());
        }
        if !self
            .options
            .iter()
            .any(|s| s.starts_with("option name UCI_LimitStrength "))
        {
            return Err("Opponent does not advertise UCI_LimitStrength".into());
        }
        self.send("setoption name UCI_LimitStrength value true")?;
        self.send(&format!("setoption name UCI_Elo value {elo}"))?;
        self.ready()
    }
    fn send(&mut self, command: &str) -> Result<()> {
        writeln!(self.input, "{command}")?;
        self.input.flush()?;
        Ok(())
    }
    fn until(&mut self, prefix: &str, timeout: Duration) -> Result<Vec<String>> {
        let deadline = Instant::now() + timeout;
        let mut lines = Vec::new();
        loop {
            let line = self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
            let done = line.starts_with(prefix);
            lines.push(line);
            if done {
                return Ok(lines);
            }
        }
    }
    fn ready(&mut self) -> Result<()> {
        self.send("isready")?;
        self.until("readyok", Duration::from_secs(30))?;
        Ok(())
    }
    fn new_game(&mut self) -> Result<()> {
        self.send("ucinewgame")?;
        self.ready()
    }
    fn best(
        &mut self,
        moves: &[String],
        time_ms: u64,
        clocks: Option<[u64; 2]>,
        increment: u64,
    ) -> Result<(String, u16, u64)> {
        self.send(&format!("position startpos moves {}", moves.join(" ")))?;
        if let Some([white, black]) = clocks {
            self.send(&format!(
                "go wtime {white} btime {black} winc {increment} binc {increment}"
            ))?;
        } else {
            self.send(&format!("go movetime {time_ms}"))?;
        }
        let lines = self.until("bestmove ", Duration::from_millis(time_ms + 10000))?;
        let mut depth = 0;
        let mut nodes = 0;
        for line in &lines {
            let words: Vec<_> = line.split_whitespace().collect();
            for pair in words.windows(2) {
                match pair[0] {
                    "depth" => depth = pair[1].parse().unwrap_or(depth),
                    "nodes" => nodes = pair[1].parse().unwrap_or(nodes),
                    _ => {}
                }
            }
        }
        let best = lines
            .last()
            .and_then(|s| s.split_whitespace().nth(1))
            .ok_or("Missing bestmove")?
            .to_owned();
        Ok((best, depth, nodes))
    }
}
impl Drop for Opponent {
    fn drop(&mut self) {
        let _ = self.send("quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn promotion(code: u8) -> Option<EntityTypeId> {
    match code {
        b'q' => Some(QUEEN),
        b'r' => Some(ROOK),
        b'b' => Some(BISHOP),
        b'n' => Some(KNIGHT),
        _ => None,
    }
}
fn apply_uci(rules: &ChessRules, timeline: &mut GameTimeline, uci: &str) -> Result<String> {
    let bytes = uci.as_bytes();
    if !(4..=5).contains(&bytes.len())
        || ![bytes[0], bytes[2]]
            .iter()
            .all(|b| (b'a'..=b'h').contains(b))
        || ![bytes[1], bytes[3]]
            .iter()
            .all(|b| (b'1'..=b'8').contains(b))
    {
        return Err(format!("Invalid UCI move {uci}").into());
    }
    let from = Position::new(u16::from(bytes[0] - b'a'), u16::from(bytes[1] - b'1'));
    let to = Position::new(u16::from(bytes[2] - b'a'), u16::from(bytes[3] - b'1'));
    let side = rules.side_to_move(timeline.current())?;
    let movement = rules
        .legal_moves_for_side_with_history(timeline.current(), Some(timeline.history()), side)?
        .into_iter()
        .find(|m| m.from == from && m.to == to)
        .ok_or("Opponent returned an illegal move")?;
    let input = if bytes.len() == 5 {
        let kind = promotion(bytes[4]).ok_or("Invalid UCI promotion")?;
        let choice = rules
            .move_choices(
                timeline.current(),
                Some(timeline.history()),
                movement,
                &StateMap::new(),
            )?
            .into_iter()
            .find(|c| {
                c.data.get("entity_type").and_then(StateValue::as_u64)
                    == Some(u64::from(kind.get()))
            })
            .ok_or("Illegal promotion")?;
        Some(ChoiceInput::from(&choice))
    } else {
        None
    };
    let mut turn = timeline.begin_turn(side.player())?;
    rules.execute_move(
        &mut turn,
        Some(timeline.history()),
        movement,
        input.as_ref(),
    )?;
    timeline.commit_turn(turn)?;
    Ok(rules.san_for_turn(
        timeline.history().previous_turn().unwrap(),
        &history_before(timeline),
    )?)
}
fn history_before(timeline: &GameTimeline) -> monumla_core::History {
    let mut before = timeline.clone();
    before.undo();
    before.history().clone()
}
fn uci_for_turn(
    rules: &ChessRules,
    timeline: &GameTimeline,
    next: &GameTimeline,
) -> Result<(String, String)> {
    let san = rules.san_for_turn(
        next.history().previous_turn().ok_or("No turn")?,
        timeline.history(),
    )?;
    let (m, promotion) = rules.resolve_san(timeline.current(), timeline.history(), &san)?;
    let mut uci = format!(
        "{}{}{}{}",
        char::from(b'a' + m.from.x as u8),
        m.from.y + 1,
        char::from(b'a' + m.to.x as u8),
        m.to.y + 1
    );
    if let Some(kind) = promotion {
        uci.push(if kind == QUEEN {
            'q'
        } else if kind == ROOK {
            'r'
        } else if kind == BISHOP {
            'b'
        } else if kind == KNIGHT {
            'n'
        } else {
            return Err("Unknown promotion".into());
        });
    }
    Ok((uci, san))
}
fn main() -> Result<()> {
    let c = Config::parse()?;
    fs::create_dir_all(&c.out)?;
    let mut opponent = Opponent::new(&c.stockfish)?;
    if let Some(elo) = c.elo {
        opponent.limit_strength(elo)?;
    }
    fs::write(c.out.join("uci-options.txt"), opponent.options.join("\n"))?;
    fs::write(
        c.out.join("settings.tsv"),
        format!(
            "opponent\t{}\nelo\t{:?}\nclock_ms\t{:?}\nincrement_ms\t{}\nmovetime_ms\t{}\n",
            opponent.name, c.elo, c.clock_ms, c.increment_ms, c.time_ms
        ),
    )?;
    let mut scores = fs::File::create(c.out.join("games.tsv"))?;
    writeln!(scores, "pair\tgame\tside\tscore\tresult\ttermination")?;
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    let revision = String::from_utf8_lossy(&revision.stdout).trim().to_owned();
    let dirty = !Command::new("git")
        .args(["status", "--porcelain"])
        .output()?
        .stdout
        .is_empty();
    let mut log = fs::File::create(c.out.join("moves.tsv"))?;
    writeln!(
        log,
        "game\tply\tengine\tcolor\tuci\tsan\tdepth\tnodes\telapsed_ms\tstop"
    )?;
    let openings = [
        "",
        "e2e4 e7e5 g1f3 b8c6 f1c4 g8f6",
        "d2d4 d7d5 c2c4 e7e6 b1c3 g8f6",
    ];
    let mut results = [0_usize; 4]; // Monumla wins, draws, losses, unfinished.
    let mut game_rows = String::new();
    for game in 0..c.games {
        opponent.new_game()?;
        let rules = ChessRules::standard();
        let mut timeline = rules.from_fen(STANDARD_FEN)?.timeline;
        let mut moves = Vec::new();
        for uci in openings[(game / 2) % openings.len()].split_whitespace() {
            apply_uci(&rules, &mut timeline, uci)?;
            moves.push(uci.to_owned());
        }
        let mut clocks = [c.clock_ms.unwrap_or(0); 2];
        let our_side = PlayerId::new(if game % 2 == 0 { 1 } else { 2 });
        let mut result = "*";
        let mut reason = "ply limit".to_owned();
        for ply in moves.len()..c.max_plies {
            let status = rules.status(timeline.current(), timeline.history())?;
            if let Some(outcome) = status.outcome {
                let outcome = outcome.as_game_outcome();
                result = if outcome.winners.contains(&PlayerId::new(1)) {
                    "1-0"
                } else if outcome.winners.contains(&PlayerId::new(2)) {
                    "0-1"
                } else {
                    "1/2-1/2"
                };
                reason = outcome.key;
                break;
            }
            let player = status.side_to_move.player();
            let side_index = if player.get() == 1 { 0 } else { 1 };
            let budget = if c.clock_ms.is_some() {
                if player == our_side {
                    (clocks[side_index] / 30 + c.increment_ms * 3 / 4)
                        .max(20)
                        .min(clocks[side_index].saturating_sub(20).max(1))
                } else {
                    clocks[side_index]
                }
            } else {
                c.time_ms
            };
            let start = Instant::now();
            let (uci, san, depth, nodes, stop) = if player == our_side {
                let mut registry = RuleRegistry::new();
                registry.register_outcome(rules.outcome_rule());
                let shared_rules = rules.clone();
                let domain = RuntimeDomain::new(registry, move |p| {
                    Ok(Rc::new(ChessInteractionRules::with_history(
                        &shared_rules,
                        p.history(),
                    )))
                });
                let report = analyze(
                    &domain,
                    &timeline,
                    player,
                    SearchOptions {
                        limits: SearchLimits {
                            max_depth: 32,
                            max_nodes: Some(50_000_000),
                        },
                        transposition_entries: 16384,
                        stop_on_forced_move: true,
                        late_move_reductions: true,
                        ..Default::default()
                    },
                    || start.elapsed() >= Duration::from_millis(budget),
                    |_| {},
                )?;
                let Some(action) = report.best_action else {
                    result = if our_side.get() == 1 { "0-1" } else { "1-0" };
                    reason = "Monumla forfeit: no completed search iteration".into();
                    break;
                };
                let next = domain.play(&timeline, &action)?;
                if let Some(outcome) = rules.status(next.current(), next.history())?.outcome {
                    if matches!(outcome, monumla_games_chess::ChessOutcome::ThreefoldRepetition | monumla_games_chess::ChessOutcome::FiftyMoveRule) {
                        timeline = next;
                        result = "1/2-1/2";
                        reason = outcome.as_game_outcome().key;
                        break;
                    }
                }
                let (uci, san) = uci_for_turn(&rules, &timeline, &next)?;
                timeline = next;
                (
                    uci,
                    san,
                    report.completed_depth,
                    report.nodes,
                    format!("{:?}", report.stop_reason),
                )
            } else {
                let (uci, depth, nodes) =
                    opponent.best(&moves, budget, c.clock_ms.map(|_| clocks), c.increment_ms)?;
                let san = apply_uci(&rules, &mut timeline, &uci)?;
                (uci, san, depth, nodes, "movetime".into())
            };
            if c.clock_ms.is_some() {
                let elapsed = start.elapsed().as_millis() as u64;
                if elapsed >= clocks[side_index] {
                    result = if player.get() == 1 { "0-1" } else { "1-0" };
                    reason = format!(
                        "{} time forfeit",
                        if player == our_side {
                            "Monumla"
                        } else {
                            "Opponent"
                        }
                    );
                    break;
                }
                clocks[side_index] = clocks[side_index] - elapsed + c.increment_ms;
            }
            let engine = if player == our_side {
                "Monumla"
            } else {
                &opponent.name
            };
            writeln!(
                log,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}",
                game + 1,
                ply + 1,
                engine,
                if player.get() == 1 { "white" } else { "black" },
                uci,
                san,
                depth,
                nodes,
                start.elapsed().as_secs_f64() * 1000.0,
                stop
            )?;
            log.flush()?;
            moves.push(uci);
        }
        if result == "*" {
            if let Some(outcome) = rules
                .status(timeline.current(), timeline.history())?
                .outcome
            {
                let outcome = outcome.as_game_outcome();
                result = if outcome.winners.contains(&PlayerId::new(1)) {
                    "1-0"
                } else if outcome.winners.contains(&PlayerId::new(2)) {
                    "0-1"
                } else {
                    "1/2-1/2"
                };
                reason = outcome.key;
            }
        }
        let our_win =
            (result == "1-0" && our_side.get() == 1) || (result == "0-1" && our_side.get() == 2);
        results[if result == "*" {
            3
        } else if result == "1/2-1/2" {
            1
        } else if our_win {
            0
        } else {
            2
        }] += 1;
        writeln!(
            scores,
            "{}\t{}\t{}\t{}\t{}\t{}",
            game / 2,
            game + 1,
            our_side.get(),
            if result == "*" {
                "NA"
            } else if result == "1/2-1/2" {
                "0.5"
            } else if our_win {
                "1"
            } else {
                "0"
            },
            result,
            reason
        )?;
        scores.flush()?;
        let mut tags = BTreeMap::new();
        tags.insert("Event".into(), "Monumla shared search benchmark".into());
        tags.insert(
            "White".into(),
            if our_side.get() == 1 {
                "Monumla".into()
            } else {
                opponent.name.clone()
            },
        );
        tags.insert(
            "Black".into(),
            if our_side.get() == 2 {
                "Monumla".into()
            } else {
                opponent.name.clone()
            },
        );
        tags.insert("Termination".into(), reason.clone());
        tags.insert("SearchBudgetMs".into(), c.time_ms.to_string());
        let mut pgn = rules.to_pgn_with_tags(STANDARD_FEN, timeline.history(), &tags)?;
        if result != "*" {
            pgn = pgn.replace("[Result \"*\"]", &format!("[Result \"{result}\"]"));
            if let Some(index) = pgn.rfind('*') {
                pgn.replace_range(index..index + 1, result);
            }
        }
        fs::write(c.out.join(format!("game-{:03}.pgn", game + 1)), pgn)?;
        game_rows.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            game + 1,
            if our_side.get() == 1 {
                "White"
            } else {
                "Black"
            },
            result,
            moves.len(),
            reason
        ));
        println!(
            "Game {}: Monumla {}, {result}, {} plies, {reason}",
            game + 1,
            if our_side.get() == 1 {
                "White"
            } else {
                "Black"
            },
            moves.len()
        );
    }
    let report = format!("# Monumla vs {}\n\nSource: `{revision}` (dirty workspace: {dirty}).\n\nEqual nominal search budget: {} ms/move; one thread each. Stockfish: Hash 16 MB, Skill Level 20, UCI_LimitStrength false, no pondering or opening book. Monumla: shared RuntimeDomain, depth cap 32, 50M-node cap, 16384 transposition slots, default quiescence and ordering, one PV. No game-specific evaluator or search. Matched opening pairs swap colors.\n\nMonumla: **{} wins, {} draws, {} losses, {} unfinished**. Unfinished games at the ply cap are not draws. This small sample is a baseline, not an Elo estimate. Deadlines are checked between nodes; generation and move/notation overhead can exceed the nominal budget. See moves.tsv for actual elapsed time and search depth.\n\n| Game | Monumla | Result | Plies | Termination |\n|---|---|---|---|---|\n{game_rows}", opponent.name, c.time_ms, results[0], results[1], results[2], results[3]);
    let report = if c.elo.is_some() || c.clock_ms.is_some() {
        format!("# Monumla vs {}\n\nSource `{revision}`, dirty: {dirty}.\nOpponent Elo setting: {:?}; clock {:?} ms + {} ms increment; fallback fixed budget {} ms/move. One thread each; Hash 16 MB; no pondering; paired openings. Monumla uses the shared RuntimeDomain, depth cap 32, 50M nodes, 16384 TT entries. Whole move time is charged in clock mode.\n\nW/D/L/unfinished: {:?}. Unfinished games are excluded, never draws. See games.tsv, settings.tsv and moves.tsv. An Elo setting is a reference pool label, not a measured human rating. Stockfish calibrates UCI_Elo at 120s+1s; other controls are uncalibrated. Use paired score intervals before drawing strength conclusions.\n\n| Game | Monumla | Result | Plies | Termination |\n|---|---|---|---|---|\n{game_rows}", opponent.name, c.elo, c.clock_ms, c.increment_ms, c.time_ms, results)
    } else {
        report
    };
    fs::write(c.out.join("report.md"), report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opponent_protocol_round_trips_special_moves_through_normal_rules() {
        for (fen, uci, san) in [
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1", "O-O"),
            ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 17", "e5d6", "exd6"),
            ("7k/P7/8/8/8/8/8/7K w - - 0 1", "a7a8n", "a8=N"),
        ] {
            let rules = ChessRules::standard();
            let mut timeline = rules.from_fen(fen).unwrap().timeline;
            let before = timeline.clone();
            assert_eq!(apply_uci(&rules, &mut timeline, uci).unwrap(), san);
            assert_eq!(
                uci_for_turn(&rules, &before, &timeline).unwrap(),
                (uci.into(), san.into())
            );
        }
    }
    #[test]
    fn rejects_illegal_and_malformed_opponent_moves_without_mutating_position() {
        let rules = ChessRules::standard();
        let initial = rules.from_fen(STANDARD_FEN).unwrap().timeline;
        for uci in ["", "a0a1", "i2i3", "e2e5", "e2e4q", "none"] {
            let mut timeline = initial.clone();
            assert!(apply_uci(&rules, &mut timeline, uci).is_err(), "{uci}");
            assert_eq!(timeline, initial);
        }
    }
}
