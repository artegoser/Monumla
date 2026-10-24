use super::*;
/// Every hidden completion is built from public results; the true enemy fleet
/// and both randomization seeds are removed before crossing the worker boundary.
pub struct FleetInformation;
impl InformationRule for FleetInformation {
    fn observe(&self, s: &GameState, viewer: PlayerId) -> Result<GameState, RuleError> {
        if !s.players.contains_key(&viewer) {
            return Err(rule_error("Unknown viewer"));
        }
        if let Some(previous) = s.ruleset_state.get("observer").and_then(StateValue::as_u64) {
            if previous != u64::from(viewer.get()) {
                return Err(rule_error(
                    "Cannot recover another player's private information",
                ));
            }
        }
        let mut view = s.clone();
        for id in view
            .entities
            .values()
            .filter(|e| e.owner != viewer && phase(s) != "finished")
            .map(|e| e.id)
            .collect::<Vec<_>>()
        {
            view.remove_entity(id)?;
        }
        for player in view.players.values_mut() {
            player.state = StateMap::new();
        }
        view.ruleset_state
            .insert("observer", u64::from(viewer.get()));
        Ok(view)
    }
    fn sample(&self, s: &GameState, viewer: PlayerId, seed: u64) -> Result<GameState, RuleError> {
        // Project again: even a caller accidentally passing authority cannot leak it.
        let mut world = self.observe(s, viewer)?;
        world.ruleset_state.remove("observer");
        if phase(s) == "battle" {
            let p = opponent(viewer);
            let ships = generate(&world, p, seed, true)?;
            install(&mut world, p, &ships)?;
            validate_fleet(&world, p)?;
        }
        Ok(world)
    }
}
