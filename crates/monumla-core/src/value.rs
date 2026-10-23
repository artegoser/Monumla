use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Default, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StateMap(Arc<BTreeMap<String, StateValue>>);

impl StateMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&StateValue> {
        self.0.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut StateValue> {
        Arc::make_mut(&mut self.0).get_mut(key)
    }

    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: impl Into<StateValue>,
    ) -> Option<StateValue> {
        Arc::make_mut(&mut self.0).insert(key.into(), value.into())
    }

    pub fn remove(&mut self, key: &str) -> Option<StateValue> {
        Arc::make_mut(&mut self.0).remove(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &StateValue)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StateValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
    List(Vec<StateValue>),
    Map(StateMap),
}

impl StateValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

impl From<bool> for StateValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for StateValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for StateValue {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

impl From<u32> for StateValue {
    fn from(value: u32) -> Self {
        Self::U64(u64::from(value))
    }
}

impl From<f64> for StateValue {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<String> for StateValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for StateValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<Vec<StateValue>> for StateValue {
    fn from(value: Vec<StateValue>) -> Self {
        Self::List(value)
    }
}

impl From<StateMap> for StateValue {
    fn from(value: StateMap) -> Self {
        Self::Map(value)
    }
}

// Canonicalize signed zero because f64 equality considers +0 and -0 equal.
impl std::hash::Hash for StateValue {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        std::mem::discriminant(self).hash(hasher);
        match self {
            Self::Bool(v) => v.hash(hasher),
            Self::I64(v) => v.hash(hasher),
            Self::U64(v) => v.hash(hasher),
            Self::F64(v) => (if *v == 0.0 { 0 } else { v.to_bits() }).hash(hasher),
            Self::String(v) => v.hash(hasher),
            Self::List(v) => v.hash(hasher),
            Self::Map(v) => v.hash(hasher),
        }
    }
}

/// Deterministic in-process identity, independent of pointer addresses.
/// Not a portable file hash or a cryptographic digest.
pub fn fingerprint(value: &impl std::hash::Hash) -> u128 {
    use std::hash::{Hash, Hasher};
    let mut low = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut low);
    let mut high = std::collections::hash_map::DefaultHasher::new();
    0x9e37_79b9_7f4a_7c15_u64.hash(&mut high);
    value.hash(&mut high);
    u128::from(low.finish()) | (u128::from(high.finish()) << 64)
}

#[cfg(test)]
mod sharing_tests {
    use super::*;
    #[test]
    fn branches_keep_nested_values_isolated() {
        let mut nested = StateMap::new();
        nested.insert("count", 3_u64);
        let mut original = StateMap::new();
        original.insert("nested", nested);
        original.insert("keep", true);
        let mut branch = original.clone();
        let StateValue::Map(child) = branch.get_mut("nested").unwrap() else {
            panic!()
        };
        child.insert("count", 9_u64);
        branch.remove("keep");
        branch.insert("new", false);
        let StateValue::Map(child) = original.get("nested").unwrap() else {
            panic!()
        };
        assert_eq!(child.get("count").and_then(StateValue::as_u64), Some(3));
        assert!(original.contains_key("keep"));
        assert!(!original.contains_key("new"));
        assert_eq!(original, original.clone());
    }
}
