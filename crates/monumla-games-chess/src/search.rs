//! Standard chess adapter; evaluation and piece values stay in this game crate.
use crate::{
    ChessDrawClaim, ChessError, ChessRules, ChessSide, PseudoMove, BISHOP, KNIGHT, PAWN, QUEEN,
    ROOK,
};
use monumla_core::{
    search::{NodeRole, SearchDomain},
    ChoiceInput, EntityTypeId, GameTimeline, RecordedAction, StateMap,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ChessSearchAction {
    Move {
        movement: PseudoMove,
        continuation: Option<ChoiceInput>,
    },
    ClaimDraw {
        claim: ChessDrawClaim,
        intended: Option<(PseudoMove, Option<EntityTypeId>)>,
    },
}

#[derive(Default)]
pub struct ChessSearch {
    rules: ChessRules,
}
impl ChessSearch {
    pub fn rules(&self) -> &ChessRules {
        &self.rules
    }
}
impl SearchDomain for ChessSearch {
    type Position = GameTimeline;
    type Action = ChessSearchAction;
    type Perspective = ChessSide;
    type Error = ChessError;

    fn terminal_score(
        &self,
        position: &GameTimeline,
        perspective: ChessSide,
    ) -> Result<Option<i32>, ChessError> {
        Ok(self
            .rules
            .status(position.current(), position.history())?
            .outcome
            .map(|outcome| {
                let outcome = outcome.as_game_outcome();
                if outcome.winners.contains(&perspective.player()) {
                    100_000
                } else if outcome.losers.contains(&perspective.player()) {
                    -100_000
                } else {
                    0
                }
            }))
    }
    fn evaluate(&self, position: &GameTimeline, perspective: ChessSide) -> Result<i32, ChessError> {
        Ok(position
            .current()
            .entities
            .values()
            .map(|entity| {
                let value = match entity.entity_type {
                    PAWN => 100,
                    KNIGHT => 320,
                    BISHOP => 330,
                    ROOK => 500,
                    QUEEN => 900,
                    _ => 0,
                };
                if entity.owner == perspective.player() {
                    value
                } else {
                    -value
                }
            })
            .sum())
    }
    fn role(
        &self,
        position: &GameTimeline,
        perspective: ChessSide,
    ) -> Result<NodeRole, ChessError> {
        Ok(
            if self.rules.side_to_move(position.current())? == perspective {
                NodeRole::Maximize
            } else {
                NodeRole::Minimize
            },
        )
    }
    fn actions(&self, position: &GameTimeline) -> Result<Vec<ChessSearchAction>, ChessError> {
        let state = position.current();
        let history = position.history();
        let status = self.rules.status(state, history)?;
        if status.outcome.is_some() {
            return Ok(Vec::new());
        }
        let mut actions = Vec::new();
        for movement in self.rules.legal_moves_for_side_with_history(
            state,
            Some(history),
            status.side_to_move,
        )? {
            let local = self
                .rules
                .piece_move_choices(state, Some(history), movement)?;
            let choices =
                self.rules
                    .move_choices(state, Some(history), movement, &StateMap::new())?;
            if !local.is_empty() && choices.is_empty() {
                continue;
            }
            if choices.is_empty() {
                actions.push(ChessSearchAction::Move {
                    movement,
                    continuation: None,
                });
            }
            for choice in choices {
                actions.push(ChessSearchAction::Move {
                    movement,
                    continuation: Some(ChoiceInput::from(&choice)),
                });
            }
        }
        // Draw claims are optional actions, so a winning side can play on.
        for (claim, available) in [
            (
                ChessDrawClaim::ThreefoldRepetition,
                status.can_claim_threefold_repetition,
            ),
            (
                ChessDrawClaim::FiftyMoveRule,
                status.can_claim_fifty_move_rule,
            ),
        ] {
            if available {
                actions.push(ChessSearchAction::ClaimDraw {
                    claim,
                    intended: None,
                });
            }
            // Standard chess cannot repeat three times before this history length.
            let might_claim = match claim {
                ChessDrawClaim::ThreefoldRepetition => history.len() >= 6,
                ChessDrawClaim::FiftyMoveRule => status.halfmove_clock >= 99,
            };
            if !available && might_claim {
                for intended in self.rules.claimable_draw_moves(state, history, claim)? {
                    actions.push(ChessSearchAction::ClaimDraw {
                        claim,
                        intended: Some(intended),
                    });
                }
            }
        }
        // Stable capture-first ordering without game knowledge in the search core.
        actions.sort_by_key(|action| match action {
            ChessSearchAction::ClaimDraw { .. } => 0,
            ChessSearchAction::Move {
                movement,
                continuation,
            } => {
                if movement.capture.is_some() {
                    1
                } else if continuation.is_some() {
                    2
                } else {
                    3
                }
            }
        });
        Ok(actions)
    }
    fn play(
        &self,
        position: &GameTimeline,
        action: &ChessSearchAction,
    ) -> Result<GameTimeline, ChessError> {
        let state = position.current();
        let history = position.history();
        let side = self.rules.side_to_move(state)?;
        let mut turn = position.begin_turn(side.player())?;
        match action {
            ChessSearchAction::Move {
                movement,
                continuation,
            } => self.rules.execute_move(
                &mut turn,
                Some(history),
                *movement,
                continuation.as_ref(),
            )?,
            ChessSearchAction::ClaimDraw { claim, intended } => {
                turn.apply_transaction(
                    RecordedAction::new("chess.search_draw_claim"),
                    |tx| -> Result<(), ChessError> {
                        if let Some((movement, promotion)) = intended {
                            self.rules.claim_draw_after_move(
                                tx.raw_state_mut(),
                                history,
                                *claim,
                                *movement,
                                *promotion,
                            )?;
                        } else {
                            self.rules.claim_draw(tx.raw_state_mut(), history, *claim)?;
                        }
                        Ok(())
                    },
                )?;
            }
        }
        let mut child = position.clone();
        child.commit_turn(turn)?;
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monumla_core::search::{search, SearchLimits};
    fn limits(depth: u16) -> SearchLimits {
        SearchLimits {
            max_depth: depth,
            max_nodes: Some(10_000),
        }
    }

    #[test]
    fn same_core_search_finds_mate_without_mutating_root() {
        let domain = ChessSearch::default();
        let timeline = domain
            .rules
            .from_fen("7k/8/5KQ1/8/8/8/8/8 w - - 0 1")
            .unwrap()
            .timeline;
        let before = timeline.clone();
        let result = search(&domain, &timeline, ChessSide::White, limits(1)).unwrap();
        assert_eq!(result.score, Some(100_000));
        let child = domain
            .play(&timeline, &result.best_action.unwrap())
            .unwrap();
        assert!(matches!(
            domain
                .rules
                .status(child.current(), child.history())
                .unwrap()
                .outcome,
            Some(crate::ChessOutcome::Checkmate { .. })
        ));
        assert_eq!(timeline, before);
    }
    #[test]
    fn promotion_actions_include_every_choice_and_replay_to_correct_piece() {
        let domain = ChessSearch::default();
        let timeline = domain
            .rules
            .from_fen("7k/P7/8/8/8/8/8/7K w - - 0 1")
            .unwrap()
            .timeline;
        let actions = domain.actions(&timeline).unwrap();
        let promotions: Vec<_> = actions
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    ChessSearchAction::Move {
                        continuation: Some(_),
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(promotions.len(), 4);
        let result = search(&domain, &timeline, ChessSide::White, limits(1)).unwrap();
        let child = domain
            .play(&timeline, &result.best_action.unwrap())
            .unwrap();
        assert!(child
            .current()
            .entities
            .values()
            .any(|e| e.entity_type == QUEEN));
    }
    #[test]
    fn losing_side_can_choose_available_draw_instead_of_losing_material() {
        let domain = ChessSearch::default();
        let timeline = domain
            .rules
            .from_fen("7k/8/8/8/8/8/4q3/K7 w - - 100 60")
            .unwrap()
            .timeline;
        let result = search(&domain, &timeline, ChessSide::White, limits(1)).unwrap();
        assert_eq!(result.score, Some(0));
        assert!(matches!(
            result.best_action,
            Some(ChessSearchAction::ClaimDraw {
                claim: ChessDrawClaim::FiftyMoveRule,
                intended: None
            })
        ));
    }
    #[test]
    fn draw_claim_by_intended_move_does_not_execute_the_move() {
        let domain = ChessSearch::default();
        let timeline = domain
            .rules
            .from_fen("7k/8/8/8/8/8/4q3/K7 w - - 99 60")
            .unwrap()
            .timeline;
        let result = search(&domain, &timeline, ChessSide::White, limits(1)).unwrap();
        let action = result.best_action.unwrap();
        assert!(matches!(
            action,
            ChessSearchAction::ClaimDraw {
                intended: Some(_),
                ..
            }
        ));
        let child = domain.play(&timeline, &action).unwrap();
        assert_eq!(child.current().entities, timeline.current().entities);
        assert_eq!(
            domain.terminal_score(&child, ChessSide::White).unwrap(),
            Some(0)
        );
    }
    #[test]
    fn special_move_actions_reuse_authoritative_execution() {
        let domain = ChessSearch::default();
        let castling = domain
            .rules
            .from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1")
            .unwrap()
            .timeline;
        let castle_actions: Vec<_> = domain
            .actions(&castling)
            .unwrap()
            .into_iter()
            .filter(|action| {
                matches!(
                    action,
                    ChessSearchAction::Move {
                        movement: PseudoMove {
                            kind: crate::ChessMoveKind::Castle { .. },
                            ..
                        },
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(castle_actions.len(), 2);
        for action in castle_actions {
            let child = domain.play(&castling, &action).unwrap();
            assert_eq!(child.history().previous_turn().unwrap().steps.len(), 1);
            assert_eq!(
                domain.rules.side_to_move(child.current()).unwrap(),
                ChessSide::Black
            );
        }
        let en_passant = domain
            .rules
            .from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 17")
            .unwrap()
            .timeline;
        let action = domain
            .actions(&en_passant)
            .unwrap()
            .into_iter()
            .find(|action| {
                matches!(
                    action,
                    ChessSearchAction::Move {
                        movement: PseudoMove {
                            kind: crate::ChessMoveKind::EnPassant { .. },
                            ..
                        },
                        ..
                    }
                )
            })
            .unwrap();
        let child = domain.play(&en_passant, &action).unwrap();
        assert_eq!(
            child.current().entities.len() + 1,
            en_passant.current().entities.len()
        );
        assert_eq!(child.history().len(), en_passant.history().len() + 1);
    }

    #[test]
    fn standard_position_actions_match_legal_opening_moves() {
        let domain = ChessSearch::default();
        let timeline = GameTimeline::new(crate::standard_chess_state().unwrap()).unwrap();
        assert_eq!(domain.actions(&timeline).unwrap().len(), 20);
        let result = search(&domain, &timeline, ChessSide::White, limits(1)).unwrap();
        assert!(domain
            .actions(&timeline)
            .unwrap()
            .contains(&result.best_action.unwrap()));
    }
}
