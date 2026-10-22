//! A native host for the generic search engine.
use monumla_core::GameTimeline;
use monumla_games_checkers::{search::DraughtsSearch, state_for_variant, DraughtsVariant, WHITE};
use monumla_games_chess::{search::ChessSearch, standard_chess_state};
use monumla_search::{search, SearchLimits};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let game = args.next().unwrap_or_else(|| "checkers".into());
    let max_depth = args.next().map(|s| s.parse()).transpose()?.unwrap_or(3);
    let max_nodes = args
        .next()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(10_000);
    let limits = SearchLimits {
        max_depth,
        max_nodes: Some(max_nodes),
    };
    match game.as_str() {
        "chess" => {
            let domain = ChessSearch::default();
            let position = match args.next() {
                Some(fen) => domain.rules().from_fen(&fen)?.timeline,
                None => GameTimeline::new(standard_chess_state()?)?,
            };
            let perspective = domain.rules().side_to_move(position.current())?;
            println!("{:#?}", search(&domain, &position, perspective, limits)?);
        }
        "checkers" | "checkers-international" => {
            let variant = if game == "checkers" { DraughtsVariant::Russian } else { DraughtsVariant::International };
            let position = GameTimeline::new(state_for_variant(variant))?;
            println!("{:#?}", search(&DraughtsSearch, &position, WHITE, limits)?);
        }
        _ => return Err("usage: analyze [chess|checkers|checkers-international] [depth] [nodes] [optional chess FEN]".into()),
    }
    Ok(())
}
