//! JSON-lines host for matched native versions; no game-specific search logic.
use monumla_search::{SearchLimits, SearchOptions};
use monumla_wasm::{experiments::Session, BestAction};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Request {
    Init {
        ruleset: String,
        pgn: Option<String>,
        #[serde(default)]
        seed: usize,
        #[serde(default)]
        opening_plies: usize,
    },
    Analyze {
        depth: u16,
        nodes: u64,
        time_ms: u64,
        #[serde(default)]
        play: bool,
    },
    Probe {
        label: String,
        depth: u16,
        nodes: u64,
        time_ms: u64,
    },
    Apply {
        action: BestAction,
    },
    Status,
    Observation,
}
fn execute(game: &mut Option<Session>, request: Request) -> Result<Value, String> {
    if let Request::Init {
        ruleset,
        pgn,
        seed,
        opening_plies,
    } = request
    {
        let mut session = Session::new_seeded(&ruleset, pgn.as_deref(), seed)?;
        let status = session.opening(seed, opening_plies)?;
        *game = Some(session);
        return serde_json::to_value(status).map_err(|e| e.to_string());
    }
    let session = game.as_mut().ok_or("Initialize first")?;
    let value = match request {
        Request::Analyze {
            depth,
            nodes,
            time_ms,
            play,
        } => serde_json::to_value(session.analyze(
            SearchOptions {
                decision_policy: if play {
                    monumla_search::DecisionPolicy::BestAvailable
                } else {
                    monumla_search::DecisionPolicy::CompletedIterations
                },
                limits: SearchLimits {
                    max_depth: depth,
                    max_nodes: Some(nodes),
                },
                transposition_entries: 16384,
                ..Default::default()
            },
            time_ms,
        )?),
        Request::Probe {
            label,
            depth,
            nodes,
            time_ms,
        } => serde_json::to_value(session.probe(
            &label,
            SearchOptions {
                limits: SearchLimits {
                    max_depth: depth,
                    max_nodes: Some(nodes),
                },
                transposition_entries: 16384,
                ..Default::default()
            },
            time_ms,
        )?),
        Request::Apply { action } => serde_json::to_value(session.apply(action)?),
        Request::Status => serde_json::to_value(session.status()?),
        Request::Observation => serde_json::to_value(session.search_observation()?),
        Request::Init { .. } => unreachable!(),
    };
    value.map_err(|e| e.to_string())
}
fn main() -> io::Result<()> {
    let mut game = None;
    let mut out = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let reply = serde_json::from_str(&line?)
            .map_err(|e| e.to_string())
            .and_then(|r| execute(&mut game, r));
        let reply = match reply {
            Ok(value) => json!({"ok": value}),
            Err(error) => json!({"error":error}),
        };
        serde_json::to_writer(&mut out, &reply)?;
        writeln!(out)?;
        out.flush()?;
    }
    Ok(())
}
