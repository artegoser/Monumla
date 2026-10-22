use super::*;
use std::cell::Cell;

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
    let result = search(&tree, &0, true, limits(2)).unwrap();
    // Iteration one: 3 nodes. Full depth two without pruning: 7 nodes.
    assert!(result.nodes < 10);
    assert_eq!(result, search(&tree, &0, true, limits(2)).unwrap());
}
