use crate::{
    CoreError, EntityId, EntityState, GameState, PlayerId, PresentationCue, StateDelta, StateMap,
    Transaction,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecordedAction {
    pub kind: String,
    pub data: StateMap,
}

impl RecordedAction {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            data: StateMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    pub before: GameState,
    pub after: GameState,
    pub action: RecordedAction,
    pub delta: StateDelta,
    pub presentation: Vec<PresentationCue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub actor: PlayerId,
    pub before: GameState,
    pub steps: Vec<StepRecord>,
    pub after: GameState,
    #[serde(default)]
    pub synthetic: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    turns: Vec<TurnRecord>,
}

impl History {
    pub fn turns(&self) -> &[TurnRecord] {
        &self.turns
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    pub fn previous_turn(&self) -> Option<&TurnRecord> {
        self.turns.last()
    }

    pub fn last_step(&self) -> Option<&StepRecord> {
        self.turns.last().and_then(|turn| turn.steps.last())
    }

    /// Returns a state at a committed turn boundary.
    ///
    /// Boundary `0` is the state before the first recorded turn. Boundary `n`
    /// is the state after turn `n - 1`.
    pub fn state_at_turn_boundary(&self, boundary: usize) -> Option<&GameState> {
        if boundary == 0 {
            return self.turns.first().map(|turn| &turn.before);
        }
        self.turns.get(boundary - 1).map(|turn| &turn.after)
    }

    pub fn entity_at_turn_boundary(
        &self,
        entity: EntityId,
        boundary: usize,
    ) -> Option<&EntityState> {
        self.state_at_turn_boundary(boundary)
            .and_then(|state| state.entities.get(&entity))
    }

    /// Returns a cloned history with one contiguous committed turn appended.
    /// Useful for rules that need to evaluate the immediate consequences of a
    /// candidate completed turn without mutating the real game timeline.
    pub fn with_appended(&self, turn: TurnRecord) -> Result<Self, CoreError> {
        if let Some(previous) = self.turns.last() {
            if previous.after != turn.before {
                return Err(CoreError::TurnStateMismatch);
            }
        }
        let mut next = self.clone();
        next.turns.push(turn);
        Ok(next)
    }

    fn push(&mut self, turn: TurnRecord) {
        self.turns.push(turn);
    }

    fn pop(&mut self) -> Option<TurnRecord> {
        self.turns.pop()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionResult<T> {
    pub value: T,
    pub delta: StateDelta,
    pub presentation: Vec<PresentationCue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnSession {
    pub actor: PlayerId,
    pub before: GameState,
    pub working: GameState,
    pub steps: Vec<StepRecord>,
    synthetic: bool,
}

impl TurnSession {
    pub fn new(state: &GameState, actor: PlayerId) -> Result<Self, CoreError> {
        if !state.players.contains_key(&actor) {
            return Err(CoreError::TurnActorNotFound(actor));
        }
        Ok(Self {
            actor,
            before: state.clone(),
            working: state.clone(),
            steps: Vec::new(),
            synthetic: false,
        })
    }

    pub fn mark_synthetic(&mut self) {
        self.synthetic = true;
    }

    pub fn is_synthetic(&self) -> bool {
        self.synthetic
    }

    pub fn apply_step<T>(
        &mut self,
        action: RecordedAction,
        operation: impl FnOnce(&mut GameState) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let result =
            self.apply_transaction(action, |transaction| operation(transaction.raw_state_mut()))?;
        Ok(result.value)
    }

    pub fn apply_transaction<T, E>(
        &mut self,
        action: RecordedAction,
        operation: impl FnOnce(&mut Transaction) -> Result<T, E>,
    ) -> Result<TransactionResult<T>, E>
    where
        E: From<CoreError>,
    {
        let before = self.working.clone();
        let mut transaction = Transaction::new(&before);
        let value = operation(&mut transaction)?;
        let outcome = transaction.finish().map_err(E::from)?;

        self.working = outcome.state.clone();
        self.steps.push(StepRecord {
            before,
            after: outcome.state,
            action,
            delta: outcome.delta.clone(),
            presentation: outcome.presentation.clone(),
        });

        Ok(TransactionResult {
            value,
            delta: outcome.delta,
            presentation: outcome.presentation,
        })
    }

    /// Creates a validated candidate state without recording it as a real step.
    pub fn speculate<T>(
        &self,
        operation: impl FnOnce(&mut GameState) -> Result<T, CoreError>,
    ) -> Result<(GameState, T), CoreError> {
        self.working.speculate(operation)
    }

    pub fn rollback(self) -> GameState {
        self.before
    }

    fn into_record(self) -> TurnRecord {
        TurnRecord {
            actor: self.actor,
            before: self.before,
            steps: self.steps,
            after: self.working,
            synthetic: self.synthetic,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameTimeline {
    current: GameState,
    history: History,
    redo: Vec<TurnRecord>,
}

impl GameTimeline {
    pub fn new(current: GameState) -> Result<Self, CoreError> {
        current.validate()?;
        Ok(Self {
            current,
            history: History::default(),
            redo: Vec::new(),
        })
    }

    pub fn current(&self) -> &GameState {
        &self.current
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn begin_turn(&self, actor: PlayerId) -> Result<TurnSession, CoreError> {
        TurnSession::new(&self.current, actor)
    }

    pub fn commit_turn(&mut self, session: TurnSession) -> Result<&TurnRecord, CoreError> {
        if session.before != self.current {
            return Err(CoreError::TurnStateMismatch);
        }
        session.working.validate()?;

        let record = session.into_record();
        self.current = record.after.clone();
        self.redo.clear();
        self.history.push(record);
        Ok(self.history.previous_turn().expect("turn was just pushed"))
    }

    pub fn undo(&mut self) -> Option<&GameState> {
        let record = self.history.pop()?;
        self.current = record.before.clone();
        self.redo.push(record);
        Some(&self.current)
    }

    pub fn redo(&mut self) -> Option<&GameState> {
        let record = self.redo.pop()?;
        if record.before != self.current {
            self.redo.push(record);
            return None;
        }
        self.current = record.after.clone();
        self.history.push(record);
        Some(&self.current)
    }

    pub fn state_turns_ago(&self, turns_ago: usize) -> Option<&GameState> {
        if turns_ago == 0 {
            return Some(&self.current);
        }
        if turns_ago > self.history.len() {
            return None;
        }
        let boundary = self.history.len() - turns_ago;
        self.history.state_at_turn_boundary(boundary)
    }

    pub fn entity_turns_ago(&self, entity: EntityId, turns_ago: usize) -> Option<&EntityState> {
        self.state_turns_ago(turns_ago)
            .and_then(|state| state.entities.get(&entity))
    }
}
