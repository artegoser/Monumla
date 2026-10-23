//! Game-independent search over complete actions, with bounded memory and work.
//! Games supply ordinary rules through the shared runtime binding.
#![forbid(unsafe_code)]

mod engine;
mod memory;
pub mod runtime;
mod table;
mod types;

pub use engine::{analyze, analyze_action, analyze_cached, search, search_with_control};
pub use memory::SearchMemory;
pub use monumla_core::fingerprint;
pub use types::*;

#[cfg(test)]
mod tests;
