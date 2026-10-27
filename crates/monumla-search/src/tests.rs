use super::*;
use std::cell::Cell;
#[path = "decision_tests.rs"]
mod decisions;
#[path = "proof_tests.rs"]
mod proofs;

struct Node {
    role: NodeRole,
    score: i32,
    terminal: bool,
    children: Vec<usize>,
}
struct Tree(Vec<Node>);
impl SearchDomain for Tree {
    type Position = usize;
    type Action = usize;
    type Perspective = bool;
    type Error = &'static str;
    fn terminal_score(&self, p: &usize, perspective: bool) -> Result<Option<i32>, Self::Error> {
        Ok(self.0[*p].terminal.then_some(if perspective {
            self.0[*p].score
        } else {
            -self.0[*p].score
        }))
    }
    fn evaluate(&self, p: &usize, perspective: bool) -> Result<i32, Self::Error> {
        Ok(if perspective {
            self.0[*p].score
        } else {
            -self.0[*p].score
        })
    }
    fn role(&self, p: &usize, perspective: bool) -> Result<NodeRole, Self::Error> {
        Ok(if perspective {
            self.0[*p].role
        } else {
            match self.0[*p].role {
                NodeRole::Maximize => NodeRole::Minimize,
                NodeRole::Minimize => NodeRole::Maximize,
            }
        })
    }
    fn actions(&self, p: &usize) -> Result<Vec<usize>, Self::Error> {
        Ok(self.0[*p].children.clone())
    }
    fn play(&self, p: &usize, action: &usize) -> Result<usize, Self::Error> {
        if self.0[*p].children.contains(action) {
            Ok(*action)
        } else {
            Err("illegal edge")
        }
    }
}
fn node(role: NodeRole, score: i32, children: &[usize]) -> Node {
    Node {
        role,
        score,
        terminal: children.is_empty(),
        children: children.to_vec(),
    }
}
fn trap() -> Tree {
    use NodeRole::*;
    Tree(vec![
        node(Maximize, 0, &[1, 2]),
        node(Minimize, 9, &[3, 4]),
        node(Minimize, 3, &[5, 6]),
        node(Maximize, -8, &[]),
        node(Maximize, 8, &[]),
        node(Maximize, 2, &[]),
        node(Maximize, 3, &[]),
    ])
}
fn limits(depth: u16) -> SearchLimits {
    SearchLimits {
        max_depth: depth,
        max_nodes: None,
    }
}

#[test]
fn sees_past_greedy_move_and_returns_replayable_principal_variation() {
    let tree = trap();
    assert_eq!(
        search(&tree, &0, true, limits(1)).unwrap().best_action,
        Some(1)
    );
    let result = search(&tree, &0, true, limits(2)).unwrap();
    assert_eq!(result.best_action, Some(2));
    assert_eq!(result.score, Some(2));
    assert_eq!(result.principal_variation, [2, 5]);
    let mut position = 0;
    for action in result.principal_variation {
        position = tree.play(&position, &action).unwrap();
    }
    assert_eq!(tree.terminal_score(&position, true).unwrap(), result.score);
}
#[test]
fn supports_extra_turns_without_flipping_score_or_role_implicitly() {
    use NodeRole::*;
    let tree = Tree(vec![
        node(Maximize, 0, &[1, 2]),
        node(Maximize, 0, &[3, 4]),
        node(Minimize, 0, &[5, 6]),
        node(Minimize, 1, &[]),
        node(Minimize, 10, &[]),
        node(Maximize, 3, &[]),
        node(Maximize, 4, &[]),
    ]);
    let result = search(&tree, &0, true, limits(2)).unwrap();
    assert_eq!(result.score, Some(10));
    assert_eq!(result.principal_variation, [1, 4]);
}
#[test]
fn fixed_perspective_can_start_at_minimizing_node() {
    let mut tree = trap();
    tree.0[0].role = NodeRole::Minimize;
    let result = search(&tree, &0, true, limits(2)).unwrap();
    assert_eq!(result.score, Some(-8));
    assert_eq!(result.best_action, Some(1));
    let reverse = search(&tree, &0, false, limits(2)).unwrap();
    assert_eq!(reverse.score, Some(8));
    assert_eq!(reverse.best_action, result.best_action);
}
#[test]
fn zero_depth_and_terminal_roots_have_no_action() {
    let tree = trap();
    let result = search(&tree, &0, true, limits(0)).unwrap();
    assert_eq!(result.completed_depth, 0);
    assert_eq!(result.score, Some(0));
    assert_eq!(result.best_action, None);
    assert_eq!(result.nodes, 1);
    let result = search(&tree, &3, true, limits(9)).unwrap();
    assert_eq!(result.score, Some(-8));
    assert_eq!(result.nodes, 1);
    assert_eq!(result.stop_reason, StopReason::Terminal);
}
#[test]
fn interruption_preserves_last_complete_iteration() {
    let result = search(
        &trap(),
        &0,
        true,
        SearchLimits {
            max_depth: 4,
            max_nodes: Some(4),
        },
    )
    .unwrap();
    assert_eq!(result.stop_reason, StopReason::NodeLimit);
    assert_eq!(result.completed_depth, 1);
    assert_eq!(result.score, Some(9));
    assert_eq!(result.best_action, Some(1));
    assert_eq!(result.nodes, 4);
}
#[test]
fn node_budgets_are_hard_and_empty_budget_does_not_invent_result() {
    for budget in 0..30 {
        let result = search(
            &trap(),
            &0,
            true,
            SearchLimits {
                max_depth: 3,
                max_nodes: Some(budget),
            },
        )
        .unwrap();
        assert!(result.nodes <= budget);
        if budget == 0 {
            assert_eq!(result.score, None);
            assert_eq!(result.best_action, None);
        }
    }
}
#[test]
fn cancellation_is_checked_at_each_node_and_preserves_completed_result() {
    let calls = Cell::new(0);
    let result = search_with_control(&trap(), &0, true, limits(3), || {
        calls.set(calls.get() + 1);
        calls.get() > 4
    })
    .unwrap();
    assert_eq!(result.stop_reason, StopReason::Cancelled);
    assert_eq!(result.completed_depth, 1);
    assert_eq!(result.nodes, 4);
    assert_eq!(result.best_action, Some(1));
}
#[test]
fn rejects_nonterminal_nodes_without_actions_and_invalid_depth() {
    let mut tree = trap();
    tree.0[0].children.clear();
    assert_eq!(
        search(&tree, &0, true, limits(1)),
        Err(SearchError::NoActions)
    );
    assert_eq!(
        search(&tree, &0, true, limits(257)),
        Err(SearchError::InvalidDepth)
    );
}
#[test]
fn domain_errors_are_propagated() {
    struct Broken(Tree);
    impl SearchDomain for Broken {
        type Position = usize;
        type Action = usize;
        type Perspective = bool;
        type Error = &'static str;
        fn terminal_score(&self, p: &usize, v: bool) -> Result<Option<i32>, Self::Error> {
            self.0.terminal_score(p, v)
        }
        fn evaluate(&self, _: &usize, _: bool) -> Result<i32, Self::Error> {
            Err("evaluation failed")
        }
        fn role(&self, p: &usize, v: bool) -> Result<NodeRole, Self::Error> {
            self.0.role(p, v)
        }
        fn actions(&self, p: &usize) -> Result<Vec<usize>, Self::Error> {
            self.0.actions(p)
        }
        fn play(&self, _: &usize, _: &usize) -> Result<usize, Self::Error> {
            Err("transition failed")
        }
    }
    let domain = Broken(trap());
    assert_eq!(
        search(&domain, &0, true, limits(0)),
        Err(SearchError::Domain("evaluation failed"))
    );
    assert_eq!(
        search(&domain, &0, true, limits(1)),
        Err(SearchError::Domain("transition failed"))
    );
}
#[test]
fn integer_extremes_do_not_overflow_or_lose_best_move() {
    use NodeRole::*;
    for role in [Maximize, Minimize] {
        let score = if role == Maximize { i32::MIN } else { i32::MAX };
        let tree = Tree(vec![node(role, 0, &[1]), node(role, score, &[])]);
        let result = search(&tree, &0, true, limits(1)).unwrap();
        assert_eq!(result.score, Some(score));
        assert_eq!(result.best_action, Some(1));
    }
}
fn generated_tree(seed: &mut u64, depth: u16, nodes: &mut Vec<Node>) -> usize {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let id = nodes.len();
    let score = ((*seed >> 32) % 201) as i32 - 100;
    let role = if *seed & 1 == 0 {
        NodeRole::Maximize
    } else {
        NodeRole::Minimize
    };
    nodes.push(node(role, score, &[]));
    if depth > 0 {
        let children = (0..3)
            .map(|_| generated_tree(seed, depth - 1, nodes))
            .collect();
        nodes[id].children = children;
        nodes[id].terminal = false;
    }
    id
}
fn exhaustive(tree: &Tree, p: usize, depth: u16) -> i32 {
    let node = &tree.0[p];
    if depth == 0 || node.terminal {
        return node.score;
    }
    let scores = node
        .children
        .iter()
        .map(|&c| exhaustive(tree, c, depth - 1));
    match node.role {
        NodeRole::Maximize => scores.max().unwrap(),
        NodeRole::Minimize => scores.min().unwrap(),
    }
}
#[test]
fn alpha_beta_matches_exhaustive_minimax_across_generated_trees_and_depths() {
    for mut seed in 0..40 {
        let mut nodes = Vec::new();
        generated_tree(&mut seed, 4, &mut nodes);
        let tree = Tree(nodes);
        for depth in 0..=4 {
            let result = search(&tree, &0, true, limits(depth)).unwrap();
            assert_eq!(result.score, Some(exhaustive(&tree, 0, depth)));
            if let Some(action) = result.best_action {
                assert_eq!(exhaustive(&tree, action, depth - 1), result.score.unwrap());
            }
        }
    }
}
#[test]
fn alpha_beta_prunes_and_results_are_deterministic() {
    let mut tree = trap();
    tree.0[2].score = 10;
    let result = analyze(
        &tree,
        &0,
        true,
        SearchOptions {
            limits: limits(2),
            principal_variation_search: false,
            ..SearchOptions::default()
        },
        || false,
        |_| {},
    )
    .unwrap();
    // Iteration one: 3 nodes. Full depth two without pruning: 7 nodes.
    assert!(result.nodes < 10);
    assert!(result.statistics.cutoffs > 0);
}

struct Cached(Tree);
impl SearchDomain for Cached {
    type Position = usize;
    type Action = usize;
    type Perspective = bool;
    type Error = &'static str;
    fn terminal_score(&self, p: &usize, v: bool) -> Result<Option<i32>, Self::Error> {
        self.0.terminal_score(p, v)
    }
    fn evaluate(&self, p: &usize, v: bool) -> Result<i32, Self::Error> {
        self.0.evaluate(p, v)
    }
    fn role(&self, p: &usize, v: bool) -> Result<NodeRole, Self::Error> {
        self.0.role(p, v)
    }
    fn actions(&self, p: &usize) -> Result<Vec<usize>, Self::Error> {
        self.0.actions(p)
    }
    fn play(&self, p: &usize, a: &usize) -> Result<usize, Self::Error> {
        self.0.play(p, a)
    }
    fn can_reduce(&self, _: &usize, _: &usize) -> bool {
        true
    }
    fn allows_stand_pat(&self, _: &usize) -> Result<Option<bool>, Self::Error> {
        // This abstract tree has no tactical continuations; exercise the fast
        // path while comparing the cached search with exhaustive minimax.
        Ok(Some(true))
    }
    fn position_key(&self, p: &usize) -> Option<u128> {
        Some(*p as u128)
    }
    fn action_key(&self, a: &usize) -> Option<u64> {
        Some(*a as u64)
    }
}
#[test]
fn transpositions_reduce_work_and_collision_replacement_preserves_scores_and_pv() {
    use NodeRole::*;
    let domain = Cached(Tree(vec![
        node(Maximize, 0, &[1, 2]),
        node(Minimize, 0, &[3, 4]),
        node(Minimize, 0, &[3, 4]),
        node(Maximize, 0, &[5, 6]),
        node(Maximize, 0, &[6, 7]),
        node(Minimize, 5, &[]),
        node(Minimize, 8, &[]),
        node(Minimize, -2, &[]),
    ]));
    let options = SearchOptions {
        limits: limits(3),
        principal_variation_search: false,
        aspiration_window: 0,
        transposition_entries: 64,
        ..SearchOptions::default()
    };
    let cached = analyze(&domain, &0, true, options, || false, |_| {}).unwrap();
    let plain = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            transposition_entries: 0,
            ..options
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(cached.score, Some(exhaustive(&domain.0, 0, 3)));
    assert_eq!(cached.score, plain.score);
    assert!(cached.statistics.transposition_hits > 0);
    assert!(cached.nodes < plain.nodes);
    let collision = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            transposition_entries: 1,
            ..options
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(cached.score, collision.score);
    for result in [cached, collision] {
        let mut position = 0;
        for action in result.principal_variation {
            position = domain.play(&position, &action).unwrap();
        }
        assert_eq!(
            domain.terminal_score(&position, true).unwrap(),
            result.score
        );
    }
}
#[test]
fn cached_bounds_and_pvs_match_exhaustive_search_on_generated_dags() {
    for mut seed in 0..25 {
        let mut nodes = Vec::new();
        generated_tree(&mut seed, 4, &mut nodes);
        // Share a subtree at the same depth, creating real transpositions.
        let left = nodes[0].children[0];
        let right = nodes[0].children[1];
        nodes[right].children[0] = nodes[left].children[0];
        let domain = Cached(Tree(nodes));
        for capacity in [1, 8, 128] {
            let result = analyze(
                &domain,
                &0,
                true,
                SearchOptions {
                    limits: limits(4),
                    transposition_entries: capacity,
                    aspiration_window: 1,
                    ..SearchOptions::default()
                },
                || false,
                |_| {},
            )
            .unwrap();
            assert_eq!(result.score, Some(exhaustive(&domain.0, 0, 4)));
        }
    }
}
struct Tactical(Tree);
impl SearchDomain for Tactical {
    type Position = usize;
    type Action = usize;
    type Perspective = bool;
    type Error = &'static str;
    fn terminal_score(&self, p: &usize, v: bool) -> Result<Option<i32>, Self::Error> {
        self.0.terminal_score(p, v)
    }
    fn evaluate(&self, p: &usize, v: bool) -> Result<i32, Self::Error> {
        self.0.evaluate(p, v)
    }
    fn role(&self, p: &usize, v: bool) -> Result<NodeRole, Self::Error> {
        self.0.role(p, v)
    }
    fn actions(&self, p: &usize) -> Result<Vec<usize>, Self::Error> {
        self.0.actions(p)
    }
    fn play(&self, p: &usize, a: &usize) -> Result<usize, Self::Error> {
        self.0.play(p, a)
    }
    fn quiescence(&self, p: &usize) -> Result<Option<Quiescence<usize>>, Self::Error> {
        Ok((*p == 1).then_some(Quiescence {
            actions: vec![3],
            stand_pat: false,
        }))
    }
}
#[test]
fn quiescence_sees_forced_tactical_reply_instead_of_standing_pat() {
    let domain = Tactical(trap());
    let options = SearchOptions {
        limits: limits(1),
        ..SearchOptions::default()
    };
    let plain = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            quiescence_depth: 0,
            ..options
        },
        || false,
        |_| {},
    )
    .unwrap();
    let tactical = analyze(&domain, &0, true, options, || false, |_| {}).unwrap();
    assert_eq!(plain.best_action, Some(1));
    assert_eq!(tactical.best_action, Some(2));
    assert_eq!(tactical.score, Some(3));
    assert!(tactical.statistics.quiescence_nodes > 0);
}
#[test]
fn progress_reports_only_completed_depths_and_budgets_include_research_and_tactics() {
    let domain = Tactical(trap());
    let mut depths = Vec::new();
    let result = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            limits: limits(3),
            aspiration_window: 1,
            ..SearchOptions::default()
        },
        || false,
        |r| depths.push((r.completed_depth, r.nodes, r.best_action)),
    )
    .unwrap();
    assert_eq!(depths.iter().map(|d| d.0).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(result.stop_reason, StopReason::Solved);
    assert!(depths.windows(2).all(|d| d[0].1 < d[1].1));
    assert_eq!(result.best_action, depths.last().unwrap().2);
    for budget in 0..30 {
        let result = search(
            &domain,
            &0,
            true,
            SearchLimits {
                max_depth: 4,
                max_nodes: Some(budget),
            },
        )
        .unwrap();
        assert!(result.nodes <= budget);
    }
}
#[test]
fn invalid_memory_and_extension_limits_are_rejected() {
    for options in [
        SearchOptions {
            transposition_entries: 1_048_577,
            ..SearchOptions::default()
        },
        SearchOptions {
            quiescence_depth: 65,
            ..SearchOptions::default()
        },
        SearchOptions {
            aspiration_window: -1,
            ..SearchOptions::default()
        },
    ] {
        assert_eq!(
            analyze(&trap(), &0, true, options, || false, |_| {}),
            Err(SearchError::InvalidOptions)
        );
    }
}

#[test]
fn played_candidate_is_searched_at_the_same_root_and_depth() {
    let tree = trap();
    let options = SearchOptions {
        limits: limits(2),
        ..Default::default()
    };
    let best = analyze(&tree, &0, true, options, || false, |_| {}).unwrap();
    let played = analyze_action(&tree, &0, true, 1, options, || false, |_| {}).unwrap();
    assert_eq!(best.score, Some(2));
    assert_eq!(played.score, Some(-8));
    assert_eq!(played.principal_variation, [1, 3]);
    assert_eq!(played.completed_depth, best.completed_depth);
    assert_eq!(
        analyze_action(&tree, &0, true, 99, options, || false, |_| {}),
        Err(SearchError::NoActions)
    );
}

#[test]
fn cached_analysis_resumes_completed_depth_and_invalidates_perspective() {
    let domain = Cached(trap());
    let mut memory = SearchMemory::new(64);
    let options = SearchOptions {
        limits: limits(1),
        transposition_entries: 64,
        ..Default::default()
    };
    let first = analyze_cached(&domain, &0, true, options, &mut memory, || false, |_| {}).unwrap();
    let mut depths = Vec::new();
    let resumed = analyze_cached(
        &domain,
        &0,
        true,
        SearchOptions {
            limits: limits(2),
            ..options
        },
        &mut memory,
        || false,
        |r| depths.push(r.completed_depth),
    )
    .unwrap();
    assert_eq!(depths, [2]);
    assert_eq!(resumed.score, Some(2));
    assert!(resumed.nodes > first.nodes);
    let returned = analyze_cached(
        &domain,
        &0,
        true,
        SearchOptions {
            limits: limits(2),
            ..options
        },
        &mut memory,
        || true,
        |_| panic!("completed work must not restart"),
    )
    .unwrap();
    assert_eq!(returned.score, resumed.score);
    assert_eq!(returned.nodes, resumed.nodes);
    let flipped = analyze_cached(
        &domain,
        &0,
        false,
        SearchOptions {
            limits: limits(2),
            ..options
        },
        &mut memory,
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(flipped.score, Some(-2));
}

#[test]
fn forced_bot_move_needs_no_evaluation_or_deep_search() {
    use NodeRole::*;
    let domain = Cached(Tree(vec![
        node(Maximize, 0, &[1]),
        node(Minimize, 0, &[2, 3]),
        node(Maximize, 9, &[]),
        node(Maximize, -3, &[]),
    ]));
    let result = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            limits: SearchLimits {
                max_depth: 256,
                max_nodes: Some(0),
            },
            stop_on_forced_move: true,
            ..Default::default()
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.best_action, Some(1));
    assert_eq!(result.stop_reason, StopReason::ForcedMove);
    assert_eq!(result.nodes, 0);
    assert_eq!(
        result.score, None,
        "forced play must not invent a depth-zero evaluation"
    );
}

#[test]
fn promising_late_probe_is_verified_at_full_depth() {
    use NodeRole::*;
    let domain = Cached(Tree(vec![
        node(Maximize, 0, &[1, 2]),
        node(Minimize, 0, &[]),
        node(Minimize, 0, &[3, 4, 5, 6, 7]),
        node(Maximize, 5, &[]),
        node(Maximize, 5, &[]),
        node(Maximize, 5, &[]),
        node(Maximize, 5, &[]),
        node(Maximize, 5, &[8]),
        node(Minimize, -1, &[9]),
        node(Maximize, -5, &[]),
    ]));
    let options = SearchOptions {
        limits: limits(4),
        aspiration_window: 0,
        late_move_reductions: true,
        ..Default::default()
    };
    let result = analyze(&domain, &0, true, options, || false, |_| {}).unwrap();
    let full = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            late_move_reductions: false,
            ..options
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.score, full.score);
    assert_eq!(result.best_action, Some(1));
    assert!(result.statistics.researches > 0);
}

#[test]
fn live_root_updates_preserve_completed_scores_and_do_not_publish_partial_depths() {
    let tree = trap();
    let stop = Cell::new(false);
    let mut seen = Vec::new();
    let mut memory = SearchMemory::new(1024);
    let options = SearchOptions {
        report_partial: true,
        limits: SearchLimits {
            max_depth: 2,
            max_nodes: Some(10000),
        },
        transposition_entries: 1024,
        ..Default::default()
    };
    let result = analyze_cached(
        &tree,
        &0,
        true,
        options,
        &mut memory,
        || stop.get(),
        |r| {
            if let Some(live) = &r.iteration {
                seen.push((r.completed_depth, live.depth, live.root_done, live.score));
                if live.depth == 2 && live.root_done == 1 && live.score.is_some() {
                    stop.set(true);
                }
            }
        },
    )
    .unwrap();
    assert_eq!(result.completed_depth, 1);
    assert_eq!(result.stop_reason, StopReason::Cancelled);
    assert!(seen
        .iter()
        .any(|(completed, depth, done, score)| *completed == 0
            && *depth == 1
            && *done == 1
            && score.is_some()));
    let live = result.iteration.as_ref().unwrap();
    assert_eq!(live.depth, 2);
    let completed = analyze(
        &tree,
        &0,
        true,
        SearchOptions {
            limits: SearchLimits {
                max_depth: 1,
                max_nodes: Some(10000),
            },
            ..Default::default()
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.score, completed.score);
    assert_eq!(result.best_action, completed.best_action);
    let resumed = analyze_cached(&tree, &0, true, options, &mut memory, || false, |_| {}).unwrap();
    assert_eq!(resumed.completed_depth, 2);
    assert!(resumed.iteration.is_none());
    assert_eq!(resumed.score, Some(exhaustive(&tree, 0, 2)));
}
#[test]
fn enabling_live_reports_preserves_exact_search_work_and_answer() {
    let tree = trap();
    let options = SearchOptions {
        limits: SearchLimits {
            max_depth: 3,
            max_nodes: Some(10000),
        },
        multi_pv: 2,
        ..Default::default()
    };
    let plain = analyze(&tree, &0, true, options, || false, |_| {}).unwrap();
    let mut reports = 0;
    let live = analyze(
        &tree,
        &0,
        true,
        SearchOptions {
            report_partial: true,
            ..options
        },
        || false,
        |r| {
            if r.iteration.is_some() {
                reports += 1;
            }
        },
    )
    .unwrap();
    assert!(reports > 0);
    assert_eq!(plain, live);
}

#[test]
fn small_chunks_finish_root_prefixes_and_all_multi_pv_passes() {
    let mut nodes = vec![node(NodeRole::Maximize, 0, &(1..=8).collect::<Vec<_>>())];
    nodes.extend((1..=8).map(|i| node(NodeRole::Minimize, i, &[])));
    let domain = Cached(Tree(nodes));
    let options = SearchOptions {
        limits: SearchLimits {
            max_depth: 1,
            max_nodes: Some(3),
        },
        transposition_entries: 64,
        multi_pv: 3,
        report_partial: true,
        ..Default::default()
    };
    let mut memory = SearchMemory::new(64);
    let mut completed = None;
    for _ in 0..32 {
        let result =
            analyze_cached(&domain, &0, true, options, &mut memory, || false, |_| {}).unwrap();
        if result.completed_depth == 1 {
            completed = Some(result);
            break;
        }
        assert!(
            result.best_action.is_none(),
            "Never promote an unfinished ranking"
        );
    }
    let result =
        completed.expect("Each chunk must advance, even when a root pass exceeds the budget");
    assert_eq!(
        result
            .variations
            .iter()
            .map(|v| v.actions[0])
            .collect::<Vec<_>>(),
        [8, 7, 6]
    );
    assert_eq!(result.score, Some(8));
}
