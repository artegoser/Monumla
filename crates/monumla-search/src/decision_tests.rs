use super::*;

fn choice_tree(role: NodeRole) -> Cached {
    let sign = if role == NodeRole::Maximize { 1 } else { -1 };
    Cached(Tree(vec![
        node(role, 0, &[1, 2, 3]),
        node(role, 0, &[]),
        node(role, sign * 5, &[]),
        node(role, sign * 100, &[]),
    ]))
}
fn play_options(nodes: u64) -> SearchOptions {
    SearchOptions {
        decision_policy: DecisionPolicy::BestAvailable,
        limits: SearchLimits {
            max_depth: 1,
            max_nodes: Some(nodes),
        },
        ..Default::default()
    }
}

#[test]
fn incomplete_decisions_have_no_completed_score_or_analysis_ranking() {
    for role in [NodeRole::Maximize, NodeRole::Minimize] {
        let tree = choice_tree(role);
        let mut memory = SearchMemory::new(65536);
        let mut updates = Vec::new();
        let result = analyze_cached(
            &tree,
            &0,
            true,
            SearchOptions {
                report_partial: true,
                ..play_options(3)
            },
            &mut memory,
            || false,
            |report| updates.push(report.clone()),
        )
        .unwrap();
        assert_eq!(result.best_action, Some(2));
        assert_eq!(result.completed_depth, 0);
        assert_eq!(result.score, None);
        assert!(result.principal_variation.is_empty() && result.variations.is_empty());
        assert!(updates
            .iter()
            .all(|r| r.best_action.is_none() && r.score.is_none()));
        let resumed = analyze_cached(
            &tree,
            &0,
            true,
            SearchOptions {
                decision_policy: DecisionPolicy::CompletedIterations,
                ..play_options(10)
            },
            &mut memory,
            || false,
            |_| {},
        )
        .unwrap();
        assert_eq!(resumed.best_action, Some(3));
        assert_eq!(resumed.completed_depth, 1);
        assert_eq!(
            resumed.score,
            Some(if role == NodeRole::Maximize {
                100
            } else {
                -100
            })
        );
    }
}

#[test]
fn a_legal_witness_survives_a_tiny_budget_but_not_immediate_cancellation() {
    let tree = choice_tree(NodeRole::Maximize);
    let result = analyze(&tree, &0, true, play_options(1), || false, |_| {}).unwrap();
    assert_eq!(result.best_action, Some(1));
    assert_eq!(result.score, None);
    assert_eq!(result.nodes, 1);
    assert!(tree.play(&0, result.best_action.as_ref().unwrap()).is_ok());
    for immediate in [true, false] {
        let result = analyze(
            &tree,
            &0,
            true,
            play_options(if immediate { 100 } else { 0 }),
            || immediate,
            |_| {},
        )
        .unwrap();
        assert_eq!(result.best_action, None);
        assert_eq!(result.nodes, 0);
    }
    let terminal = analyze(&tree, &3, true, play_options(100), || false, |_| {}).unwrap();
    assert_eq!(terminal.best_action, None);
    assert_eq!(terminal.stop_reason, StopReason::Terminal);
}

#[test]
fn root_restrictions_never_return_an_unchecked_action() {
    let tree = choice_tree(NodeRole::Maximize);
    let calls = Cell::new(0);
    let result = analyze_action(
        &tree,
        &0,
        true,
        99,
        play_options(100),
        || {
            calls.set(calls.get() + 1);
            calls.get() >= 2
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(result.best_action, None);
    assert_eq!(
        analyze_action(&tree, &0, true, 99, play_options(100), || false, |_| {}).unwrap_err(),
        SearchError::NoActions
    );
}

#[test]
fn restricted_candidate_still_gets_evaluated_when_forced_move_shortcut_is_enabled() {
    let tree = Cached(Tree(vec![
        node(NodeRole::Maximize, 0, &[1]),
        node(NodeRole::Minimize, 5, &[]),
    ]));
    let result = analyze_action(
        &tree,
        &0,
        true,
        1,
        SearchOptions {
            stop_on_forced_move: true,
            ..play_options(100)
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.score, Some(5));
    assert_eq!(result.completed_depth, 1);
    assert_ne!(result.stop_reason, StopReason::ForcedMove);
}

#[test]
fn play_during_interrupted_multipv_preserves_the_primary_decision() {
    let tree = choice_tree(NodeRole::Maximize);
    let result = analyze(
        &tree,
        &0,
        true,
        SearchOptions {
            multi_pv: 3,
            ..play_options(6)
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.best_action, Some(3));
    assert_eq!(result.completed_depth, 0);
    assert_eq!(result.score, None);
    assert!(result.variations.is_empty());
}
