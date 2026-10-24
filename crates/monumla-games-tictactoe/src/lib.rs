//! Configurable rectangular alignment games, using the ordinary choice protocol.
#![forbid(unsafe_code)]
use monumla_core::*;
pub const CROSS: PlayerId = PlayerId::new(1);
pub const CIRCLE: PlayerId = PlayerId::new(2);
pub const MARK: EntityTypeId = EntityTypeId::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub width: u16,
    pub height: u16,
    pub connect: u16,
}
impl Config {
    pub fn new(width: u16, height: u16, connect: u16) -> Result<Self, RuleError> {
        if !(3..=9).contains(&width)
            || !(3..=9).contains(&height)
            || connect < 3
            || connect > width.max(height)
        {
            return Err(RuleError::Rejected(
                "Use 3–9 rows/columns and a reachable line of at least three".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            connect,
        })
    }
    pub fn standard() -> Self {
        Self {
            width: 3,
            height: 3,
            connect: 3,
        }
    }
}
pub fn initial_state(config: Config) -> Result<GameState, RuleError> {
    Config::new(config.width, config.height, config.connect)?;
    let mut s = GameState::new(config.width, config.height)?;
    s.add_player(PlayerState::new(CROSS))?;
    s.add_player(PlayerState::new(CIRCLE))?;
    s.ruleset_state.insert("connect", u64::from(config.connect));
    s.set_active_players(vec![CROSS])?;
    Ok(s)
}
pub fn connect(s: &GameState) -> u16 {
    s.ruleset_state
        .get("connect")
        .and_then(StateValue::as_u64)
        .unwrap_or(3) as u16
}
pub fn coordinate(p: Position) -> String {
    format!("{}{}", char::from(b'A' + p.x as u8), p.y + 1)
}
fn lines(s: &GameState) -> Vec<Vec<Position>> {
    let k = i32::from(connect(s));
    let mut out = Vec::new();
    for y in 0..s.board.height() {
        for x in 0..s.board.width() {
            for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                let end_x = i32::from(x) + (k - 1) * dx;
                let end_y = i32::from(y) + (k - 1) * dy;
                if end_x < 0
                    || end_y < 0
                    || end_x >= i32::from(s.board.width())
                    || end_y >= i32::from(s.board.height())
                {
                    continue;
                }
                out.push(
                    (0..k)
                        .map(|i| {
                            Position::new(
                                (i32::from(x) + i * dx) as u16,
                                (i32::from(y) + i * dy) as u16,
                            )
                        })
                        .collect(),
                );
            }
        }
    }
    out
}
pub fn winning_line(s: &GameState) -> Option<(PlayerId, Vec<Position>)> {
    for line in lines(s) {
        let owner = s.entity_at(line[0]).ok().flatten().map(|e| e.owner);
        if let Some(owner) = owner {
            if line.iter().all(|p| {
                s.entity_at(*p)
                    .ok()
                    .flatten()
                    .is_some_and(|e| e.owner == owner)
            }) {
                return Some((owner, line));
            }
        }
    }
    None
}
pub fn outcome(s: &GameState) -> Option<GameOutcome> {
    if let Some((p, _)) = winning_line(s) {
        Some(
            GameOutcome::new("tictactoe.line")
                .with_winner(p)
                .with_loser(if p == CROSS { CIRCLE } else { CROSS }),
        )
    } else if s.entities.len() == usize::from(s.board.width()) * usize::from(s.board.height()) {
        Some(GameOutcome::new("tictactoe.draw"))
    } else {
        None
    }
}
struct MarkRule;
impl EntityRule for MarkRule {
    fn presentation(&self, c: EntityRuleContext<'_>) -> Result<EntityPresentation, RuleError> {
        Ok(EntityPresentation::new(if c.entity().owner == CROSS {
            "tictactoe/cross"
        } else {
            "tictactoe/circle"
        })
        .with_label(if c.entity().owner == CROSS {
            "Cross"
        } else {
            "Circle"
        }))
    }
}
struct LineOutcome;
impl OutcomeRule for LineOutcome {
    fn evaluate(&self, c: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        Ok(outcome(c.state()))
    }
}
struct LineScore;
impl PositionScoreRule for LineScore {
    fn score(&self, c: RuleContext<'_>, p: PlayerId) -> Result<i32, RuleError> {
        let s = c.state();
        let mut value = 0;
        for line in lines(s) {
            let (mut own, mut other) = (0, 0);
            for pos in line {
                if let Some(e) = s.entity_at(pos)? {
                    if e.owner == p {
                        own += 1;
                    } else {
                        other += 1;
                    }
                }
            }
            if other == 0 {
                value += 4_i32.pow(own).min(4000)
            }
            if own == 0 {
                value -= 4_i32.pow(other).min(4000)
            }
        }
        Ok(value.clamp(-20000, 20000))
    }
}
pub fn registry() -> RuleRegistry {
    let mut r = RuleRegistry::new();
    r.register_entity(MARK, MarkRule).unwrap();
    r.register_outcome(LineOutcome);
    r.set_position_score(LineScore);
    r
}
#[derive(Clone, Copy, Default)]
pub struct TicTacToeRules;
impl InteractionRules for TicTacToeRules {
    fn choices(&self, t: &TurnSession, _: &StateMap) -> Result<Vec<ChoiceSpec>, InteractionError> {
        if outcome(&t.working).is_some() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        for y in 0..t.working.board.height() {
            for x in 0..t.working.board.width() {
                let p = Position::new(x, y);
                if t.working.entity_at(p)?.is_none() {
                    out.push(ChoiceSpec::position(p).with_label(coordinate(p)));
                }
            }
        }
        Ok(out)
    }
    fn apply_choice(
        &self,
        t: &mut TurnSession,
        _: &mut StateMap,
        c: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        let ChoiceKind::SelectPosition { position } = c.kind else {
            return Err(InteractionError::RuleViolation(
                "Select an empty square".into(),
            ));
        };
        if t.working.turn.active_players != vec![t.actor]
            || outcome(&t.working).is_some()
            || t.working.entity_at(position)?.is_some()
        {
            return Err(InteractionError::RuleViolation("Illegal placement".into()));
        }
        let mut a = RecordedAction::new("tictactoe.place");
        a.data.insert("notation", coordinate(position));
        let actor = t.actor;
        t.apply_step(a, |s| {
            let id = EntityId::new(s.entities.keys().map(|id| id.get()).max().unwrap_or(0) + 1);
            s.add_entity(EntityState::new(id, MARK, actor, position))?;
            s.set_active_players(vec![if actor == CROSS { CIRCLE } else { CROSS }])?;
            Ok(())
        })?;
        Ok(InteractionFlow::FinishTurn)
    }
}
pub fn turn_notation(t: &TurnRecord) -> String {
    t.steps
        .iter()
        .filter_map(|s| s.action.data.get("notation").and_then(StateValue::as_str))
        .collect::<Vec<_>>()
        .join(" → ")
}
#[cfg(test)]
mod tests;
