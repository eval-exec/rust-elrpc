use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents values that can be serialized/deserialized in EPC protocol
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EpcValue {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Symbol(String),
    List(Vec<EpcValue>),
    Dict(HashMap<String, EpcValue>),
}

impl EpcValue {
    pub fn is_nil(&self) -> bool {
        matches!(self, EpcValue::Nil)
    }
    
    pub fn as_string(&self) -> Option<&str> {
        match self {
            EpcValue::String(s) => Some(s),
            _ => None,
        }
    }
    
    pub fn as_int(&self) -> Option<i64> {
        match self {
            EpcValue::Int(i) => Some(*i),
            _ => None,
        }
    }
    
    pub fn as_list(&self) -> Option<&Vec<EpcValue>> {
        match self {
            EpcValue::List(list) => Some(list),
            _ => None,
        }
    }
}

impl From<()> for EpcValue {
    fn from(_: ()) -> Self {
        EpcValue::Nil
    }
}

impl From<bool> for EpcValue {
    fn from(b: bool) -> Self {
        EpcValue::Bool(b)
    }
}

impl From<i32> for EpcValue {
    fn from(i: i32) -> Self {
        EpcValue::Int(i as i64)
    }
}

impl From<i64> for EpcValue {
    fn from(i: i64) -> Self {
        EpcValue::Int(i)
    }
}

impl From<f64> for EpcValue {
    fn from(f: f64) -> Self {
        EpcValue::Float(f)
    }
}

impl From<String> for EpcValue {
    fn from(s: String) -> Self {
        EpcValue::String(s)
    }
}

impl From<&str> for EpcValue {
    fn from(s: &str) -> Self {
        EpcValue::String(s.to_string())
    }
}

impl<T: Into<EpcValue>> From<Vec<T>> for EpcValue {
    fn from(vec: Vec<T>) -> Self {
        EpcValue::List(vec.into_iter().map(|x| x.into()).collect())
    }
}

impl<T: Into<EpcValue>> From<HashMap<String, T>> for EpcValue {
    fn from(map: HashMap<String, T>) -> Self {
        EpcValue::Dict(map.into_iter().map(|(k, v)| (k, v.into())).collect())
    }
}