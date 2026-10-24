use crate::{
    AbilityId, ChoiceId, CoreError, EntityId, Position, RecordedAction, StateMap, TurnSession,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChoiceKind {
    SelectEntity { entity: EntityId },
    SelectPosition { position: Position },
    SelectAbility { ability: AbilityId },
    SelectOption { key: String },
    FinishTurn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceInput {
    pub kind: ChoiceKind,
    pub data: StateMap,
}

/// Presentation choices only adjust a frontend draft. Every legal complete turn
/// must remain reachable without them; rules must not change game state here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoicePurpose {
    #[default]
    Action,
    Presentation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceSpec {
    #[serde(default)]
    pub purpose: ChoicePurpose,
    #[serde(default)]
    pub preview: Vec<Position>,
    pub kind: ChoiceKind,
    pub label: Option<String>,
    pub asset_key: Option<String>,
    pub data: StateMap,
}

impl ChoiceSpec {
    pub fn new(kind: ChoiceKind) -> Self {
        Self {
            kind,
            purpose: ChoicePurpose::Action,
            preview: Vec::new(),
            label: None,
            asset_key: None,
            data: StateMap::new(),
        }
    }

    pub fn for_presentation(mut self) -> Self {
        self.purpose = ChoicePurpose::Presentation;
        self
    }

    /// Geometry supplied by the rules for a legal action preview. It is not
    /// part of the replay input and cannot alter legality or search identity.
    pub fn with_preview(mut self, positions: impl IntoIterator<Item = Position>) -> Self {
        self.preview = positions.into_iter().collect();
        self
    }

    pub fn entity(entity: EntityId) -> Self {
        Self::new(ChoiceKind::SelectEntity { entity })
    }

    pub fn position(position: Position) -> Self {
        Self::new(ChoiceKind::SelectPosition { position })
    }

    pub fn ability(ability: AbilityId) -> Self {
        Self::new(ChoiceKind::SelectAbility { ability })
    }

    pub fn option(key: impl Into<String>) -> Self {
        Self::new(ChoiceKind::SelectOption { key: key.into() })
    }

    pub fn finish_turn() -> Self {
        Self::new(ChoiceKind::FinishTurn)
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_asset_key(mut self, asset_key: impl Into<String>) -> Self {
        self.asset_key = Some(asset_key.into());
        self
    }
}

impl From<&ChoiceSpec> for ChoiceInput {
    fn from(choice: &ChoiceSpec) -> Self {
        Self {
            kind: choice.kind.clone(),
            data: choice.data.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    #[serde(default)]
    pub purpose: ChoicePurpose,
    #[serde(default)]
    pub preview: Vec<Position>,
    pub id: ChoiceId,
    pub kind: ChoiceKind,
    pub label: Option<String>,
    pub asset_key: Option<String>,
    pub data: StateMap,
}

impl From<&Choice> for ChoiceInput {
    fn from(choice: &Choice) -> Self {
        Self {
            kind: choice.kind.clone(),
            data: choice.data.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Interaction {
    pub generation: u64,
    pub choices: Vec<Choice>,
}

impl Interaction {
    pub fn choice(&self, id: ChoiceId) -> Result<&Choice, InteractionError> {
        self.choices
            .iter()
            .find(|choice| choice.id == id)
            .ok_or(InteractionError::StaleOrInvalidChoice(id))
    }

    pub fn is_empty(&self) -> bool {
        self.choices.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChoiceIssuer {
    generation: u64,
    next_choice_id: u64,
}

impl ChoiceIssuer {
    pub fn issue(&mut self, specs: Vec<ChoiceSpec>) -> Interaction {
        self.generation = self.generation.wrapping_add(1).max(1);
        let generation = self.generation;

        let choices = specs
            .into_iter()
            .map(|spec| {
                self.next_choice_id = self.next_choice_id.wrapping_add(1).max(1);
                Choice {
                    id: ChoiceId::new(self.next_choice_id),
                    kind: spec.kind,
                    purpose: spec.purpose,
                    preview: spec.preview,
                    label: spec.label,
                    asset_key: spec.asset_key,
                    data: spec.data,
                }
            })
            .collect();

        Interaction {
            generation,
            choices,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionFlow {
    Continue,
    FinishTurn,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionUpdate {
    Continued(Interaction),
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InteractionError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("choice {0} is stale or invalid for the current interaction")]
    StaleOrInvalidChoice(ChoiceId),
    #[error("interaction has already finished")]
    AlreadyFinished,
    #[error("turn is not finished yet")]
    TurnNotFinished,
    #[error("rule rejected the interaction: {0}")]
    RuleViolation(String),
}

/// Rules generate frontend-facing choices from the current working state and
/// resolve a selected choice. A rule may mutate `turn.working` by recording a
/// step, or it may only update `draft` while collecting a multi-part input.
pub trait InteractionRules {
    fn choices(
        &self,
        turn: &TurnSession,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError>;

    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError>;
}

#[derive(Clone)]
pub struct InteractionDriver<R> {
    rules: R,
    turn: TurnSession,
    draft: StateMap,
    issuer: ChoiceIssuer,
    current: Interaction,
    finished: bool,
}

impl<R: InteractionRules> InteractionDriver<R> {
    pub fn new(rules: R, turn: TurnSession) -> Result<Self, InteractionError> {
        let mut driver = Self {
            rules,
            turn,
            draft: StateMap::new(),
            issuer: ChoiceIssuer::default(),
            current: Interaction::default(),
            finished: false,
        };
        driver.refresh()?;
        Ok(driver)
    }

    pub fn interaction(&self) -> &Interaction {
        &self.current
    }

    pub fn turn(&self) -> &TurnSession {
        &self.turn
    }

    pub fn draft(&self) -> &StateMap {
        &self.draft
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Clears transient interaction input without mutating the working game state.
    ///
    /// Frontends use this for UI-only cancellation such as deselecting a piece or
    /// abandoning an unfinished multi-part action. Choices are regenerated from
    /// the same authoritative working state with a fresh generation.
    pub fn reset_draft(&mut self) -> Result<Interaction, InteractionError> {
        if self.finished {
            return Err(InteractionError::AlreadyFinished);
        }
        self.draft = StateMap::new();
        if self.turn.steps.is_empty() {
            self.turn.input = crate::TurnInput::default();
        }
        self.refresh()?;
        Ok(self.current.clone())
    }

    pub fn choose(&mut self, id: ChoiceId) -> Result<InteractionUpdate, InteractionError> {
        if self.finished {
            return Err(InteractionError::AlreadyFinished);
        }

        let choice = self.current.choice(id)?.clone();
        let mut guard = (choice.purpose == ChoicePurpose::Presentation)
            .then(|| (self.turn.clone(), self.draft.clone()));
        let flow = match self
            .rules
            .apply_choice(&mut self.turn, &mut self.draft, &choice)
        {
            Ok(flow) => flow,
            Err(error) => {
                if let Some((before, draft)) = guard.take() {
                    self.turn = before;
                    self.draft = draft;
                }
                return Err(error);
            }
        };
        if let Some((before, draft)) = guard {
            if flow != InteractionFlow::Continue
                || self.turn.working != before.working
                || self.turn.steps != before.steps
            {
                self.turn = before;
                self.draft = draft;
                return Err(InteractionError::RuleViolation(
                    "Presentation choices cannot change game state or finish a turn".into(),
                ));
            }
        }
        self.turn.input.choices.push(ChoiceInput::from(&choice));
        match flow {
            InteractionFlow::Continue => {
                self.refresh()?;
                Ok(InteractionUpdate::Continued(self.current.clone()))
            }
            InteractionFlow::FinishTurn => {
                self.finished = true;
                self.current = self.issuer.issue(Vec::new());
                Ok(InteractionUpdate::Finished)
            }
        }
    }

    pub fn into_turn(self) -> Result<TurnSession, InteractionError> {
        if self.finished {
            Ok(self.turn)
        } else {
            Err(InteractionError::TurnNotFinished)
        }
    }

    fn refresh(&mut self) -> Result<(), InteractionError> {
        let choices = self.rules.choices(&self.turn, &self.draft)?;
        self.current = self.issuer.issue(choices);
        Ok(())
    }
}

/// Small helper for rules that want to record a named step without defining a
/// richer action record yet.
pub fn recorded_step(kind: impl Into<String>) -> RecordedAction {
    RecordedAction::new(kind)
}

impl<R: InteractionRules + ?Sized> InteractionRules for std::sync::Arc<R> {
    fn choices(
        &self,
        turn: &TurnSession,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        (**self).choices(turn, draft)
    }
    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        (**self).apply_choice(turn, draft, choice)
    }
}

impl<R: InteractionRules + ?Sized> InteractionRules for std::rc::Rc<R> {
    fn choices(
        &self,
        turn: &TurnSession,
        draft: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        (**self).choices(turn, draft)
    }
    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        draft: &mut StateMap,
        choice: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        (**self).apply_choice(turn, draft, choice)
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    use crate::{GameState, PlayerId, PlayerState};
    struct Focus {
        mutates: bool,
    }
    impl InteractionRules for Focus {
        fn choices(
            &self,
            _: &TurnSession,
            _: &StateMap,
        ) -> Result<Vec<ChoiceSpec>, InteractionError> {
            Ok(vec![ChoiceSpec::option("focus").for_presentation()])
        }
        fn apply_choice(
            &self,
            t: &mut TurnSession,
            draft: &mut StateMap,
            _: &Choice,
        ) -> Result<InteractionFlow, InteractionError> {
            draft.insert("focus", true);
            if self.mutates {
                t.working.ruleset_state.insert("illegal", true);
            }
            Ok(InteractionFlow::Continue)
        }
    }
    fn state() -> GameState {
        let mut s = GameState::new(1, 1).unwrap();
        s.add_player(PlayerState::new(PlayerId::new(1))).unwrap();
        s
    }
    #[test]
    fn presentation_only_updates_draft() {
        let s = state();
        let mut d = InteractionDriver::new(
            Focus { mutates: false },
            TurnSession::new(&s, PlayerId::new(1)).unwrap(),
        )
        .unwrap();
        let id = d.interaction().choices[0].id;
        d.choose(id).unwrap();
        assert_eq!(d.turn().working, s);
        assert!(d.draft().contains_key("focus"));
    }
    #[test]
    fn presentation_mutation_is_rejected_and_rolled_back() {
        let s = state();
        let mut d = InteractionDriver::new(
            Focus { mutates: true },
            TurnSession::new(&s, PlayerId::new(1)).unwrap(),
        )
        .unwrap();
        let id = d.interaction().choices[0].id;
        assert!(d.choose(id).is_err());
        assert_eq!(d.turn().working, s);
        assert!(!d.draft().contains_key("focus"));
    }
}
