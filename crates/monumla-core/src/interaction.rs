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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceSpec {
    pub kind: ChoiceKind,
    pub label: Option<String>,
    pub asset_key: Option<String>,
    pub data: StateMap,
}

impl ChoiceSpec {
    pub fn new(kind: ChoiceKind) -> Self {
        Self {
            kind,
            label: None,
            asset_key: None,
            data: StateMap::new(),
        }
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
        self.refresh()?;
        Ok(self.current.clone())
    }

    pub fn choose(&mut self, id: ChoiceId) -> Result<InteractionUpdate, InteractionError> {
        if self.finished {
            return Err(InteractionError::AlreadyFinished);
        }

        let choice = self.current.choice(id)?.clone();
        match self
            .rules
            .apply_choice(&mut self.turn, &mut self.draft, &choice)?
        {
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
