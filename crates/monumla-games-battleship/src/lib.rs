//! Classic 10×10 Battleship: setup, shots and visibility through ordinary rules.
#![forbid(unsafe_code)]
mod fleet;
mod information;
mod sampling;
#[cfg(test)]
mod tests;

use fleet::{fleet, generate, install};
pub use fleet::{place_ship, rotate_ship, validate_fleet, FLEET};
pub use information::FleetInformation;
use monumla_core::*;

pub const FIRST: PlayerId = PlayerId::new(1);
pub const SECOND: PlayerId = PlayerId::new(2);
pub const DECK: EntityTypeId = EntityTypeId::new(1);
pub fn opponent(p: PlayerId) -> PlayerId {
    if p == FIRST {
        SECOND
    } else {
        FIRST
    }
}
pub fn offset(p: PlayerId) -> u16 {
    if p == FIRST {
        0
    } else {
        10
    }
}
pub fn number(s: &GameState, key: &str) -> u64 {
    s.ruleset_state
        .get(key)
        .and_then(StateValue::as_u64)
        .unwrap_or(0)
}
pub fn text<'a>(s: &'a GameState, key: &str) -> &'a str {
    s.ruleset_state
        .get(key)
        .and_then(StateValue::as_str)
        .unwrap_or("")
}
pub fn ready(s: &GameState, p: PlayerId) -> bool {
    number(s, &format!("ready.{}", p.get())) == 1
}
pub fn shot(s: &GameState, owner: PlayerId, cell: usize) -> &str {
    text(s, &format!("shot.{}.{}", owner.get(), cell))
}
pub fn phase(s: &GameState) -> &str {
    text(s, "phase")
}
pub fn coordinate(cell: usize) -> String {
    format!("{}{}", char::from(b'A' + (cell % 10) as u8), cell / 10 + 1)
}
fn reject(message: &str) -> InteractionError {
    InteractionError::RuleViolation(message.into())
}
fn rule_error(message: &str) -> RuleError {
    RuleError::Rejected(message.into())
}

pub fn initial_state(seed: u64) -> GameState {
    let mut s = GameState::new(20, 10).expect("valid oceans");
    for p in [FIRST, SECOND] {
        let mut player = PlayerState::new(p);
        player
            .state
            .insert("seed", seed.wrapping_add(u64::from(p.get()) * 0x9e3779b9));
        s.add_player(player).unwrap();
    }
    s.ruleset_state.insert("phase", "setup");
    s.set_active_players(vec![FIRST]).unwrap();
    s
}
pub fn registry() -> RuleRegistry {
    let mut r = RuleRegistry::new();
    r.register_entity(DECK, DeckRule).unwrap();
    r.register_outcome(FleetOutcome);
    r.set_information(FleetInformation::default());
    r.set_position_score(FleetScore);
    r
}
struct DeckRule;
impl EntityRule for DeckRule {
    fn presentation(&self, _: EntityRuleContext<'_>) -> Result<EntityPresentation, RuleError> {
        Ok(EntityPresentation::new("battleship/deck").with_label("Ship deck"))
    }
}
struct FleetOutcome;
impl OutcomeRule for FleetOutcome {
    fn evaluate(&self, c: RuleContext<'_>) -> Result<Option<GameOutcome>, RuleError> {
        let winner = number(c.state(), "winner");
        Ok((winner > 0).then(|| {
            let p = PlayerId::new(winner as u32);
            GameOutcome::new("battleship.fleet_sunk")
                .with_winner(p)
                .with_loser(opponent(p))
        }))
    }
}
struct FleetScore;
impl PositionScoreRule for FleetScore {
    fn score(&self, c: RuleContext<'_>, p: PlayerId) -> Result<i32, RuleError> {
        let setup = c.state().entities.values().filter(|e| e.owner == p).count() as i32 * 10
            + if ready(c.state(), p) { 1000 } else { 0 }
            - if ready(c.state(), opponent(p)) {
                1200
            } else {
                0
            };
        let damage = |owner| {
            (0..100)
                .filter(|i| matches!(shot(c.state(), owner, *i), "hit" | "sunk"))
                .count() as i32
        };
        Ok(setup + (damage(opponent(p)) - damage(p)) * 100)
    }
}
#[derive(Clone, Copy, Default)]
pub struct BattleshipRules;
impl InteractionRules for BattleshipRules {
    fn choices(
        &self,
        turn: &TurnSession,
        _: &StateMap,
    ) -> Result<Vec<ChoiceSpec>, InteractionError> {
        let s = &turn.working;
        let p = turn.actor;
        let mut out = Vec::new();
        if phase(s) == "finished" {
            return Ok(out);
        }
        if phase(s) == "setup" {
            let ships = fleet(s, p);
            for (index, &length) in FLEET.iter().enumerate() {
                if ships.iter().any(|(i, _)| *i == index) {
                    continue;
                }
                for cell in 0..100 {
                    for vertical in [false, true] {
                        if fleet::can_place(s, p, index, cell, vertical) {
                            let mut c = ChoiceSpec::position(Position::new(
                                offset(p) + (cell % 10) as u16,
                                (cell / 10) as u16,
                            ))
                            .with_label(format!("Place {length}-deck ship"))
                            .with_preview(
                                fleet::cells(cell, length, vertical)
                                    .unwrap()
                                    .into_iter()
                                    .map(|part| {
                                        Position::new(
                                            offset(p) + (part % 10) as u16,
                                            (part / 10) as u16,
                                        )
                                    }),
                            );
                            c.data.insert("ship", index as u64);
                            c.data.insert("vertical", vertical);
                            out.push(c);
                        }
                    }
                }
                // Place equal-length ships in a stable order; all fleet layouts remain possible.
                break;
            }
            for (i, _) in ships {
                out.push(ChoiceSpec::option(format!("remove:{i}")).with_label("Remove ship"));
                let mut rotated = s.clone();
                if rotate_ship(&mut rotated, p, i).is_ok() {
                    out.push(
                        ChoiceSpec::option(format!("rotate:{i}")).with_label("Rotate placed ship"),
                    );
                }
            }
            out.push(ChoiceSpec::option("arrange").with_label("Auto arrange"));
            if !s.entities.values().all(|e| e.owner != p) {
                out.push(ChoiceSpec::option("clear").with_label("Clear fleet"));
            }
            if validate_fleet(s, p).is_ok() {
                out.push(ChoiceSpec::option("ready").with_label("Ready"));
            }
        } else {
            let target = opponent(p);
            for cell in 0..100 {
                if shot(s, target, cell).is_empty() {
                    out.push(
                        ChoiceSpec::position(Position::new(
                            offset(target) + (cell % 10) as u16,
                            (cell / 10) as u16,
                        ))
                        .with_label(coordinate(cell)),
                    );
                }
            }
        }
        Ok(out)
    }
    fn apply_choice(
        &self,
        turn: &mut TurnSession,
        _: &mut StateMap,
        c: &Choice,
    ) -> Result<InteractionFlow, InteractionError> {
        if !turn.working.turn.active_players.contains(&turn.actor) {
            return Err(reject("Not your turn"));
        }
        // Also validate direct callers, rather than relying on frontend filtering.
        if !self
            .choices(turn, &StateMap::new())?
            .iter()
            .any(|x| ChoiceInput::from(x) == ChoiceInput::from(c))
        {
            return Err(reject("Illegal fleet action"));
        }
        let p = turn.actor;
        let mut next = turn.working.clone();
        let mut action = RecordedAction::new("battleship.setup");
        match (&c.kind, phase(&turn.working)) {
            (ChoiceKind::SelectPosition { position }, "setup") => {
                let i = c
                    .data
                    .get("ship")
                    .and_then(StateValue::as_u64)
                    .ok_or_else(|| reject("Missing ship"))? as usize;
                let vertical = c
                    .data
                    .get("vertical")
                    .and_then(StateValue::as_bool)
                    .unwrap_or(false);
                place_ship(
                    &mut next,
                    p,
                    i,
                    usize::from(position.y) * 10 + usize::from(position.x - offset(p)),
                    vertical,
                )
                .map_err(|e| reject(&e.to_string()))?;
                action.data.insert(
                    "notation",
                    format!(
                        "Place {} · {}",
                        FLEET[i],
                        coordinate(
                            usize::from(position.y) * 10 + usize::from(position.x - offset(p))
                        )
                    ),
                );
            }
            (ChoiceKind::SelectOption { key }, "setup") => {
                match key.as_str() {
                    "arrange" => {
                        let seed = next.players[&p]
                            .state
                            .get("seed")
                            .and_then(StateValue::as_u64)
                            .unwrap_or(1);
                        let ships =
                            generate(&next, p, seed, false).map_err(|e| reject(&e.to_string()))?;
                        install(&mut next, p, &ships).map_err(|e| reject(&e.to_string()))?;
                        next.players
                            .get_mut(&p)
                            .unwrap()
                            .state
                            .insert("seed", seed.wrapping_add(0x9e3779b97f4a7c15));
                    }
                    key if key.starts_with("rotate:") => {
                        let index = key[7..]
                            .parse::<usize>()
                            .map_err(|_| reject("Invalid ship"))?;
                        rotate_ship(&mut next, p, index).map_err(|e| reject(&e.to_string()))?;
                    }
                    "clear" => {
                        for id in next
                            .entities
                            .values()
                            .filter(|e| e.owner == p)
                            .map(|e| e.id)
                            .collect::<Vec<_>>()
                        {
                            next.remove_entity(id)?;
                        }
                    }
                    "ready" => {
                        validate_fleet(&next, p).map_err(|e| reject(&e.to_string()))?;
                        next.ruleset_state
                            .insert(format!("ready.{}", p.get()), 1_u64);
                        if ready(&next, opponent(p)) {
                            next.ruleset_state.insert("phase", "battle");
                            next.set_active_players(vec![FIRST])?;
                        } else {
                            next.set_active_players(vec![opponent(p)])?;
                        }
                    }
                    _ => {
                        let index = key
                            .strip_prefix("remove:")
                            .and_then(|x| x.parse::<u64>().ok())
                            .ok_or_else(|| reject("Unknown setup action"))?;
                        let ids = next
                            .entities
                            .values()
                            .filter(|e| {
                                e.owner == p
                                    && e.state.get("ship").and_then(StateValue::as_u64)
                                        == Some(index)
                            })
                            .map(|e| e.id)
                            .collect::<Vec<_>>();
                        for id in ids {
                            next.remove_entity(id)?;
                        }
                    }
                }
                action
                    .data
                    .insert("notation", c.label.clone().unwrap_or_else(|| key.clone()));
            }
            (ChoiceKind::SelectPosition { position }, "battle") => {
                let target = opponent(p);
                let cell = usize::from(position.y) * 10 + usize::from(position.x - offset(target));
                let entity = next.entity_at(*position)?.cloned();
                let mut result = "miss";
                if let Some(e) = entity {
                    next.entity_mut(e.id)?.state.insert("hit", true);
                    result = "hit";
                    let index = e.state.get("ship").and_then(StateValue::as_u64).unwrap();
                    let parts = next
                        .entities
                        .values()
                        .filter(|e| {
                            e.owner == target
                                && e.state.get("ship").and_then(StateValue::as_u64) == Some(index)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if parts
                        .iter()
                        .all(|e| e.state.get("hit").and_then(StateValue::as_bool) == Some(true))
                    {
                        result = "sunk";
                        for part in &parts {
                            let at = usize::from(part.position.y) * 10
                                + usize::from(part.position.x - offset(target));
                            next.ruleset_state
                                .insert(format!("shot.{}.{}", target.get(), at), "sunk");
                        }
                        for part in &parts {
                            for y in
                                part.position.y.saturating_sub(1)..=(part.position.y + 1).min(9)
                            {
                                for x in part.position.x.saturating_sub(1).max(offset(target))
                                    ..=(part.position.x + 1).min(offset(target) + 9)
                                {
                                    let at = usize::from(y) * 10 + usize::from(x - offset(target));
                                    if shot(&next, target, at).is_empty() {
                                        next.ruleset_state.insert(
                                            format!("shot.{}.{}", target.get(), at),
                                            "water",
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                next.ruleset_state
                    .insert(format!("shot.{}.{}", target.get(), cell), result);
                if (0..100)
                    .filter(|i| matches!(shot(&next, target, *i), "hit" | "sunk"))
                    .count()
                    == 20
                {
                    next.ruleset_state.insert("winner", u64::from(p.get()));
                    next.ruleset_state.insert("phase", "finished");
                } else if result == "miss" {
                    next.set_active_players(vec![target])?;
                }
                action.kind = "battleship.shot".into();
                action
                    .data
                    .insert("notation", format!("{} · {result}", coordinate(cell)));
                action.data.insert("result", result);
            }
            _ => return Err(reject("Invalid phase")),
        }
        turn.apply_step(action, |s| {
            *s = next;
            Ok(())
        })?;
        Ok(InteractionFlow::FinishTurn)
    }
}
pub fn turn_notation(turn: &TurnRecord) -> String {
    turn.steps
        .iter()
        .filter_map(|s| s.action.data.get("notation").and_then(StateValue::as_str))
        .collect::<Vec<_>>()
        .join(" → ")
}
