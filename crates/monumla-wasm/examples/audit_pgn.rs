//! Inspect repetition and terminal rules in a supplied game, without a search.
use monumla_core::History;
use monumla_games_chess::ChessRules;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args().nth(1).ok_or("Usage: audit_pgn PGN_FILE")?;
    let rules = ChessRules::standard();
    let imported = rules.from_pgn(&std::fs::read_to_string(file)?)?;
    let mut history = History::default();
    for (ply, turn) in imported.timeline.history().turns().iter().enumerate() {
        history = history.with_appended(turn.clone())?;
        let status = rules.status(&turn.after, &history)?;
        if status.repetition_count > 1 || status.outcome.is_some() {
            println!(
                "ply={} repetitions={} claimable={} outcome={:?}",
                ply + 1,
                status.repetition_count,
                status.can_claim_threefold_repetition,
                status.outcome
            );
        }
    }
    println!(
        "fen={}",
        rules.to_fen(imported.timeline.current(), imported.timeline.history())?
    );
    Ok(())
}
