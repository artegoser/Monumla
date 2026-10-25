//! Bounded reusable tables, ordering statistics and completed root results.
use crate::{table::Table, SearchResult, SearchVariation};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RootFrontier<A> {
    pub key: u128,
    pub depth: u16,
    pub window: (i32, i32),
    pub excluded: Vec<A>,
    pub allowed: Option<Vec<A>>,
    pub actions: Arc<Vec<A>>,
    pub next: usize,
    pub best: Option<SearchVariation<A>>,
    pub alpha: i32,
    pub beta: i32,
}

/// Bounded reusable work. Keep one memory with one rules/evaluation model.
/// Perspective or quiescence-policy changes invalidate cached scores.
pub struct SearchMemory<A, P> {
    pub(crate) table: Table<A>,
    pub(crate) quiet_table: Table<A>,
    pub(crate) killers: Vec<Vec<A>>,
    pub(crate) history: HashMap<u64, u32>,
    pub(crate) frontier: Option<RootFrontier<A>>,
    pub(crate) partial_iterations: HashMap<u128, (u16, Vec<SearchVariation<A>>)>,
    pub(crate) results: HashMap<u128, SearchResult<A>>,
    pub(crate) perspective: Option<P>,
    pub(crate) policy: Option<(u8, usize, bool)>,
    pub(crate) capacity: usize,
}
impl<A: Clone, P> SearchMemory<A, P> {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.min(1_048_576);
        Self {
            table: Table::new(capacity),
            quiet_table: Table::new(capacity / 4),
            killers: vec![Vec::new(); 322],
            history: HashMap::new(),
            results: HashMap::new(),
            frontier: None,
            partial_iterations: HashMap::new(),
            perspective: None,
            policy: None,
            capacity,
        }
    }
}
