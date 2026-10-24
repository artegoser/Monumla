//! Review a played input against its pre-move observation, never the actual
//! hidden outcome. Lucky hits and unlucky misses do not determine move quality.
use super::*;
use monumla_core::TurnInput;
use monumla_search::InformationMemory;
impl GameHandle {
    pub(super) fn information_assessment(
        &self,
        input: TurnInput,
        mut options: SearchOptions,
        time_ms: f64,
    ) -> Result<MoveAssessment, String> {
        let mut report = MoveAssessment {
            samples: 0,
            depth: 1,
            best_score: None,
            played_score: None,
            loss: None,
            symbol: None,
            label: "Sampled estimate pending".into(),
            best_move: None,
        };
        if input.choices.is_empty() {
            report.label = "Game action".into();
            return Ok(report);
        }
        if monumla_games_battleship::phase(self.timeline().current()) == "setup" {
            report.label = "Fleet preparation".into();
            return Ok(report);
        }
        let domain = self.domain();
        let player = self.timeline().current().turn.active_players[0];
        let root = self.analysis_timeline()?;
        let mut memory = InformationMemory::new();
        options.limits.max_nodes = Some(100_000);
        options.multi_pv = 3;
        let deadline = now_ms() + time_ms;
        let result = monumla_search::analyze_information(
            &domain,
            &root,
            player,
            options,
            &mut memory,
            || now_ms() >= deadline,
        )
        .map_err(string_error)?;
        let action = RuntimeAction::from_input(input);
        let Some(comparison) = memory.compare(&action) else {
            return Err("Played input was not available in its observation".into());
        };
        let loss = comparison
            .best_score
            .saturating_sub(comparison.played_score)
            .max(0);
        report.samples = comparison.samples;
        report.best_score = Some(comparison.best_score);
        report.played_score = Some(comparison.played_score);
        report.loss = Some(loss);
        report.best_move = result.best_action.as_ref().map(|a| {
            a.input
                .choices
                .iter()
                .filter_map(|c| {
                    if let ChoiceKind::SelectPosition { position } = c.kind {
                        Some(monumla_games_battleship::coordinate(
                            usize::from(position.y) * 10 + usize::from(position.x % 10),
                        ))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" → ")
        });
        if !comparison.standard_error.is_finite() {
            report.label = "More samples needed".into();
            return Ok(report);
        }
        let difference = if comparison.is_best {
            comparison.margin
        } else {
            loss
        };
        // Block-based error is approximate for a finite Markov chain. Suppress
        // confident-sounding grades when differences do not exceed its noise.
        if f64::from(difference) <= 2.0 * comparison.standard_error {
            report.symbol = Some("·".into());
            report.label = "Comparable sampled move".into();
            return Ok(report);
        }
        let (symbol, label) = if comparison.is_best && comparison.margin >= 200 {
            ("!!", "Great move")
        } else if comparison.is_best && comparison.margin >= 80 {
            ("!", "Strong move")
        } else if comparison.is_best {
            ("✓", "Best move")
        } else if loss >= 250 {
            ("??", "Blunder")
        } else if loss >= 100 {
            ("?", "Mistake")
        } else if loss >= 40 {
            ("?!", "Inaccuracy")
        } else {
            ("·", "Good move")
        };
        report.symbol = Some(symbol.into());
        report.label = format!("{label} · sampled estimate");
        Ok(report)
    }
}
