//! Enumerate and deduplicate complete ordinary interaction turns.
use super::*;

impl RuntimeDomain {
    pub(super) fn enumerate(
        &self,
        position: &GameTimeline,
        mut finish: impl FnMut(RuntimeAction) -> Result<bool, InteractionError>,
    ) -> Result<(), InteractionError> {
        let key = position.semantic_digest();
        let mut stack = vec![(self.driver(position)?, Vec::new())];
        let mut visited = HashSet::new();
        let mut outcomes = HashSet::new();
        while let Some((driver, path)) = stack.pop() {
            if path.len() > 128 {
                return Err(InteractionError::RuleViolation(
                    "Turn continuation exceeds the search safety limit".into(),
                ));
            }
            let steps: Vec<_> = driver
                .turn()
                .steps
                .iter()
                .map(|s| (&s.action.kind, &s.action.data))
                .collect();
            let draft_key = fingerprint(&(
                driver.turn().working.semantic_digest(),
                driver.draft(),
                steps,
            ));
            if !visited.insert(draft_key) {
                continue;
            }
            for choice in &driver.interaction().choices {
                if choice.purpose == monumla_core::ChoicePurpose::Presentation {
                    continue;
                }
                let mut child = driver.clone();
                let mut choices = path.clone();
                choices.push(ChoiceInput::from(choice));
                child.choose(choice.id)?;
                if child.is_finished() {
                    let turn = child.into_turn()?;
                    let semantic: Vec<_> = turn
                        .steps
                        .iter()
                        .map(|step| (&step.action.kind, &step.action.data))
                        .collect();
                    let outcome_key = fingerprint(&(turn.working.semantic_digest(), semantic));
                    if outcomes.insert(outcome_key)
                        && !finish(RuntimeAction {
                            input: TurnInput { choices },
                            position_key: Some(key),
                            turn: Some(Arc::new(turn)),
                        })?
                    {
                        return Ok(());
                    }
                } else {
                    stack.push((child, choices));
                }
            }
        }
        Ok(())
    }
    pub(super) fn losing_declaration(
        &self,
        position: &GameTimeline,
        action: &RuntimeAction,
    ) -> Result<bool, InteractionError> {
        let turn = action
            .turn
            .as_ref()
            .expect("enumerated action has a cached turn");
        if turn.before.entities != turn.working.entities {
            return Ok(false);
        }
        let outcome = self
            .registry
            .outcome(RuleContext::from_turn(turn, Some(position.history())))
            .map_err(|e| InteractionError::RuleViolation(e.to_string()))?;
        Ok(outcome.is_some_and(|o| o.losers.contains(&turn.actor)))
    }
    pub(super) fn cache_actions(
        &self,
        position: &GameTimeline,
        key: u128,
        results: Vec<RuntimeAction>,
    ) -> Result<Vec<RuntimeAction>, InteractionError> {
        // Immediate losing declarations (for example resignation) are dominated
        // when an ordinary legal continuation exists. This uses outcomes only.
        let mut losing = Vec::new();
        let mut live = Vec::new();
        for action in results {
            if self.losing_declaration(position, &action)? {
                losing.push(action);
            } else {
                live.push(action);
            }
        }
        let results = if live.is_empty() { losing } else { live };
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= 16 || cache.values().map(Vec::len).sum::<usize>() + results.len() > 2048 {
            cache.clear();
        }
        cache.insert(key, results.clone());
        Ok(results)
    }
}
