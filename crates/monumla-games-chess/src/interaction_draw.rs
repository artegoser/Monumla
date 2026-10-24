//! Draw claims are ordinary legal actions, available to every interaction client.
use crate::{ChessDrawClaim, ChessError, ChessRules, ChessStatus};
use monumla_core::{
    Choice, ChoiceKind, ChoiceSpec, GameState, History, RecordedAction, TurnSession,
};

const CLAIMS: [(&str, &str, ChessDrawClaim); 2] = [
    (
        "chess_draw_claim_threefold",
        "Claim threefold repetition",
        ChessDrawClaim::ThreefoldRepetition,
    ),
    (
        "chess_draw_claim_fifty_move",
        "Claim fifty-move draw",
        ChessDrawClaim::FiftyMoveRule,
    ),
];
pub(super) fn choices(
    rules: &ChessRules,
    state: &GameState,
    history: &History,
    status: &ChessStatus,
) -> Result<Vec<ChoiceSpec>, ChessError> {
    let mut out = Vec::new();
    for (key, label, claim) in CLAIMS {
        let current = match claim {
            ChessDrawClaim::ThreefoldRepetition => status.can_claim_threefold_repetition,
            ChessDrawClaim::FiftyMoveRule => status.can_claim_fifty_move_rule,
        };
        if current {
            out.push(ChoiceSpec::option(key).with_label(label));
            continue;
        }
        let possible = match claim {
            ChessDrawClaim::ThreefoldRepetition => {
                rules.has_repetition_claim_target(state, history)?
            }
            ChessDrawClaim::FiftyMoveRule => status.halfmove_clock == 99,
        };
        if !possible {
            continue;
        }
        for (movement, promotion) in rules.claimable_draw_moves(state, history, claim)? {
            let san = rules.san_for_move(state, history, movement, promotion)?;
            out.push(
                ChoiceSpec::option(format!("{key}_by_move:{san}"))
                    .with_label(format!("{label} with {san}")),
            );
        }
    }
    Ok(out)
}
pub(super) fn apply(
    rules: &ChessRules,
    history: &History,
    turn: &mut TurnSession,
    choice: &Choice,
) -> Result<bool, ChessError> {
    let ChoiceKind::SelectOption { key } = &choice.kind else {
        return Ok(false);
    };
    for (base, _, claim) in CLAIMS {
        let intended = key.strip_prefix(&format!("{base}_by_move:"));
        if key != base && intended.is_none() {
            continue;
        }
        let mut action = RecordedAction::new(if intended.is_some() {
            format!("{base}_by_move")
        } else {
            base.to_owned()
        });
        if let Some(san) = intended {
            action.data.insert("san", san.to_owned());
        }
        turn.apply_transaction(action, |transaction| -> Result<(), ChessError> {
            if let Some(san) = intended {
                let (movement, promotion) =
                    rules.resolve_san(transaction.raw_state_mut(), history, san)?;
                rules.claim_draw_after_move(
                    transaction.raw_state_mut(),
                    history,
                    claim,
                    movement,
                    promotion,
                )?;
            } else {
                rules.claim_draw(transaction.raw_state_mut(), history, claim)?;
            }
            Ok(())
        })?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChessInteractionRules;
    use monumla_core::InteractionDriver;
    #[test]
    fn current_and_intended_claims_replay_as_normal_actions_without_playing_the_move() {
        for pgn in [
            "1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 *",
            "1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 *",
        ] {
            let rules = ChessRules::standard();
            let mut timeline = rules.from_pgn(pgn).unwrap().timeline;
            let actor = rules.side_to_move(timeline.current()).unwrap().player();
            let before = timeline.current().clone();
            let mut driver = InteractionDriver::new(
                ChessInteractionRules::with_history(&rules, timeline.history()),
                timeline.begin_turn(actor).unwrap(),
            )
            .unwrap();
            let id = driver.interaction().choices.iter().find(|c| matches!(&c.kind, ChoiceKind::SelectOption{key} if key.starts_with("chess_draw_claim_threefold"))).unwrap().id;
            driver.choose(id).unwrap();
            timeline.commit_turn(driver.into_turn().unwrap()).unwrap();
            assert_eq!(timeline.current().entities, before.entities);
            assert_eq!(timeline.current().turn, before.turn);
            assert_eq!(
                rules
                    .status(timeline.current(), timeline.history())
                    .unwrap()
                    .outcome,
                Some(crate::ChessOutcome::ThreefoldRepetition)
            );
            assert_eq!(
                timeline
                    .history()
                    .previous_turn()
                    .unwrap()
                    .input
                    .choices
                    .len(),
                1
            );
        }
    }
}
