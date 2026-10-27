use super::*;

#[test]
fn nominal_iterations_can_exceed_physical_depth_when_moves_are_reduced() {
    fn append(nodes: &mut Vec<Node>, remaining: u16) -> usize {
        let index = nodes.len();
        nodes.push(node(
            if remaining % 2 == 0 {
                NodeRole::Maximize
            } else {
                NodeRole::Minimize
            },
            0,
            &[],
        ));
        if remaining > 0 {
            let children = (0..6).map(|_| append(nodes, remaining - 1)).collect();
            nodes[index].children = children;
            nodes[index].terminal = false;
        }
        index
    }
    let mut nodes = Vec::new();
    append(&mut nodes, 4);
    let domain = Cached(Tree(nodes));
    let result = analyze(
        &domain,
        &0,
        true,
        SearchOptions {
            limits: limits(32),
            late_move_reductions: true,
            ..Default::default()
        },
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.stop_reason, StopReason::Solved);
    assert_eq!(result.score, Some(0));
    assert_eq!(result.statistics.selective_depth, 4);
    assert!(result.completed_depth > result.statistics.selective_depth);
}

#[test]
fn finite_trees_stop_at_the_proof_instead_of_the_requested_iteration() {
    for seed in 0..40 {
        let mut random = seed;
        let mut nodes = Vec::new();
        generated_tree(&mut random, 4, &mut nodes);
        let domain = Cached(Tree(nodes));
        for capacity in [0, 1, 64] {
            for reductions in [false, true] {
                let result = analyze(
                    &domain,
                    &0,
                    true,
                    SearchOptions {
                        limits: limits(32),
                        transposition_entries: capacity,
                        late_move_reductions: reductions,
                        multi_pv: 3,
                        ..Default::default()
                    },
                    || false,
                    |_| {},
                )
                .unwrap();
                assert_eq!(result.stop_reason, StopReason::Solved, "seed {seed}");
                assert!(result.completed_depth <= 4);
                assert!(result.statistics.selective_depth <= 4);
                assert_eq!(result.score, Some(exhaustive(&domain.0, 0, 4)));
                assert!(result.variations.iter().all(|v| v.proven));
                for variation in result.variations {
                    assert_eq!(
                        variation.score,
                        exhaustive(&domain.0, variation.actions[0], 4)
                    );
                }
            }
        }
    }
}

#[test]
fn a_terminal_best_line_does_not_prove_heuristic_siblings() {
    use NodeRole::*;
    let domain = Cached(Tree(vec![
        node(Maximize, 0, &[1, 2]),
        node(Minimize, 100, &[]),
        node(Minimize, 3, &[3]),
        node(Maximize, 3, &[4]),
        node(Minimize, 3, &[4]), // No terminal result at any horizon.
    ]));
    let options = SearchOptions {
        limits: limits(3),
        quiescence_depth: 0,
        transposition_entries: 64,
        ..Default::default()
    };
    let mut memory = SearchMemory::new(64);
    // Interrupt after the first terminal root choice. The resumed prefix must
    // become unproven when the other choice reaches a heuristic leaf.
    let partial = analyze_cached(
        &domain,
        &0,
        true,
        SearchOptions {
            limits: SearchLimits {
                max_depth: 3,
                max_nodes: Some(2),
            },
            ..options
        },
        &mut memory,
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(partial.stop_reason, StopReason::NodeLimit);
    let result = analyze_cached(&domain, &0, true, options, &mut memory, || false, |_| {}).unwrap();
    assert_eq!(result.best_action, Some(1));
    assert_eq!(result.principal_variation, [1]);
    assert_eq!(result.stop_reason, StopReason::DepthLimit);
    assert!(!result.variations[0].proven);
    assert_eq!(result.statistics.selective_depth, 3);
}

#[test]
fn chunked_multipv_retains_proofs_and_solved_cache_does_no_new_work() {
    use NodeRole::*;
    let mut nodes = vec![node(Maximize, 0, &(1..=8).collect::<Vec<_>>())];
    nodes.extend((1..=8).map(|i| node(Minimize, i, &[])));
    let domain = Cached(Tree(nodes));
    let options = SearchOptions {
        limits: SearchLimits {
            max_depth: 32,
            max_nodes: Some(3),
        },
        transposition_entries: 64,
        multi_pv: 3,
        ..Default::default()
    };
    let mut memory = SearchMemory::new(64);
    let result = (0..32)
        .find_map(|_| {
            let result =
                analyze_cached(&domain, &0, true, options, &mut memory, || false, |_| {}).unwrap();
            (result.stop_reason == StopReason::Solved).then_some(result)
        })
        .expect("Completed root prefixes must eventually prove every requested variation");
    assert_eq!(result.completed_depth, 1);
    assert_eq!(result.statistics.selective_depth, 1);
    assert_eq!(
        result
            .variations
            .iter()
            .map(|v| v.score)
            .collect::<Vec<_>>(),
        [8, 7, 6]
    );
    assert!(result.variations.iter().all(|v| v.proven));
    let cached = analyze_cached(
        &domain,
        &0,
        true,
        options,
        &mut memory,
        || panic!("Solved roots need no traversal"),
        |_| panic!("No new iteration"),
    )
    .unwrap();
    assert_eq!(cached, result);
}

#[test]
fn proven_transpositions_survive_heuristic_replacements_and_greater_depths() {
    use crate::table::{Bound, Entry, Table};
    let mut table = Table::new(1);
    table.store(Entry {
        key: 1,
        depth: 2,
        score: 5,
        bound: Bound::Lower,
        proven: true,
        line: vec![7],
        generation: 2,
    });
    table.store(Entry {
        key: 1,
        depth: 32,
        score: 999,
        bound: Bound::Exact,
        proven: false,
        line: vec![8],
        generation: 32,
    });
    assert_eq!(table.get(1).unwrap().score, 5);
    let domain = Cached(trap());
    let options = SearchOptions {
        limits: limits(32),
        transposition_entries: 64,
        ..Default::default()
    };
    let mut memory = SearchMemory::new(64);
    let solved = analyze_cached(&domain, &0, true, options, &mut memory, || false, |_| {}).unwrap();
    memory.results.clear(); // Exercise the TT rather than the completed-root cache.
    let reused = analyze_cached(&domain, &0, true, options, &mut memory, || false, |_| {}).unwrap();
    assert_eq!(reused.stop_reason, StopReason::Solved);
    assert_eq!(reused.score, solved.score);
    assert_eq!(reused.principal_variation, solved.principal_variation);
    assert_eq!(reused.nodes, 1);
    assert!(reused.statistics.transposition_hits > 0);
}
