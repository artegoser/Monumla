//! Runtime glue shared by placement games; search remains in monumla-search.
use super::*;
use monumla_core::{PlayerId, RuleRegistry};
use monumla_games_battleship as sea;
use monumla_games_tictactoe as tic;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Battleship,
    TicTacToe,
}
pub(super) struct RulesRuntime {
    pub kind: Kind,
    pub timeline: GameTimeline,
    pub interaction: Option<InteractionDriver<Rc<dyn InteractionRules>>>,
    pub viewer: PlayerId,
    pub review_input: Option<monumla_core::TurnInput>,
}
impl RulesRuntime {
    pub fn new(kind: Kind, state: GameState) -> Result<Self, String> {
        let mut r = Self {
            kind,
            timeline: GameTimeline::new(state).map_err(string_error)?,
            interaction: None,
            viewer: PlayerId::new(1),
            review_input: None,
        };
        r.rebuild_interaction()?;
        Ok(r)
    }
    pub fn id(&self) -> &'static str {
        match self.kind {
            Kind::Battleship => "battleship",
            Kind::TicTacToe => "tictactoe",
        }
    }
    pub fn registry(&self) -> RuleRegistry {
        match self.kind {
            Kind::Battleship => sea::registry(),
            Kind::TicTacToe => tic::registry(),
        }
    }
    pub fn rules(&self) -> Rc<dyn InteractionRules> {
        match self.kind {
            Kind::Battleship => Rc::new(sea::BattleshipRules),
            Kind::TicTacToe => Rc::new(tic::TicTacToeRules),
        }
    }
    pub fn rebuild_interaction(&mut self) -> Result<(), String> {
        let s = self.timeline.current();
        let registry = self.registry();
        if registry
            .outcome(RuleContext::from_state(s, Some(self.timeline.history())))
            .map_err(string_error)?
            .is_some()
        {
            self.interaction = None;
            return Ok(());
        }
        let turn = self
            .timeline
            .begin_turn(active_actor(s, self.id())?)
            .map_err(string_error)?;
        self.interaction = Some(InteractionDriver::new(self.rules(), turn).map_err(string_error)?);
        Ok(())
    }
    pub fn choose(&mut self, id: ChoiceId) -> Result<(bool, Vec<StepRecord>), String> {
        let r = choose_driver(&mut self.interaction, &mut self.timeline, id)?;
        if r.0 {
            self.rebuild_interaction()?
        }
        Ok(r)
    }
    pub fn undo(&mut self) -> Result<(), String> {
        self.timeline.undo().ok_or("No turn to undo")?;
        self.rebuild_interaction()
    }
    pub fn redo(&mut self) -> Result<(), String> {
        self.timeline.redo().ok_or("No turn to redo")?;
        self.rebuild_interaction()
    }
    pub fn game_view(&self) -> Result<GameView, String> {
        let authoritative = visible_state(&self.timeline, self.interaction.as_ref());
        let registry = self.registry();
        let state = if let Some(info) = registry.information() {
            info.observe(authoritative, self.viewer)
                .map_err(string_error)?
        } else {
            authoritative.clone()
        };
        let outcome = registry
            .outcome(RuleContext::from_state(&state, None))
            .map_err(string_error)?;
        let mut details = state.ruleset_state.clone();
        details.insert("viewer", u64::from(self.viewer.get()));
        let title = match self.kind {
            Kind::Battleship => "Battleship".into(),
            Kind::TicTacToe => format!(
                "Tic-tac-toe · {}×{}",
                state.board.width(),
                state.board.height()
            ),
        };
        let status = if let Some(o) = &outcome {
            if o.winners.is_empty() {
                "Draw".into()
            } else {
                format!("Player {} wins", o.winners[0].get())
            }
        } else if self.kind == Kind::Battleship {
            if sea::phase(&state) == "setup" {
                format!(
                    "Player {} · place your fleet",
                    state.turn.active_players[0].get()
                )
            } else {
                format!("Player {} to fire", state.turn.active_players[0].get())
            }
        } else {
            format!(
                "{} to move · connect {}",
                if state.turn.active_players[0] == tic::CROSS {
                    "Cross"
                } else {
                    "Circle"
                },
                tic::connect(&state)
            )
        };
        Ok(GameView::new(
            self.id(),
            &title,
            self.id(),
            &state,
            presentation_entities(&registry, &state, self.timeline.history())?,
            status,
            outcome,
            None,
            details,
            self.timeline.can_undo(),
            self.timeline.can_redo(),
            None,
        ))
    }
    pub fn interaction_view(&self) -> InteractionView {
        if self.kind == Kind::Battleship
            && !self
                .timeline
                .current()
                .turn
                .active_players
                .contains(&self.viewer)
        {
            return InteractionView::default();
        }
        let Some(d) = &self.interaction else {
            return InteractionView::default();
        };
        interaction_view(
            d.interaction().generation,
            None,
            None,
            None,
            &d.interaction().choices,
        )
    }
    pub fn history_view(&self) -> Vec<HistoryTurnView> {
        self.timeline
            .history()
            .turns()
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let hidden = self.kind == Kind::Battleship
                    && t.actor != self.viewer
                    && t.steps.iter().any(|s| s.action.kind == "battleship.setup");
                HistoryTurnView {
                    index: i as u32,
                    actor: t.actor.get(),
                    turn_number: i as u32 + 1,
                    actor_label: format!("P{}", t.actor.get()),
                    notation: if hidden {
                        "Fleet preparation".into()
                    } else {
                        match self.kind {
                            Kind::Battleship => sea::turn_notation(t),
                            Kind::TicTacToe => tic::turn_notation(t),
                        }
                    },
                    actions: if hidden { vec![] } else { action_views(t) },
                }
            })
            .collect()
    }
}
