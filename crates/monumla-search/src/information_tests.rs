use super::*;
use std::cell::RefCell;

#[derive(Clone)]
struct Position {
    seed: u64,
    result: Option<i32>,
}
struct Hidden {
    sampled: RefCell<Vec<u64>>,
}

#[test]
fn incomplete_sample_decisions_use_only_a_public_legal_witness() {
    let domain = Hidden {
        sampled: RefCell::new(Vec::new()),
    };
    let result = analyze_information(
        &domain,
        &Position {
            seed: 99,
            result: None,
        },
        true,
        SearchOptions {
            decision_policy: DecisionPolicy::BestAvailable,
            limits: SearchLimits {
                max_depth: 1,
                max_nodes: Some(2),
            },
            ..Default::default()
        },
        &mut InformationMemory::new(),
        || false,
    )
    .unwrap();
    // The observation's first action is 0. The sampled hidden world orders 1
    // first and rewards it; no partial sampled score may select that action.
    assert_eq!(result.best_action, Some(0));
    assert_eq!(result.statistics.samples, 0);
    assert_eq!(result.score, None);
    assert!(result.variations.is_empty());
}
impl SearchDomain for Hidden {
    type Position = Position;
    type Action = usize;
    type Perspective = bool;
    type Error = &'static str;
    fn terminal_score(&self, _: &Position, _: bool) -> Result<Option<i32>, Self::Error> {
        Ok(None)
    }
    fn evaluate(&self, p: &Position, _: bool) -> Result<i32, Self::Error> {
        Ok(p.result.unwrap_or(0))
    }
    fn role(&self, _: &Position, _: bool) -> Result<NodeRole, Self::Error> {
        Ok(NodeRole::Maximize)
    }
    fn actions(&self, p: &Position) -> Result<Vec<usize>, Self::Error> {
        let mut actions = (0..8).collect::<Vec<_>>();
        actions.rotate_left(p.seed as usize % 8);
        Ok(actions)
    }
    fn play(&self, p: &Position, action: &usize) -> Result<Position, Self::Error> {
        Ok(Position {
            seed: p.seed,
            result: Some(if *action == p.seed as usize % 8 {
                100
            } else {
                0
            }),
        })
    }
    fn position_key(&self, _: &Position) -> Option<u128> {
        Some(1)
    }
}
impl InformationDomain for Hidden {
    fn observe(&self, p: &Position, _: bool) -> Result<Position, Self::Error> {
        Ok(Position {
            seed: 0,
            result: p.result,
        })
    }
    fn sample(&self, _: &Position, _: bool, seed: u64) -> Result<Position, Self::Error> {
        self.sampled.borrow_mut().push(seed);
        Ok(Position { seed, result: None })
    }
}
#[test]
fn interrupted_world_batches_resume_without_resampling_or_mixing_candidate_counts() {
    let domain = Hidden {
        sampled: RefCell::new(Vec::new()),
    };
    let root = Position {
        seed: 99,
        result: None,
    };
    let options = SearchOptions {
        limits: SearchLimits {
            max_depth: 1,
            max_nodes: Some(16),
        },
        multi_pv: 3,
        ..Default::default()
    };
    let mut whole = InformationMemory::new();
    let expected =
        analyze_information(&domain, &root, true, options, &mut whole, || false).unwrap();
    domain.sampled.borrow_mut().clear();
    let mut memory = InformationMemory::new();
    let mut last = None;
    for chunk in 0..8 {
        let result = analyze_information(
            &domain,
            &root,
            true,
            SearchOptions {
                limits: SearchLimits {
                    max_nodes: Some(2),
                    ..options.limits
                },
                ..options
            },
            &mut memory,
            || false,
        )
        .unwrap();
        assert_eq!(result.nodes, (chunk + 1) * 2);
        if chunk < 3 {
            assert_eq!(result.score, None);
            assert_eq!(result.best_action, None);
            assert!(memory.compare(&1).is_none());
        }
        assert_eq!(result.statistics.samples, (chunk + 1) / 4);
        last = Some(result);
    }
    assert_eq!(*domain.sampled.borrow(), [1, 2]);
    assert_eq!(last.unwrap(), expected);
}
#[test]
fn immediate_cancellation_does_not_invent_a_sample_or_action() {
    let domain = Hidden {
        sampled: RefCell::new(Vec::new()),
    };
    let result = analyze_information(
        &domain,
        &Position {
            seed: 0,
            result: None,
        },
        true,
        SearchOptions::default(),
        &mut InformationMemory::new(),
        || true,
    )
    .unwrap();
    assert_eq!(result.statistics.samples, 0);
    assert_eq!(result.best_action, None);
    assert_eq!(result.score, None);
    assert!(domain.sampled.borrow().is_empty());
}

#[test]
fn sampled_progress_only_publishes_complete_world_estimates() {
    let domain = Hidden {
        sampled: RefCell::new(Vec::new()),
    };
    let options = SearchOptions {
        limits: SearchLimits {
            max_depth: 1,
            max_nodes: Some(19),
        },
        report_partial: true,
        ..Default::default()
    };
    let mut reports = Vec::new();
    let result = analyze_information_with_progress(
        &domain,
        &Position {
            seed: 0,
            result: None,
        },
        true,
        options,
        &mut InformationMemory::new(),
        || false,
        |report| reports.push(report.clone()),
    )
    .unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].statistics.samples, 1);
    assert_eq!(reports[1].statistics.samples, 2);
    assert_eq!(reports[1].nodes, 16);
    assert_eq!(result.nodes, 19);
    assert_eq!(result.score, reports[1].score);
    assert_eq!(result.variations, reports[1].variations);
}
