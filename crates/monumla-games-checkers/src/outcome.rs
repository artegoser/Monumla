use crate::{is_king, legal_moves, moves::forced, DraughtsVariant, BLACK, CHECKER, WHITE};
use monumla_core::{
    GameOutcome, GameState, OutcomeRule, PlayerId, RuleContext, RuleError, StateValue,
};

const QUIET: &str = "checkers.draw.quiet";
const BALANCE: &str = "checkers.draw.balance";
const SHORT: &str = "checkers.draw.short";
const LONG: &str = "checkers.draw.long";

fn count(state: &GameState, key: &str, player: PlayerId) -> u64 {
    state
        .ruleset_state
        .get(&format!("{key}.{}", player.get()))
        .and_then(StateValue::as_u64)
        .unwrap_or(0)
}
fn clock(
    before: &GameState,
    after: &mut GameState,
    key: &str,
    player: PlayerId,
    eligible: bool,
    continuing: bool,
) {
    for side in [WHITE, BLACK] {
        let value = if eligible && continuing {
            count(before, key, side).saturating_add(u64::from(side == player))
        } else {
            0
        };
        after
            .ruleset_state
            .insert(format!("{key}.{}", side.get()), value);
    }
}
fn pieces(state: &GameState, player: PlayerId) -> Vec<&monumla_core::EntityState> {
    state
        .entities
        .values()
        .filter(|e| e.owner == player && e.entity_type == CHECKER)
        .collect()
}
fn both_kings(state: &GameState) -> bool {
    [WHITE, BLACK]
        .into_iter()
        .all(|side| pieces(state, side).into_iter().any(is_king))
}
fn ending_side(state: &GameState, short: bool) -> Option<PlayerId> {
    let key = if short { SHORT } else { LONG };
    let black_first = state
        .ruleset_state
        .get(&format!("{key}.side"))
        .and_then(StateValue::as_u64)
        == Some(u64::from(BLACK.get()));
    let sides = if black_first {
        [(BLACK, WHITE), (WHITE, BLACK)]
    } else {
        [(WHITE, BLACK), (BLACK, WHITE)]
    };
    for (strong, weak) in sides {
        let a = pieces(state, strong);
        let b = pieces(state, weak);
        if b.len() != 1 || !is_king(b[0]) || a.is_empty() || !a.iter().any(|e| is_king(e)) {
            continue;
        }
        let russian = DraughtsVariant::from_state(state) == DraughtsVariant::Russian;
        let eligible = if short {
            a.len() <= 2 || (russian && a.len() == 3 && b[0].position.x == b[0].position.y)
        } else if russian {
            a.len() >= 3 && a.iter().all(|e| is_king(e))
        } else {
            a.len() <= 3
        };
        if eligible {
            return Some(strong);
        }
    }
    None
}

pub(crate) fn update_draw_counters(
    before: &GameState,
    after: &mut GameState,
    player: PlayerId,
    was_king: bool,
    capture: bool,
    promotion: bool,
) {
    clock(before, after, QUIET, player, was_king && !capture, true);
    let balance =
        !capture && !promotion && both_kings(after) && (4..=7).contains(&after.entities.len());
    // The move entering an ending establishes its starting position; count subsequent moves.
    clock(
        before,
        after,
        BALANCE,
        player,
        balance,
        both_kings(before) && before.entities.len() == after.entities.len(),
    );
    for (key, short) in [(SHORT, true), (LONG, false)] {
        let previous = ending_side(before, short);
        // Preserve the established stronger side when a capture leaves king vs king.
        if let Some(side) = previous {
            after
                .ruleset_state
                .insert(format!("{key}.side"), u64::from(side.get()));
        }
        let next = ending_side(after, short);
        clock(before, after, key, player, next.is_some(), next == previous);
        if let Some(side) = next {
            after
                .ruleset_state
                .insert(format!("{key}.side"), u64::from(side.get()));
        } else {
            after.ruleset_state.remove(&format!("{key}.side"));
        }
    }
}

fn position(state: &GameState) -> Vec<(u16, u16, u32, bool)> {
    let mut pieces: Vec<_> = state
        .entities
        .values()
        .filter(|e| e.entity_type == CHECKER)
        .map(|e| (e.position.x, e.position.y, e.owner.get(), is_king(e)))
        .collect();
    pieces.sort_unstable();
    pieces
}

pub struct CheckersOutcomeRule;
impl OutcomeRule for CheckersOutcomeRule {
    fn evaluate(&self, context: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        let state = context.state();
        // Partial capture steps are not positions at a turn boundary.
        if forced(state).is_some() {
            return Ok(None);
        }
        let player = match state.turn.active_players.as_slice() {
            [player] => *player,
            _ => return Ok(None),
        };
        if legal_moves(state, player)
            .map_err(|e| RuleError::Rejected(e.to_string()))?
            .is_empty()
        {
            return Ok(Some(
                GameOutcome::new("checkers.no_legal_moves")
                    .with_winner(if player == WHITE { BLACK } else { WHITE })
                    .with_loser(player),
            ));
        }
        if let Some(history) = context.history() {
            let current = position(state);
            // Every before-state is a distinct previous boundary; current is counted once.
            let repetitions = history
                .turns()
                .iter()
                .filter(|turn| {
                    turn.before.turn.active_players == state.turn.active_players
                        && position(&turn.before) == current
                })
                .count()
                + 1;
            if repetitions >= 3 {
                return Ok(Some(GameOutcome::new("checkers.threefold_repetition")));
            }
        }
        let russian = DraughtsVariant::from_state(state) == DraughtsVariant::Russian;
        for (key, limit, reason) in [
            (
                QUIET,
                if russian { 15 } else { 25 },
                "checkers.king_moves_draw",
            ),
            (SHORT, 5, "checkers.five_move_draw"),
            (
                LONG,
                if russian { 15 } else { 16 },
                "checkers.limited_ending_draw",
            ),
            (
                BALANCE,
                if state.entities.len() <= 5 { 30 } else { 60 },
                "checkers.material_balance_draw",
            ),
        ] {
            if key == BALANCE && !russian {
                continue;
            }
            let reached = if key == LONG && russian {
                ending_side(state, false).is_some_and(|strong| count(state, key, strong) >= limit)
            } else {
                [WHITE, BLACK]
                    .into_iter()
                    .all(|side| count(state, key, side) >= limit)
            };
            if reached {
                return Ok(Some(GameOutcome::new(reason)));
            }
        }
        Ok(None)
    }
}
