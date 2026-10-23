//! Bounded reusable tables, ordering statistics and completed root results.
use crate::{table::Table, SearchResult};
use std::collections::HashMap;

/// Bounded reusable work. Keep one memory with one rules/evaluation model.
/// Perspective or quiescence-policy changes invalidate cached scores.
pub struct SearchMemory<A, P> {
    pub(crate) table: Table<A>,
    pub(crate) quiet_table: Table<A>,
    pub(crate) killers: Vec<Vec<A>>,
    pub(crate) history: HashMap<u64, u32>,
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
            perspective: None,
            policy: None,
            capacity,
        }
    }
}
