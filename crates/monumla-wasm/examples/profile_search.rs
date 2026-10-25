//! Profile shared search hooks on a recorded chess position, without an opponent.
use monumla_core::PlayerId;
use monumla_search::{analyze, NodeRole, Quiescence, SearchDomain, SearchLimits, SearchOptions};
use monumla_wasm::experiments::Session;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    time::{Duration, Instant},
};
struct Profile<'a, D> {
    domain: &'a D,
    times: RefCell<BTreeMap<&'static str, (usize, Duration)>>,
}
impl<D> Profile<'_, D> {
    fn measure<T>(&self, name: &'static str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = f();
        let mut times = self.times.borrow_mut();
        let entry = times.entry(name).or_default();
        entry.0 += 1;
        entry.1 += start.elapsed();
        result
    }
}
impl<D: SearchDomain> SearchDomain for Profile<'_, D> {
    type Position = D::Position;
    type Action = D::Action;
    type Perspective = D::Perspective;
    type Error = D::Error;
    fn prepare(&self, p: &D::Position) -> Result<(), D::Error> {
        self.measure("prepare", || self.domain.prepare(p))
    }
    fn terminal_score(&self, p: &D::Position, s: D::Perspective) -> Result<Option<i32>, D::Error> {
        self.domain.terminal_score(p, s)
    }
    fn terminal_score_at_ply(
        &self,
        p: &D::Position,
        s: D::Perspective,
        ply: u16,
    ) -> Result<Option<i32>, D::Error> {
        self.measure("outcome", || self.domain.terminal_score_at_ply(p, s, ply))
    }
    fn evaluate(&self, p: &D::Position, s: D::Perspective) -> Result<i32, D::Error> {
        self.measure("evaluate", || self.domain.evaluate(p, s))
    }
    fn role(&self, p: &D::Position, s: D::Perspective) -> Result<NodeRole, D::Error> {
        self.domain.role(p, s)
    }
    fn actions(&self, p: &D::Position) -> Result<Vec<D::Action>, D::Error> {
        self.measure("actions", || self.domain.actions(p))
    }
    fn play(&self, p: &D::Position, a: &D::Action) -> Result<D::Position, D::Error> {
        self.measure("play", || self.domain.play(p, a))
    }
    fn can_reduce(&self, p: &D::Position, a: &D::Action) -> bool {
        self.measure("reducible", || self.domain.can_reduce(p, a))
    }
    fn allows_stand_pat(&self, p: &D::Position) -> Result<Option<bool>, D::Error> {
        self.measure("quiet_witness", || self.domain.allows_stand_pat(p))
    }
    fn quiescence(&self, p: &D::Position) -> Result<Option<Quiescence<D::Action>>, D::Error> {
        self.measure("quiescence", || self.domain.quiescence(p))
    }
    fn position_key(&self, p: &D::Position) -> Option<u128> {
        self.measure("position_key", || self.domain.position_key(p))
    }
    fn action_key(&self, a: &D::Action) -> Option<u64> {
        self.measure("action_key", || self.domain.action_key(a))
    }
    fn action_priority(&self, p: &D::Position, a: &D::Action) -> i32 {
        self.measure("action_priority", || self.domain.action_priority(p, a))
    }
    fn score_to_cache(&self, s: i32, p: u16) -> i32 {
        self.domain.score_to_cache(s, p)
    }
    fn score_from_cache(&self, s: i32, p: u16) -> i32 {
        self.domain.score_from_cache(s, p)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let file = args
        .next()
        .ok_or("Usage: profile_search PGN_FILE [DEPTH] [NODES]")?;
    let depth = args.next().map(|s| s.parse()).transpose()?.unwrap_or(1);
    let nodes = args.next().map(|s| s.parse()).transpose()?.unwrap_or(10000);
    let opening_plies = args.next().map(|s| s.parse()).transpose()?.unwrap_or(0);
    let ruleset = if matches!(
        file.as_str(),
        "go" | "checkers" | "checkers-international" | "tictactoe"
    ) || file.starts_with("go:")
        || file.starts_with("tictactoe:")
    {
        file.as_str()
    } else {
        "chess"
    };
    let source = if file == "-" || ruleset != "chess" {
        "*".to_owned()
    } else {
        std::fs::read_to_string(&file)?
    };
    let mut source = source
        .lines()
        .filter(|l| !l.starts_with("[Result "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned();
    for ending in ["1/2-1/2", "1-0", "0-1", "*"] {
        if source.ends_with(ending) {
            source.truncate(source.len() - ending.len());
            break;
        }
    }
    source.push_str(" *");
    let mut session = Session::new(
        ruleset,
        if ruleset == "chess" {
            Some(&source)
        } else {
            None
        },
    )?;
    if opening_plies > 0 { session.opening(1, opening_plies)?; }
    let timeline = session.search_observation()?;
    let perspective: PlayerId = timeline.current().turn.active_players[0];
    let domain = Profile {
        domain: session.domain(),
        times: RefCell::default(),
    };
    let start = Instant::now();
    let report = analyze(
        &domain,
        &timeline,
        perspective,
        SearchOptions {
            limits: SearchLimits {
                max_depth: depth,
                max_nodes: Some(nodes),
            },
            transposition_entries: 16384,
            late_move_reductions: true,
            ..Default::default()
        },
        || false,
        |_| {},
    )?;
    println!(
        "depth={} nodes={} elapsed_ms={:.2} score={:?}",
        report.completed_depth,
        report.nodes,
        start.elapsed().as_secs_f64() * 1000.,
        report.score
    );
    println!("hook\tcalls\tms");
    for (name, (count, time)) in domain.times.borrow().iter() {
        println!("{name}\t{count}\t{:.3}", time.as_secs_f64() * 1000.);
    }
    Ok(())
}
