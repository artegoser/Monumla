use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StateMap(BTreeMap<String, StateValue>);

impl StateMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&StateValue> {
        self.0.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut StateValue> {
        self.0.get_mut(key)
    }

    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: impl Into<StateValue>,
    ) -> Option<StateValue> {
        self.0.insert(key.into(), value.into())
    }

    pub fn remove(&mut self, key: &str) -> Option<StateValue> {
        self.0.remove(key)
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
